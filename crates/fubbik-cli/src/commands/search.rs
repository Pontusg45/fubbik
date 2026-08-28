use anyhow::Result;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, query: &str, limit: u32, mode: OutputMode) -> Result<()> {
    let chunks = client.list_chunks(None, Some(query), limit).await?;
    match mode {
        OutputMode::Human => super::render_table(&chunks),
        OutputMode::Json => output::json(&chunks)?,
        OutputMode::Quiet => {
            for chunk in chunks {
                println!("{}", chunk.id);
            }
        }
    }
    Ok(())
}
