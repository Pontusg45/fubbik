use anyhow::Result;

use crate::client::Client;
use crate::output::OutputMode;

pub async fn run(client: &Client, query: &str, limit: u32, mode: OutputMode) -> Result<()> {
    let chunks = client.list_chunks(None, Some(query), limit).await?;
    super::render_chunks(mode, chunks)
}
