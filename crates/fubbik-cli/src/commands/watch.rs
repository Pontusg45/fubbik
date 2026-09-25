use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, directory: &Path, mode: OutputMode) -> Result<()> {
    let root = directory
        .canonicalize()
        .with_context(|| format!("could not open {}", directory.display()))?;
    if !root.is_dir() {
        bail!("not a directory: {}", directory.display());
    }
    let mut known = snapshot(&root)?;
    if mode == OutputMode::Human {
        println!("Watching {} for changes...", root.display());
    }
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    loop {
        interval.tick().await;
        let current = snapshot(&root)?;
        for (path, modified) in &current {
            let changed = match known.get(path) {
                Some(previous) => modified > previous,
                None => true,
            };
            if !changed {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            let Ok(context) = client
                .context_for_file(&relative, None, 8000, "json-legacy")
                .await
            else {
                continue;
            };
            let chunks = context["chunks"].as_array().cloned().unwrap_or_default();
            if chunks.is_empty() {
                continue;
            }
            render_event(&relative, &chunks, mode)?;
        }
        known = current;
    }
}

fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, SystemTime>> {
    let mut files = BTreeMap::new();
    collect(root, &mut files)?;
    Ok(files)
}

fn collect(directory: &Path, files: &mut BTreeMap<PathBuf, SystemTime>) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("could not read {}", directory.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || matches!(name.as_ref(), "node_modules" | "target" | "dist" | "build")
        {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect(&path, files)?;
        } else if file_type.is_file() {
            files.insert(
                path,
                entry
                    .metadata()?
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH),
            );
        }
    }
    Ok(())
}

fn render_event(path: &str, chunks: &[serde_json::Value], mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&serde_json::json!({"path":path,"chunks":chunks}));
    }
    if mode == OutputMode::Quiet {
        println!("{path}");
        return Ok(());
    }
    println!("\n{path} — {} relevant chunk(s):", chunks.len());
    for chunk in chunks.iter().take(5) {
        println!(
            "  [{}] {} ({})",
            chunk["type"].as_str().unwrap_or("note"),
            chunk["title"].as_str().unwrap_or("Untitled"),
            chunk["matchReason"].as_str().unwrap_or("related")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_recurse_while_ignoring_generated_and_hidden_directories() {
        // Given source files plus hidden, dependency, and build outputs
        let directory = tempfile::tempdir().unwrap();
        for relative in [
            "src/main.rs",
            ".git/config",
            "node_modules/pkg/index.js",
            "target/debug/app",
        ] {
            let path = directory.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "content").unwrap();
        }
        // When the watch snapshot is collected
        let files = snapshot(directory.path()).unwrap();
        // Then only the project source file is watched
        assert_eq!(files.len(), 1);
        assert!(files.keys().next().unwrap().ends_with("src/main.rs"));
    }
}
