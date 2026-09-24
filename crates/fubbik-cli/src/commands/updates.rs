use anyhow::{Result, bail};

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(
    client: &Client,
    tag: Option<&str>,
    list_tags: bool,
    space_id: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    if list_tags {
        let value = client.list_update_tags(space_id).await?;
        if mode == OutputMode::Json {
            return output::json(&value["tags"]);
        }
        let tags = value["tags"].as_array().cloned().unwrap_or_default();
        if tags.is_empty() {
            println!("No update tags found.");
        }
        for item in tags {
            println!(
                "{}  {} update(s)",
                item["tag"].as_str().unwrap_or("?"),
                item["count"].as_i64().unwrap_or(0)
            );
        }
        return Ok(());
    }

    let tag = tag.ok_or_else(|| {
        anyhow::anyhow!("provide --tag <name> to list updates, or --tags to list all tags")
    })?;
    if tag.trim().is_empty() {
        bail!("update tag must not be empty");
    }
    let value = client.list_updates(tag, space_id).await?;
    if mode == OutputMode::Json {
        return output::json(&value["updates"]);
    }
    let updates = value["updates"].as_array().cloned().unwrap_or_default();
    if updates.is_empty() {
        println!("No updates found for tag {tag:?}.");
    }
    for update in updates {
        let action = if update["version"].as_i64() == Some(0) {
            "created"
        } else {
            "updated"
        };
        println!(
            "{}  {}  {action}",
            update["chunkId"].as_str().unwrap_or("?"),
            update["chunkTitle"].as_str().unwrap_or("Untitled")
        );
    }
    Ok(())
}
