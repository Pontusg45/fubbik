use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, mode: OutputMode) -> Result<()> {
    let health = client.health().await?;
    let status = health["status"].as_str().unwrap_or("unknown");
    match mode {
        OutputMode::Json => return output::json(&health),
        OutputMode::Quiet => {
            println!("{status}");
            return Ok(());
        }
        OutputMode::Human => {}
    }
    let rendered = if status == "ok" {
        status.green().to_string()
    } else {
        status.red().to_string()
    };
    println!("status:   {rendered}");
    println!("database: {}", health["database"]);
    println!("version:  {}", health["version"].as_str().unwrap_or("?"));
    Ok(())
}
