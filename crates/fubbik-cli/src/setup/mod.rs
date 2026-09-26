use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::client::{Client, Space};
use crate::output::{self, OutputMode};

mod connections;
mod discover;
mod documents;
mod import;
mod interaction;
mod metadata;
mod model;
mod patterns;
mod preview;
mod util;

use connections::infer_connections;
pub use discover::discover;
use documents::scan_docs;
pub use import::import;
use interaction::{confirm, git_remote, project_name, prompt_name};
use metadata::scan_metadata;
pub use model::{DiscoveredChunk, DiscoveredConnection, Discovery, ImportError, ImportReport, Tip};
use patterns::scan_patterns;
use preview::render_preview;
use util::{
    chunk, collect_files, dependencies, find_files_containing, find_named_directories,
    markdown_links, markdown_title, read_json, relative_path, workspace_patterns,
};

const IGNORED: [&str; 10] = [
    "node_modules",
    ".git",
    ".turbo",
    "dist",
    "build",
    ".next",
    ".output",
    ".cache",
    "coverage",
    ".fubbik",
];

pub async fn run(
    client: &Client,
    dry_run: bool,
    yes: bool,
    force: bool,
    mode: OutputMode,
) -> Result<()> {
    client.health().await.with_context(|| {
        format!(
            "cannot connect to server at {}; is it running?",
            client.base_url()
        )
    })?;

    let directory = std::env::current_dir()?.canonicalize()?;
    let local_path = directory.to_string_lossy().into_owned();
    let remote = git_remote();
    let mut space = client
        .detect_space(Some(&local_path), remote.as_deref())
        .await?;
    if space.is_none() {
        let default_name = project_name(&directory, remote.as_deref());
        let name = if remote.is_none() && !yes && mode == OutputMode::Human {
            prompt_name(&default_name)?
        } else {
            default_name
        };
        space = Some(
            client
                .create_space(&name, Some(&local_path), remote.as_deref())
                .await?,
        );
    }
    let space = space.context("space detection or creation returned no space")?;

    if mode == OutputMode::Human {
        println!("\nDetected space: {}\n", space.name);
    }
    if !force {
        let page = client.list_chunk_page(Some(&space.id), 0).await?;
        if page.total > 0 && !yes {
            if mode != OutputMode::Human {
                bail!(
                    "space already has {} chunks; pass --yes or --force to continue",
                    page.total
                );
            }
            println!("This space already has {} chunks.", page.total);
            if !confirm("Continue and add more?")? {
                println!("Aborted. Use --force to re-import.");
                return Ok(());
            }
        }
    }

    if mode == OutputMode::Human {
        println!("Scanning project...");
    }
    let discovery = discover(&directory)?;
    if discovery.chunks.is_empty() {
        if mode == OutputMode::Json {
            return output::json(&serde_json::json!({
                "chunks": 0,
                "message": "No knowledge sources found"
            }));
        }
        println!("No knowledge sources found in this project.");
        return Ok(());
    }

    if dry_run {
        return render_preview(
            &discovery.chunks,
            &discovery.connections,
            &discovery.tags,
            mode,
        );
    }
    if !yes
        && mode == OutputMode::Human
        && !confirm(&format!(
            "Import {} chunks to fubbik?",
            discovery.chunks.len()
        ))?
    {
        println!("Aborted.");
        return Ok(());
    }

    let result = import(client, &space, &discovery).await;
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({
            "chunksCreated": result.chunks_created,
            "connectionsCreated": result.connections_created,
            "errors": result.errors,
            "tips": discovery.tips,
        })),
        OutputMode::Quiet => {
            println!("{}", result.chunks_created);
            Ok(())
        }
        OutputMode::Human => {
            println!("{} chunks created", result.chunks_created);
            if result.connections_created > 0 {
                println!("{} connections established", result.connections_created);
            }
            for error in result.errors.iter().take(5) {
                eprintln!("{}: {}", error.item, error.error);
            }
            if !discovery.tips.is_empty() {
                println!("\nYou might also want to add:");
                for tip in discovery.tips {
                    println!("  • {} — {}", tip.title, tip.detail);
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{discover, infer_connections, project_name};

    #[test]
    fn discovery_combines_docs_metadata_and_code_patterns() {
        // Given a documented TypeScript project with React components and Vitest tests
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("README.md"), "# Example\nUses React.").unwrap();
        std::fs::write(
            directory.path().join("package.json"),
            r#"{"dependencies":{"react":"19"},"devDependencies":{"vitest":"3"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(directory.path().join("src/components")).unwrap();
        std::fs::create_dir_all(directory.path().join("tests")).unwrap();
        std::fs::create_dir_all(directory.path().join(".github/workflows")).unwrap();
        std::fs::write(
            directory.path().join(".github/workflows/check.yml"),
            "name: check",
        )
        .unwrap();

        // When the project is discovered
        let discovery = discover(directory.path()).unwrap();

        // Then all three tiers and their useful metadata are present
        assert!(discovery.chunks.iter().any(|chunk| chunk.tier == 1));
        assert!(
            discovery
                .chunks
                .iter()
                .any(|chunk| chunk.tier == 2 && chunk.title == "Tech Stack")
        );
        assert!(
            discovery
                .chunks
                .iter()
                .any(|chunk| chunk.title == "CI/CD Configuration")
        );
        assert!(
            discovery
                .chunks
                .iter()
                .any(|chunk| chunk.tier == 3 && chunk.title == "Testing Conventions")
        );
        assert!(discovery.tags.contains(&"react".to_owned()));
        assert!(discovery.tips.is_empty());
    }

    #[test]
    fn connection_inference_links_markdown_references_and_architecture() {
        // Given discovered docs that link to one another and a monorepo tech stack
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("README.md"),
            "# Home\n[Guide](docs/guide.md)",
        )
        .unwrap();
        std::fs::create_dir(directory.path().join("docs")).unwrap();
        std::fs::write(directory.path().join("docs/guide.md"), "# Guide").unwrap();
        std::fs::write(
            directory.path().join("package.json"),
            r#"{"workspaces":["apps/*"],"dependencies":{"react":"19"}}"#,
        )
        .unwrap();
        let discovery = discover(directory.path()).unwrap();

        // When connections are inferred
        let connections = infer_connections(&discovery.chunks);

        // Then document references and structural relationships are retained
        assert!(
            connections
                .iter()
                .any(|connection| connection.relation == "references")
        );
        assert!(
            connections
                .iter()
                .any(|connection| connection.relation == "part_of")
        );
    }

    #[test]
    fn project_names_support_ssh_and_https_git_remotes() {
        // Given common Git remote URL formats
        let directory = std::path::Path::new("/tmp/fallback");

        // When project names are derived
        let ssh = project_name(directory, Some("git@github.com:team/fubbik.git"));
        let https = project_name(directory, Some("https://github.com/team/fubbik.git"));

        // Then both formats produce the repository name
        assert_eq!(ssh, "fubbik");
        assert_eq!(https, "fubbik");
    }

    #[test]
    fn documentation_only_discovery_ignores_generated_directories() {
        // Given
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("README.md"), "# Project").unwrap();
        for ignored in ["node_modules", "dist", ".git"] {
            let path = directory.path().join(ignored);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("SECRET.md"), "# Ignore me").unwrap();
        }

        // When
        let discovery = discover(directory.path()).unwrap();

        // Then
        assert_eq!(discovery.chunks.len(), 1);
        assert_eq!(discovery.chunks[0].source, "README.md");
    }

    #[test]
    fn malformed_metadata_does_not_hide_valid_documents() {
        // Given
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("README.md"), "# Project").unwrap();
        std::fs::write(directory.path().join("package.json"), "{broken").unwrap();

        // When
        let discovery = discover(directory.path()).unwrap();

        // Then
        assert_eq!(discovery.chunks.len(), 1);
        assert_eq!(discovery.chunks[0].title, "Project README");
    }

    #[test]
    fn discovery_is_deterministic_and_deduplicates_connections() {
        // Given
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("docs")).unwrap();
        std::fs::write(directory.path().join("docs/b.md"), "# B").unwrap();
        std::fs::write(directory.path().join("docs/a.md"), "# A").unwrap();
        std::fs::write(
            directory.path().join("README.md"),
            "# Home\n[A](docs/a.md) and [A again](docs/a.md)",
        )
        .unwrap();

        // When
        let first = discover(directory.path()).unwrap();
        let second = discover(directory.path()).unwrap();

        // Then
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        assert_eq!(first.connections.len(), 1);
        assert_eq!(first.connections[0].relation, "references");
    }
}
