use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, id: &str, mode: OutputMode) -> Result<()> {
    let chunk = client.get_chunk(id).await?;
    if output::id_or_json(mode, &chunk.id, &chunk)? {
        return Ok(());
    }
    println!("{}", chunk.title.bold());
    println!("{} {}", "type:".dimmed(), chunk.chunk_type);
    println!("{} {}", "id:".dimmed(), chunk.id);
    println!();
    println!("{}", chunk.content);
    Ok(())
}
