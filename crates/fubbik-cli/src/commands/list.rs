use anyhow::Result;

use crate::client::Client;
use crate::output::OutputMode;

pub async fn run(
    client: &Client,
    chunk_type: Option<&str>,
    limit: u32,
    mode: OutputMode,
) -> Result<()> {
    let chunks = client.list_chunks(chunk_type, None, limit).await?;
    super::render_chunks(mode, chunks)
}
