use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let health = client.health().await?;
    let status = health.status.clone();
    let rendered = if status == "ok" {
        status.green().to_string()
    } else {
        status.red().to_string()
    };
    let human = format!(
        "status:   {rendered}\ndatabase: {}\nAGE:      {}",
        health.db,
        if health.age_available {
            "available"
        } else {
            "unavailable"
        }
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
