use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Deserialize)]
struct JsonChunk {
    title: String,
    #[serde(default)]
    content: String,
    #[serde(rename = "type")]
    chunk_type: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JsonImport {
    Array(Vec<JsonChunk>),
    Envelope { chunks: Vec<JsonChunk> },
}

impl JsonImport {
    fn into_chunks(self) -> Vec<JsonChunk> {
        match self {
            Self::Array(chunks) | Self::Envelope { chunks } => chunks,
        }
    }
}

pub async fn run(
    client: &Client,
    path: &Path,
    space: Option<&str>,
    default_type: &str,
    recursive: bool,
    mode: OutputMode,
) -> Result<()> {
    if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("json") {
        return import_json(client, path, space, default_type, mode).await;
    }
    import_markdown(client, path, space, recursive, mode).await
}

async fn import_json(
    client: &Client,
    path: &Path,
    space: Option<&str>,
    default_type: &str,
    mode: OutputMode,
) -> Result<()> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let chunks: JsonImport = serde_json::from_str(&raw).context("invalid JSON import format")?;
    let space_id = client.resolve_space(space).await?;
    let spaces = space_id.into_iter().collect::<Vec<_>>();
    let mut added = Vec::new();
    let mut errors = Vec::new();
    for chunk in chunks.into_chunks() {
        match client
            .create_chunk(
                &chunk.title,
                &chunk.content,
                chunk.chunk_type.as_deref().unwrap_or(default_type),
                &chunk.tags,
                &spaces,
            )
            .await
        {
            Ok(chunk) => added.push(serde_json::json!({"id": chunk.id, "title": chunk.title})),
            Err(error) => errors.push(format!("{}: {error}", chunk.title)),
        }
    }
    render_json_import(&added, &errors, mode)?;
    if !errors.is_empty() {
        bail!("{} chunk(s) could not be imported", errors.len());
    }
    Ok(())
}

async fn import_markdown(
    client: &Client,
    path: &Path,
    space: Option<&str>,
    recursive: bool,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client
        .resolve_space(space)
        .await?
        .context("Markdown import requires --space")?;
    let (base, paths) = markdown_paths(path, recursive)?;
    if paths.is_empty() {
        bail!("no .md files found");
    }
    if paths.len() > 500 {
        bail!("found {} files; import at most 500 at a time", paths.len());
    }
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = path.strip_prefix(&base).unwrap_or(&path);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read {}", path.display()))?;
        files.push((relative.to_string_lossy().into_owned(), content));
    }
    let result = client.import_chunk_documents(&files, &space_id).await?;
    if mode == OutputMode::Json {
        return output::json(&result);
    }
    if mode == OutputMode::Quiet {
        println!("{}", result["created"].as_i64().unwrap_or(0));
    } else {
        println!(
            "Created: {} | Skipped: {} | Errors: {}",
            result["created"].as_i64().unwrap_or(0),
            result["skipped"].as_i64().unwrap_or(0),
            result["errors"].as_array().map_or(0, Vec::len)
        );
    }
    Ok(())
}

fn render_json_import(
    added: &[serde_json::Value],
    errors: &[String],
    mode: OutputMode,
) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&serde_json::json!({"added": added, "errors": errors}));
    }
    if mode == OutputMode::Quiet {
        for chunk in added {
            println!("{}", chunk["id"].as_str().unwrap_or(""));
        }
    } else {
        println!("Imported {} chunks", added.len());
        for error in errors {
            eprintln!("Failed: {error}");
        }
    }
    Ok(())
}

fn markdown_paths(path: &Path, recursive: bool) -> Result<(PathBuf, Vec<PathBuf>)> {
    if path.is_file() {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            bail!("path must be a .json file, .md file, or directory");
        }
        let absolute = path.canonicalize()?;
        let base = absolute
            .parent()
            .context("Markdown file has no parent")?
            .to_owned();
        return Ok((base, vec![absolute]));
    }
    if !path.is_dir() {
        bail!("path not found: {}", path.display());
    }
    let base = path.canonicalize()?;
    let mut files = Vec::new();
    collect_markdown(&base, recursive, &mut files)?;
    files.sort();
    Ok((base, files))
}

fn collect_markdown(directory: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && recursive {
            collect_markdown(&entry.path(), true, files)?;
        } else if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_collection_honors_recursive_mode() {
        // Given Markdown files at the root and in a nested directory
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("root.md"), "root").unwrap();
        std::fs::create_dir(directory.path().join("nested")).unwrap();
        std::fs::write(directory.path().join("nested/child.md"), "child").unwrap();
        // When files are collected with and without recursion
        let (_, shallow) = markdown_paths(directory.path(), false).unwrap();
        let (_, deep) = markdown_paths(directory.path(), true).unwrap();
        // Then nested Markdown is only present in recursive mode
        assert_eq!(shallow.len(), 1);
        assert_eq!(deep.len(), 2);
    }
}
