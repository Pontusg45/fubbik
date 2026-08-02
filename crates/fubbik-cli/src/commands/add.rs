use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;

pub async fn run(client: &Client, title: &str, content: &str, chunk_type: &str) -> Result<()> {
    let chunk = client.create_chunk(title, content, chunk_type).await?;
    println!(
        "{} {} {}",
        "created".green(),
        chunk.id.dimmed(),
        chunk.title
    );
    Ok(())
}
