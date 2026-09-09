use std::process::Command;

use anyhow::{Result, bail};
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;

use crate::SpaceCommand;
use crate::client::{Client, Space};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: SpaceCommand, mode: OutputMode) -> Result<()> {
    match command {
        SpaceCommand::List => render_list(client.list_spaces().await?, mode),
        SpaceCommand::Add { name } => {
            let cwd = std::env::current_dir()?;
            let local_path = cwd.to_string_lossy();
            let remote = git_remote();
            let space = client
                .create_space(&name, Some(&local_path), remote.as_deref())
                .await?;
            render_one(&space, "created", mode)
        }
        SpaceCommand::Remove { name, force } => {
            let spaces = client.list_spaces().await?;
            let space = spaces
                .into_iter()
                .find(|space| space.name == name)
                .ok_or_else(|| anyhow::anyhow!("space {name:?} not found"))?;
            if !force {
                bail!("refusing to remove a space without --force");
            }
            let result = client.delete_space(&space.id).await?;
            match mode {
                OutputMode::Json => output::json(&result),
                OutputMode::Quiet => {
                    println!("{}", space.id);
                    Ok(())
                }
                OutputMode::Human => {
                    println!("{} {}", "removed".green(), space.name);
                    Ok(())
                }
            }
        }
        SpaceCommand::Current => {
            let cwd = std::env::current_dir()?;
            let local_path = cwd.to_string_lossy();
            let remote = git_remote();
            let space = client
                .detect_space(Some(&local_path), remote.as_deref())
                .await?
                .ok_or_else(|| anyhow::anyhow!("no space detected for this directory"))?;
            render_one(&space, "current", mode)
        }
    }
}

fn git_remote() -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn render_list(spaces: Vec<Space>, mode: OutputMode) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(&spaces),
        OutputMode::Quiet => {
            for space in spaces {
                println!("{}", space.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            if spaces.is_empty() {
                println!("No spaces found.");
                return Ok(());
            }
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec!["ID", "Kind", "Name"]);
            for space in spaces {
                table.add_row(vec![space.id, space.kind, space.name]);
            }
            println!("{table}");
            Ok(())
        }
    }
}

fn render_one(space: &Space, verb: &str, mode: OutputMode) -> Result<()> {
    if !output::id_or_json(mode, &space.id, space)? {
        println!("{} {} {}", verb.green(), space.id.dimmed(), space.name);
    }
    Ok(())
}
