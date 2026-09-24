use std::path::Path;

use anyhow::{Context, Result, bail};
use owo_colors::OwoColorize;

use crate::client::{ClaudeMdResponse, Client};
use crate::config;
use crate::output::{self, OutputMode};
use crate::{ContextCommand, ContextSnapshotCommand};

pub async fn run(client: &Client, command: ContextCommand, mode: OutputMode) -> Result<()> {
    match command {
        ContextCommand::About {
            concept,
            max_tokens,
            space,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let format = if mode == OutputMode::Json {
                "structured-json"
            } else {
                "structured-md"
            };
            let value = client
                .context_about(
                    &concept,
                    space.as_deref(),
                    checked_budget(max_tokens)?,
                    format,
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::Export {
            max_tokens,
            space,
            format,
            for_path,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let value = client
                .export_context(
                    space.as_deref(),
                    checked_budget(max_tokens.unwrap_or(settings.context.max_tokens))?,
                    &format,
                    for_path.as_deref(),
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::For {
            path,
            space,
            max_tokens,
            format,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let value = client
                .context_for_file(
                    &path.to_string_lossy(),
                    space.as_deref(),
                    checked_budget(max_tokens.unwrap_or(settings.context.max_tokens))?,
                    &format,
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::ForPlan {
            plan_id,
            max_tokens,
            space: _,
        } => {
            let format = context_format(mode);
            let value = client
                .context_for_plan(&plan_id, checked_budget(max_tokens)?, format)
                .await?;
            render_context(value, mode)
        }
        ContextCommand::ForDiff {
            staged,
            max_tokens,
            space,
        } => {
            let paths = changed_files(staged)?;
            if paths.is_empty() {
                return match mode {
                    OutputMode::Json => {
                        output::json(&serde_json::json!({ "files": [], "chunks": [] }))
                    }
                    _ => {
                        println!("No changed files.");
                        Ok(())
                    }
                };
            }
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let value = client
                .context_for_files(
                    &paths,
                    space.as_deref(),
                    checked_budget(max_tokens)?,
                    context_format(mode),
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::Dir {
            directory,
            space,
            output: output_path,
            max_files,
            max_tokens,
        } => {
            if max_files == 0 {
                bail!("max files must be greater than zero");
            }
            let paths = collect_directory_files(&directory, max_files)?;
            if paths.is_empty() {
                bail!("no files found in {}", directory.display());
            }
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let format = if output_path.is_some() {
                "structured-md"
            } else {
                context_format(mode)
            };
            let value = client
                .context_for_files(
                    &paths,
                    space.as_deref(),
                    checked_budget(max_tokens.unwrap_or(settings.context.max_tokens))?,
                    format,
                )
                .await?;
            if let Some(path) = output_path {
                let content = value["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("context response omitted markdown content"))?;
                write_generated(&path, content)?;
                return match mode {
                    OutputMode::Json => output::json(&serde_json::json!({
                        "output": path,
                        "files": paths.len(),
                        "chunks": value["totalChunks"],
                    })),
                    OutputMode::Quiet => {
                        println!("{}", path.display());
                        Ok(())
                    }
                    OutputMode::Human => {
                        println!(
                            "Wrote context for {} file(s) to {}",
                            paths.len(),
                            path.display()
                        );
                        Ok(())
                    }
                };
            }
            render_context(value, mode)
        }
        ContextCommand::Snapshot { command } => run_snapshot(client, command, mode).await,
        ContextCommand::ClaudeMd {
            space,
            tag,
            max_tokens,
            output: path,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let response = client
                .claude_md(
                    space.as_deref(),
                    tag.as_deref().or(Some(settings.claude_md.tag.as_str())),
                    checked_budget(max_tokens.unwrap_or(settings.claude_md.max_tokens))?,
                )
                .await?;
            if let Some(path) = path {
                write_generated(&path, &response.content)?;
                render_written(&path, &response, mode)
            } else {
                render_claude_md(&response, mode)
            }
        }
    }
}

async fn run_snapshot(
    client: &Client,
    command: ContextSnapshotCommand,
    mode: OutputMode,
) -> Result<()> {
    match command {
        ContextSnapshotCommand::Create {
            plan,
            task,
            about,
            files,
            max_tokens,
            space,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let value = client
                .create_context_snapshot(
                    plan.as_deref(),
                    task.as_deref(),
                    about.as_deref(),
                    &files,
                    space.as_deref(),
                    checked_budget(max_tokens)?,
                )
                .await?;
            let id = value["snapshotId"].as_str().unwrap_or("");
            if !output::id_or_json(mode, id, &value)? {
                println!(
                    "Snapshot created: {id}\nChunks: {}  Tokens: {}\nCreated: {}",
                    value["chunkCount"].as_u64().unwrap_or(0),
                    value["tokenCount"].as_u64().unwrap_or(0),
                    value["createdAt"].as_str().unwrap_or("")
                );
            }
            Ok(())
        }
        ContextSnapshotCommand::Get { snapshot_id } => {
            let value = client.get_context_snapshot(&snapshot_id).await?;
            render_snapshot(&value, mode)
        }
        ContextSnapshotCommand::List => {
            let snapshots = client.list_context_snapshots().await?;
            match mode {
                OutputMode::Json => output::json(&snapshots),
                OutputMode::Quiet => {
                    for snapshot in snapshots {
                        if let Some(id) = snapshot["id"].as_str() {
                            println!("{id}");
                        }
                    }
                    Ok(())
                }
                OutputMode::Human => {
                    if snapshots.is_empty() {
                        println!("No snapshots found.");
                    }
                    for snapshot in snapshots {
                        println!(
                            "{}  tokens:{}  {}",
                            snapshot["id"].as_str().unwrap_or("?"),
                            snapshot["tokenCount"].as_u64().unwrap_or(0),
                            snapshot["createdAt"].as_str().unwrap_or("")
                        );
                    }
                    Ok(())
                }
            }
        }
        ContextSnapshotCommand::Delete { snapshot_id } => {
            client.delete_context_snapshot(&snapshot_id).await?;
            match mode {
                OutputMode::Json => output::json(&serde_json::json!({
                    "deleted": true, "snapshotId": snapshot_id
                })),
                _ => {
                    println!("Deleted snapshot {snapshot_id}");
                    Ok(())
                }
            }
        }
    }
}

fn render_snapshot(value: &serde_json::Value, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(value);
    }
    println!(
        "# Context Snapshot: {}\nCreated: {}  Tokens: {}  Chunks: {}\n",
        value["id"].as_str().unwrap_or("?"),
        value["createdAt"].as_str().unwrap_or(""),
        value["tokenCount"].as_u64().unwrap_or(0),
        value["chunks"].as_array().map_or(0, Vec::len)
    );
    if let Some(chunks) = value["chunks"].as_array() {
        for chunk in chunks {
            println!(
                "## {} [{}]\n\n{}\n",
                chunk["title"].as_str().unwrap_or("Untitled"),
                chunk["type"].as_str().unwrap_or("unknown"),
                chunk["content"].as_str().unwrap_or("")
            );
            if let Some(rationale) = chunk["rationale"].as_str() {
                println!("Rationale: {rationale}\n");
            }
        }
    }
    Ok(())
}

pub async fn sync(
    client: &Client,
    output_path: Option<&Path>,
    space: Option<&str>,
    tag: Option<&str>,
    max_tokens: Option<usize>,
    dry_run: bool,
    mode: OutputMode,
) -> Result<()> {
    let (settings, _) = config::load()?;
    let path = output_path.unwrap_or(&settings.claude_md.output);
    let space = client
        .resolve_space(space.or(settings.space.as_deref()))
        .await?;
    let response = client
        .claude_md(
            space.as_deref(),
            tag.or(Some(settings.claude_md.tag.as_str())),
            checked_budget(max_tokens.unwrap_or(settings.claude_md.max_tokens))?,
        )
        .await?;
    if dry_run {
        return match mode {
            OutputMode::Json => output::json(&serde_json::json!({
                "dryRun": true, "output": path, "chunks": response.chunks,
                "bytes": response.content.len(), "content": response.content,
            })),
            _ => {
                print!("{}", response.content);
                if !response.content.ends_with('\n') {
                    println!();
                }
                Ok(())
            }
        };
    }
    write_generated(path, &response.content)?;
    render_written(path, &response, mode)
}

fn checked_budget(value: usize) -> Result<usize> {
    if value == 0 {
        bail!("max tokens must be greater than zero");
    }
    Ok(value)
}

fn context_format(mode: OutputMode) -> &'static str {
    if mode == OutputMode::Json {
        "structured-json"
    } else {
        "structured-md"
    }
}

fn changed_files(staged: bool) -> Result<Vec<String>> {
    let mut command = std::process::Command::new("git");
    command.args(["diff", "--name-only"]);
    if staged {
        command.arg("--staged");
    }
    let result = command.output().context("could not run git diff")?;
    if !result.status.success() {
        bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(String::from_utf8(result.stdout)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn collect_directory_files(directory: &Path, max_files: usize) -> Result<Vec<String>> {
    let cwd = std::env::current_dir()?;
    let root = if directory.is_absolute() {
        directory.to_owned()
    } else {
        cwd.join(directory)
    };
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let mut files = Vec::new();
    collect_directory_files_into(&root, &cwd, max_files, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_directory_files_into(
    directory: &Path,
    cwd: &Path,
    max_files: usize,
    files: &mut Vec<String>,
) -> Result<()> {
    if files.len() >= max_files {
        return Ok(());
    }
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("could not read directory {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        if files.len() >= max_files {
            break;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_directory_files_into(&entry.path(), cwd, max_files, files)?;
        } else if file_type.is_file() {
            let path = entry.path();
            files.push(
                path.strip_prefix(cwd)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    Ok(())
}

fn render_context(value: serde_json::Value, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&value);
    }
    if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
        print!("{content}");
        if !content.ends_with('\n') {
            println!();
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}

fn render_claude_md(response: &ClaudeMdResponse, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(response);
    }
    print!("{}", response.content);
    if !response.content.ends_with('\n') {
        println!();
    }
    Ok(())
}

fn render_written(path: &Path, response: &ClaudeMdResponse, mode: OutputMode) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({
            "output": path, "chunks": response.chunks, "bytes": response.content.len()
        }))?,
        OutputMode::Quiet => println!("{}", path.display()),
        OutputMode::Human => println!(
            "{} {} ({} chunks)",
            "wrote".green(),
            path.display(),
            response.chunks
        ),
    }
    Ok(())
}

fn write_generated(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temporary = path.with_extension(format!("fubbik.{}.tmp", std::process::id()));
    std::fs::write(&temporary, content)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::collect_directory_files_into;

    #[test]
    fn directory_collection_skips_hidden_and_dependency_directories_and_honors_limit() {
        // Given a source tree with visible, hidden, and dependency files
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();
        std::fs::create_dir(root.path().join("node_modules")).unwrap();
        std::fs::write(root.path().join("README.md"), "readme").unwrap();
        std::fs::write(root.path().join("src/lib.rs"), "lib").unwrap();
        std::fs::write(root.path().join("src/main.rs"), "main").unwrap();
        std::fs::write(root.path().join(".git/config"), "hidden").unwrap();
        std::fs::write(root.path().join("node_modules/pkg.js"), "dependency").unwrap();

        // When at most two context paths are collected
        let mut files = Vec::new();
        collect_directory_files_into(root.path(), root.path(), 2, &mut files).unwrap();

        // Then collection is bounded and excluded directories are absent
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|path| !path.contains(".git")));
        assert!(files.iter().all(|path| !path.contains("node_modules")));
    }
}
