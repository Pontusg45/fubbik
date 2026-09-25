use anyhow::{Result, bail};

use crate::PromptCommand;
use crate::client::{Chunk, Client};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: PromptCommand, mode: OutputMode) -> Result<()> {
    match command {
        PromptCommand::List => list(client, mode).await,
        PromptCommand::Get { name } => get(client, &name, mode).await,
        PromptCommand::Add {
            title,
            content,
            content_file,
        } => {
            let read_stdin = content_file.as_deref() == Some(std::path::Path::new("-"));
            let file = (!read_stdin).then_some(content_file.as_deref()).flatten();
            let content = super::input::read(content, file, read_stdin, true)?.unwrap_or_default();
            let chunk = client
                .create_chunk(&title, &content, "note", &["prompt".into()], &[])
                .await?;
            if !output::id_or_json(mode, &chunk.id, &chunk)? {
                println!("Created prompt: {title}");
            }
            Ok(())
        }
    }
}

async fn list(client: &Client, mode: OutputMode) -> Result<()> {
    let chunks = client.list_prompt_chunks(None).await?;
    match mode {
        OutputMode::Json => output::json(&chunks),
        OutputMode::Quiet => {
            for chunk in chunks {
                println!("{}", chunk.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            super::render_table(&chunks);
            Ok(())
        }
    }
}

async fn get(client: &Client, name: &str, mode: OutputMode) -> Result<()> {
    let chunk = match client.get_chunk(name).await {
        Ok(chunk) => Some(chunk),
        Err(_) => client
            .list_prompt_chunks(Some(name))
            .await?
            .into_iter()
            .next(),
    };
    let Some(chunk) = chunk else {
        bail!("prompt {name:?} not found");
    };
    render_prompt(&chunk, mode)
}

fn render_prompt(chunk: &Chunk, mode: OutputMode) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(chunk),
        OutputMode::Quiet => {
            println!("{}", chunk.id);
            Ok(())
        }
        OutputMode::Human => {
            println!("{}", chunk.content);
            Ok(())
        }
    }
}
