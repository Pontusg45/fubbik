use anyhow::{Result, bail};

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, id: Option<&str>, all: bool, mode: OutputMode) -> Result<()> {
    let result = match (id, all) {
        (Some(id), false) => client.enrich_chunk(id).await?,
        (None, true) => client.enrich_all().await?,
        (Some(_), true) => bail!("provide a chunk id or --all, not both"),
        (None, false) => bail!("provide a chunk id or --all"),
    };
    match mode {
        OutputMode::Json => output::json(&result),
        OutputMode::Quiet => {
            if let Some(id) = id {
                println!("{id}");
            } else {
                println!(
                    "{}",
                    result.get("enriched").unwrap_or(&serde_json::Value::Null)
                );
            }
            Ok(())
        }
        OutputMode::Human => {
            if let Some(id) = id {
                println!("Enriched chunk {id}");
            } else {
                println!("Enriched {} chunk(s)", result["enriched"]);
            }
            Ok(())
        }
    }
}
