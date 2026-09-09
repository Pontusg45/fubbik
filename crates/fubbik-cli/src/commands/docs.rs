use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::DocsCommand;
use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: DocsCommand, mode: OutputMode) -> Result<()> {
    match command {
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
        DocsCommand::Sync { id, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let document = client.get_document(&id).await?;
            let source = document["sourcePath"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("document has no source path"))?;
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
