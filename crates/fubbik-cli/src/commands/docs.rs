use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::DocsCommand;
use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: DocsCommand, mode: OutputMode) -> Result<()> {
    match command {
        DocsCommand::Extract {
            path,
            language,
            project,
            space,
            preview,
        } => {
            super::source_docs::run(
                client,
                &path,
                language.as_deref(),
                project.as_deref(),
                space.as_deref(),
                preview,
                mode,
            )
            .await
        }
        DocsCommand::List { space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let documents = client.list_documents(space.as_deref()).await?;
            match mode {
                OutputMode::Json => output::json(&documents),
                OutputMode::Quiet => {
                    for document in documents {
                        if let Some(id) = document["id"].as_str() {
                            println!("{id}");
                        }
                    }
                    Ok(())
                }
                OutputMode::Human => {
                    if documents.is_empty() {
                        println!("No documents found.");
                    }
                    for document in documents {
                        println!(
                            "{}  {}",
                            document["id"].as_str().unwrap_or("?"),
                            document["title"].as_str().unwrap_or("Untitled")
                        );
                    }
                    Ok(())
                }
            }
        }
        DocsCommand::Show { id } => render_value(client.get_document(&id).await?, mode),
        DocsCommand::Import { path, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let source_path = absolute(&path)?;
            let content = std::fs::read_to_string(&source_path)
                .with_context(|| format!("could not read {}", source_path.display()))?;
            let result = client
                .import_document(&source_path.to_string_lossy(), &content, space.as_deref())
                .await?;
            render_mutation(result, "Imported", mode)
        }
        DocsCommand::ImportDir { dir, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let files = collect_markdown_files(&dir)?;
            if files.is_empty() {
                bail!("no .md files found in directory");
            }
            let mut documents = Vec::with_capacity(files.len());
            for path in files {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("could not read {}", path.display()))?;
                documents.push((path.to_string_lossy().into_owned(), content));
            }
            let results = client
                .import_documents(&documents, space.as_deref())
                .await?;
            match mode {
                OutputMode::Json => output::json(&results),
                OutputMode::Quiet => {
                    for result in results {
                        if let Some(id) = result["document"]["id"].as_str() {
                            println!("{id}");
                        }
                    }
                    Ok(())
                }
                OutputMode::Human => {
                    println!("Imported {} document(s)", results.len());
                    Ok(())
                }
            }
        }
        DocsCommand::Sync { id, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let document = client.get_document(&id).await?;
            let source = document["sourcePath"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("document has no source path"))?;
            if source.starts_with("source-docs://") {
                bail!(
                    "source documentation must be refreshed with `fubbik docs extract` using the same project, language and space"
                );
            }
            let content = std::fs::read_to_string(source)
                .with_context(|| format!("could not read {source}"))?;
            render_mutation(
                client
                    .sync_document(&id, &content, space.as_deref())
                    .await?,
                "Synced",
                mode,
            )
        }
        DocsCommand::Render { id } => {
            let value = client.render_document(&id).await?;
            if mode == OutputMode::Json {
                output::json(&value)
            } else {
                let markdown = value["markdown"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("render response omitted markdown"))?;
                print!("{markdown}");
                Ok(())
            }
        }
    }
}

fn absolute(path: &Path) -> Result<std::path::PathBuf> {
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
        bail!("document import requires a .md file");
    }
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    })
}

fn collect_markdown_files(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let dir = if dir.is_absolute() {
        dir.to_owned()
    } else {
        std::env::current_dir()?.join(dir)
    };
    if !dir.is_dir() {
        bail!("not a directory: {}", dir.display());
    }
    let mut files = Vec::new();
    collect_markdown_files_into(&dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown_files_into(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("could not read directory {}", dir.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_markdown_files_into(&entry.path(), files)?;
        } else if file_type.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn render_mutation(value: serde_json::Value, verb: &str, mode: OutputMode) -> Result<()> {
    let id = value["document"]["id"].as_str().unwrap_or("");
    if !output::id_or_json(mode, id, &value)? {
        println!("{verb} document {id}");
    }
    Ok(())
}

fn render_value(value: serde_json::Value, mode: OutputMode) -> Result<()> {
    let id = value["id"].as_str().unwrap_or("");
    if !output::id_or_json(mode, id, &value)? {
        println!(
            "{}\n{}",
            value["title"].as_str().unwrap_or("Untitled"),
            serde_json::to_string_pretty(&value)?
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::collect_markdown_files;

    #[test]
    fn markdown_collection_recurses_and_ignores_other_files() {
        // Given a directory with Markdown at two levels and an unrelated file
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("nested")).unwrap();
        std::fs::write(root.path().join("root.md"), "root").unwrap();
        std::fs::write(root.path().join("nested/child.md"), "child").unwrap();
        std::fs::write(root.path().join("ignored.txt"), "ignored").unwrap();

        // When Markdown files are collected recursively
        let files = collect_markdown_files(root.path()).unwrap();

        // Then only the Markdown files are returned in stable path order
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("nested/child.md"));
        assert!(files[1].ends_with("root.md"));
    }
}
