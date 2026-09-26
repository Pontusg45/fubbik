use anyhow::Result;

use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let health = client.health().await?;
    let stats = client.stats().await?;
    let status = health.status.clone();
    let human = format!(
        "server:      {}\nstatus:      {}\nchunks:      {}\nconnections: {}\ntags:        {}",
        client.base_url(),
        status,
        stats.chunks,
        stats.connections,
        stats.tags
    );
    let result = serde_json::json!({
        "server": client.base_url(),
        "health": health,
        "stats": stats,
    });
    output::render(
        mode,
        CommandOutput {
            data: result,
            human,
            quiet: vec![status],
        },
    )
}
