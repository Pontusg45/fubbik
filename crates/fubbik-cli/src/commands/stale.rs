use anyhow::Result;

use crate::StaleCommand;
use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

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
            let human = if flags.is_empty() {
                "No stale chunks found.".into()
            } else {
                flags
                    .iter()
                    .map(|flag| {
                        format!(
                            "{}  {}",
                            flag["chunkId"].as_str().unwrap_or("?"),
                            flag["reason"].as_str().unwrap_or("stale")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let quiet = flags
                .iter()
                .filter_map(|flag| flag["chunkId"].as_str().map(str::to_string))
                .collect();
            output::render(
                mode,
                CommandOutput {
                    data: flags,
                    human,
                    quiet,
                },
            )
        }
        StaleCommand::Dismiss { id } => {
            let result = client.dismiss_stale(&id).await?;
            output::render(
                mode,
                CommandOutput {
                    data: result,
                    human: format!("Dismissed staleness for {id}"),
                    quiet: vec![id],
                },
            )
        }
    }
}
