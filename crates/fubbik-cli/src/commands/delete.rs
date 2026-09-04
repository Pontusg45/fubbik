use std::io::{IsTerminal, Write};

use anyhow::{Result, bail};
use owo_colors::OwoColorize;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, id: &str, yes: bool, mode: OutputMode) -> Result<()> {
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to delete non-interactively; pass --yes");
        }
        print!("Delete chunk {id}? [y/N] ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            bail!("delete cancelled");
        }
    }
    let result = client.delete_chunk(id).await?;
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({ "id": id, "result": result }))?,
        OutputMode::Quiet => println!("{id}"),
        OutputMode::Human => println!("{} {}", "deleted".green(), id.dimmed()),
    }
    Ok(())
}
