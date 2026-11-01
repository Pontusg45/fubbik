use anyhow::Result;

use crate::client::Client;
use crate::output::OutputMode;

pub async fn run(
    client: &Client,
    query: &str,
    limit: u32,
    offset: u32,
    all: bool,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = client
        .query_chunks(None, Some(query), space_id.as_deref(), limit, offset, all)
        .await?;
    super::render_chunks(mode, chunks)
}
