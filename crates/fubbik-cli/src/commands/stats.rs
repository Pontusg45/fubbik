use anyhow::Result;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let stats = client.stats().await?;
    match mode {
        OutputMode::Json => output::json(&stats),
        OutputMode::Quiet => {
            println!("{}", stats["chunks"]);
            Ok(())
        }
        OutputMode::Human => {
            println!("chunks:      {}", stats["chunks"]);
            println!("connections: {}", stats["connections"]);
            println!("tags:        {}", stats["tags"]);
            Ok(())
        }
    }
}
