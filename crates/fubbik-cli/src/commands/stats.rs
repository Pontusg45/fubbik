use anyhow::Result;

use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let stats = client.stats().await?;
    let human = format!(
        "chunks:      {}\nconnections: {}\ntags:        {}",
        stats.chunks, stats.connections, stats.tags
    );
    let quiet = vec![stats.chunks.to_string()];
    output::render(
        mode,
        CommandOutput {
            data: stats,
            human,
            quiet,
        },
    )
}
