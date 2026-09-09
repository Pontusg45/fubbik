use anyhow::Result;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let health = client.health().await?;
    let stats = client.stats().await?;
    let result = serde_json::json!({
        "server": client.base_url(),
        "health": health,
        "stats": stats,
    });
    match mode {
        OutputMode::Json => output::json(&result),
        OutputMode::Quiet => {
            println!(
                "{}",
                result["health"]["status"].as_str().unwrap_or("unknown")
            );
            Ok(())
        }
        OutputMode::Human => {
            println!("server:      {}", client.base_url());
            println!(
                "status:      {}",
                result["health"]["status"].as_str().unwrap_or("unknown")
            );
            println!("chunks:      {}", result["stats"]["chunks"]);
            println!("connections: {}", result["stats"]["connections"]);
            println!("tags:        {}", result["stats"]["tags"]);
            Ok(())
        }
    }
}
