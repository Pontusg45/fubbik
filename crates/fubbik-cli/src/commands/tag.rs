use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;

use crate::TagCommand;
use crate::client::{Client, Tag};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: TagCommand, mode: OutputMode) -> Result<()> {
    match command {
        TagCommand::List => render_list(client.list_tags().await?, mode),
        TagCommand::Add { name, tag_type } => render_one(
            &client.create_tag(&name, tag_type.as_deref()).await?,
            "created",
            mode,
        ),
        TagCommand::Rename { id, name } => {
            render_one(&client.update_tag(&id, &name).await?, "updated", mode)
        }
        TagCommand::Remove { id } => {
            let result = client.delete_tag(&id).await?;
            match mode {
                OutputMode::Json => output::json(&result),
                OutputMode::Quiet => {
                    println!("{id}");
                    Ok(())
                }
                OutputMode::Human => {
                    println!("{} {}", "removed".green(), id.dimmed());
                    Ok(())
                }
            }
        }
        TagCommand::Normalize { confirm, space } => {
            super::tag_normalize::run(client, confirm, space.as_deref(), mode).await
        }
    }
}

fn render_list(tags: Vec<Tag>, mode: OutputMode) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(&tags),
        OutputMode::Quiet => {
            for tag in tags {
                println!("{}", tag.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            if tags.is_empty() {
                println!("No tags found.");
                return Ok(());
            }
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec!["ID", "Chunks", "Name"]);
            for tag in tags {
                table.add_row(vec![tag.id, tag.chunk_count.to_string(), tag.name]);
            }
            println!("{table}");
            Ok(())
        }
    }
}

fn render_one(tag: &Tag, verb: &str, mode: OutputMode) -> Result<()> {
    if !output::id_or_json(mode, &tag.id, tag)? {
        println!("{} {} {}", verb.green(), tag.id.dimmed(), tag.name);
    }
    Ok(())
}
