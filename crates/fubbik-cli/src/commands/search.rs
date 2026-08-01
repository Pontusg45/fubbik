use anyhow::Result;

use crate::client::Client;

pub async fn run(client: &Client, query: &str, limit: u32) -> Result<()> {
    let chunks = client.list_chunks(None, Some(query), limit).await?;
    super::render_table(&chunks);
    Ok(())
}
