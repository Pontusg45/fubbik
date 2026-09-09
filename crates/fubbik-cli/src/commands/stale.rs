use anyhow::Result;

use crate::StaleCommand;
use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: StaleCommand, mode: OutputMode) -> Result<()> {
    match command {
        StaleCommand::List {
            reason,
            space,
            limit,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let flags = client
                .list_stale(reason.as_deref(), space.as_deref(), limit)
                .await?;
            match mode {
                OutputMode::Json => output::json(&flags),
                OutputMode::Quiet => {
                    for flag in flags {
                        if let Some(id) = flag["chunkId"].as_str() {
                            println!("{id}");
                        }
                    }
                    Ok(())
                }
                OutputMode::Human => {
                    if flags.is_empty() {
                        println!("No stale chunks found.");
                    }
                    for flag in flags {
                        println!(
                            "{}  {}",
                            flag["chunkId"].as_str().unwrap_or("?"),
                            flag["reason"].as_str().unwrap_or("stale")
                        );
                    }
                    Ok(())
                }
            }
        }
        StaleCommand::Dismiss { id } => {
            let result = client.dismiss_stale(&id).await?;
            if !output::id_or_json(mode, &id, &result)? {
                println!("Dismissed staleness for {id}");
            }
            Ok(())
        }
    }
}
