use anyhow::Result;

use crate::GenerateCommand;
use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: GenerateCommand, mode: OutputMode) -> Result<()> {
    let (space, format) = match command {
        GenerateCommand::ClaudeMd { space } => (space, "claude"),
        GenerateCommand::AgentsMd { space } => (space, "agents"),
        GenerateCommand::Cursorrules { space } => (space, "cursor"),
    };
    let space = client
        .resolve_space(Some(&space))
        .await?
        .expect("an explicit space resolves to an id");
    let value = client.generate_instructions(&space, format).await?;
    if mode == OutputMode::Json {
        return output::json(&value);
    }
    let content = value["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("instruction response omitted content"))?;
    print!("{content}");
    if !content.ends_with('\n') {
        println!();
    }
    Ok(())
}
