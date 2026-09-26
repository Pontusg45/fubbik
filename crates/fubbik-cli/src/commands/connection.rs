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
    output::render(
        mode,
        output::CommandOutput {
            quiet: vec![connection.id.clone()],
            data: connection,
            human: format!(
                "{} {} → {} ({})",
                "linked".green(),
                source,
                target,
                relation
            ),
        },
    )
}

pub async fn unlink(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let result = client.delete_connection(id).await?;
    output::render(
        mode,
        output::CommandOutput {
            data: result,
            human: format!("{} {}", "unlinked".green(), id.dimmed()),
            quiet: vec![id.into()],
        },
    )
}
