use anyhow::Result;

use crate::client::Client;

pub async fn run(client: &Client, chunk_type: Option<&str>, limit: u32) -> Result<()> {
    let chunks = client.list_chunks(chunk_type, None, limit).await?;
    super::render_table(&chunks);
    Ok(())
}
