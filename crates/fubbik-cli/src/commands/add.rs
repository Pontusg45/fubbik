use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(
    client: &Client,
    title: &str,
    content: &str,
    chunk_type: &str,
    mode: OutputMode,
) -> Result<()> {
    let chunk = client.create_chunk(title, content, chunk_type).await?;
    if !output::id_or_json(mode, &chunk.id, &chunk)? {
        println!(
            "{} {} {}",
            "created".green(),
            chunk.id.dimmed(),
            chunk.title
        );
    }
    Ok(())
}
