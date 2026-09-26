use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let health = client.health().await?;
    let status = health["status"].as_str().unwrap_or("unknown").to_string();
    let rendered = if status == "ok" {
        status.green().to_string()
    } else {
        status.red().to_string()
    };
    let human = format!(
        "status:   {rendered}\ndatabase: {}\nversion:  {}",
        health["database"],
        health["version"].as_str().unwrap_or("?")
    );
    output::render(
        mode,
        CommandOutput {
            data: health,
            human,
            quiet: vec![status],
        },
    )
}
