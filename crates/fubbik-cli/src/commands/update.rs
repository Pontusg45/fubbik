use anyhow::Result;
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, OutputMode};

#[allow(clippy::too_many_arguments)]
pub async fn run(
    client: &Client,
    id: &str,
    title: Option<&str>,
    content: Option<&str>,
    chunk_type: Option<&str>,
    tags: Option<&[String]>,
    spaces: Option<&[String]>,
    mode: OutputMode,
) -> Result<()> {
    let chunk = client
        .update_chunk(id, title, content, chunk_type, tags, spaces)
        .await?;
    if !output::id_or_json(mode, &chunk.id, &chunk)? {
        println!(
            "{} {} {}",
            "updated".green(),
            chunk.id.dimmed(),
            chunk.title
        );
    }
    Ok(())
}
