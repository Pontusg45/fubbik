use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;

pub async fn run(client: &Client, id: &str) -> Result<()> {
    let chunk = client.get_chunk(id).await?;
    println!("{}", chunk.title.bold());
    println!("{} {}", "type:".dimmed(), chunk.chunk_type);
    println!("{} {}", "id:".dimmed(), chunk.id);
    println!();
    println!("{}", chunk.content);
    Ok(())
}
