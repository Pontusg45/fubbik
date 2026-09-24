use anyhow::Result;
use owo_colors::OwoColorize;

use crate::TaskCommand;
use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: TaskCommand, mode: OutputMode) -> Result<()> {
    match command {
        TaskCommand::Add {
            title,
            description,
            space,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let plan = client
                .create_quick_task(&title, description.as_deref(), space.as_deref())
                .await?;
            if !output::id_or_json(mode, &plan.id, &plan)? {
                println!("{} {} {}", "created".green(), plan.id.dimmed(), plan.title);
            }
        }
        TaskCommand::List { status } => {
            let plans = client.list_plans(Some(&status), None).await?;
            if mode == OutputMode::Json {
                return output::json(&plans);
            }
            if mode == OutputMode::Quiet {
                for plan in plans {
                    println!("{}", plan.id);
                }
                return Ok(());
            }
            if plans.is_empty() {
                println!("No tasks found.");
            } else {
                for plan in plans {
                    let next = plan.next_action.as_deref().unwrap_or(&plan.title);
                    println!("  {:<12} {} {}", plan.status, next, plan.id.dimmed());
                }
            }
        }
        TaskCommand::Claim { id } => {
            let task = client.set_quick_task_status(&id, "in_progress").await?;
            match mode {
                OutputMode::Json => output::json(&task)?,
                OutputMode::Quiet => println!("{id}"),
                OutputMode::Human => println!("{} {}", "claimed".green(), task.title),
            }
        }
        TaskCommand::Done { id, note } => {
            let (task, plan) = client.complete_quick_task(&id, note.as_deref()).await?;
            if mode == OutputMode::Json {
                return output::json(&serde_json::json!({ "task": task, "plan": plan }));
            }
            if mode == OutputMode::Quiet {
                println!("{}", plan.id);
            } else {
                println!("{} {}", "completed".green(), task.title);
            }
        }
    }
    Ok(())
}
