use anyhow::Result;
use owo_colors::OwoColorize;

use crate::TaskCommand;
use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

pub async fn run(client: &Client, command: TaskCommand, mode: OutputMode) -> Result<()> {
    match command {
        TaskCommand::Add {
            title,
            description,
            space,
            priority,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let plan = client
                .create_quick_task(&title, description.as_deref(), space.as_deref(), &priority)
                .await?;
            let human = format!("{} {} {}", "created".green(), plan.id.dimmed(), plan.title);
            let quiet = vec![plan.id.clone()];
            output::render(
                mode,
                CommandOutput {
                    data: plan,
                    human,
                    quiet,
                },
            )?;
        }
        TaskCommand::List { status } => {
            let plans = client.list_quick_tasks(&status).await?;
            let human = if plans.is_empty() {
                "No tasks found.".into()
            } else {
                plans
                    .iter()
                    .map(|plan| {
                        let next = plan.next_action.as_deref().unwrap_or(&plan.title);
                        let priority = plan.metadata["priority"].as_str().unwrap_or("normal");
                        format!(
                            "  {:<7} {:<12} {} {}",
                            priority,
                            plan.status,
                            next,
                            plan.id.dimmed()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let quiet = plans.iter().map(|plan| plan.id.clone()).collect();
            output::render(
                mode,
                CommandOutput {
                    data: plans,
                    human,
                    quiet,
                },
            )?;
        }
        TaskCommand::Claim { id } => {
            let detail = client.claim_quick_task(&id).await?;
            let human = format!("{} {}", "claimed".green(), detail.plan.title);
            output::render(
                mode,
                CommandOutput {
                    data: detail,
                    human,
                    quiet: vec![id],
                },
            )?;
        }
        TaskCommand::Done { id, note } => {
            let plan = client.complete_quick_task(&id, note.as_deref()).await?;
            let human = format!("{} {}", "completed".green(), plan.title);
            let quiet = vec![plan.id.clone()];
            output::render(
                mode,
                CommandOutput {
                    data: plan,
                    human,
                    quiet,
                },
            )?;
        }
    }
    Ok(())
}
