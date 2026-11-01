use anyhow::Result;

use crate::client::Client;
use crate::output::OutputMode;

pub async fn run(
    client: &Client,
    chunk_type: Option<&str>,
    limit: u32,
    offset: u32,
    all: bool,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = client
        .query_chunks(chunk_type, None, space_id.as_deref(), limit, offset, all)
        .await?;
    super::render_chunks(mode, chunks)
}
