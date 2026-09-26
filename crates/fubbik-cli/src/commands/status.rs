use anyhow::Result;

use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let health = client.health().await?;
    let stats = client.stats().await?;
    let result = serde_json::json!({
        "server": client.base_url(),
        "health": health,
        "stats": stats,
    });
    let status = result["health"]["status"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let human = format!(
        "server:      {}\nstatus:      {}\nchunks:      {}\nconnections: {}\ntags:        {}",
        client.base_url(),
        status,
        result["stats"]["chunks"],
        result["stats"]["connections"],
        result["stats"]["tags"]
    );
    output::render(
        mode,
        CommandOutput {
            data: result,
            human,
            quiet: vec![status],
        },
    )
}
