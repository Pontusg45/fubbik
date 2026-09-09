use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn link(
    client: &Client,
    source: &str,
    target: &str,
    relation: &str,
    mode: OutputMode,
) -> Result<()> {
    let connection = client.create_connection(source, target, relation).await?;
    if !output::id_or_json(mode, &connection.id, &connection)? {
        println!(
            "{} {} → {} ({})",
            "linked".green(),
            source,
            target,
            relation
        );
    }
    Ok(())
}

pub async fn unlink(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let result = client.delete_connection(id).await?;
    match mode {
        OutputMode::Json => output::json(&result),
        OutputMode::Quiet => {
            println!("{id}");
            Ok(())
        }
        OutputMode::Human => {
            println!("{} {}", "unlinked".green(), id.dimmed());
            Ok(())
        }
    }
}
