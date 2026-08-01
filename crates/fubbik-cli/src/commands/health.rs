use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;

pub async fn run(client: &Client) -> Result<()> {
    let health = client.health().await?;
    let status = health["status"].as_str().unwrap_or("unknown");
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
