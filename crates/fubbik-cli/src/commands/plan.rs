use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;

use crate::PlanCommand;
use crate::client::{Client, Plan};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: PlanCommand, mode: OutputMode) -> Result<()> {
    match run_command(client, command, mode).await {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(crate::capabilities::explain_not_found(client.base_url(), "plans", error).await)
        }
    }
}

async fn run_command(client: &Client, command: PlanCommand, mode: OutputMode) -> Result<()> {
    match command {
        PlanCommand::List { status, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let plans = client
                .list_plans(status.as_deref(), space.as_deref())
                .await?;
            output::render(
                mode,
                output::CommandOutput {
                    human: render_table(&plans),
                    quiet: plans.iter().map(|plan| plan.id.clone()).collect(),
                    data: plans,
                },
            )?;
        }
        PlanCommand::Show { id } => {
            let detail = client.get_plan(&id).await?;
            let mut lines = vec![
                format!("{} {}", detail.plan.title.bold(), detail.plan.id.dimmed()),
                format!("{} {}", "status:".dimmed(), detail.plan.status),
            ];
            if let Some(description) = &detail.plan.description {
                lines.push(format!("\n{description}"));
            }
            if !detail.tasks.is_empty() {
                lines.push(format!("\n{}", "Tasks".bold()));
                for task in &detail.tasks {
                    lines.push(format!(
                        "  {:<12} {} {}",
                        task.status,
                        task.title,
                        task.id.dimmed()
                    ));
                }
            }
            output::render(
                mode,
                output::CommandOutput {
                    human: lines.join("\n"),
                    quiet: vec![detail.plan.id.clone()],
                    data: detail,
                },
            )?;
        }
        PlanCommand::Create {
            title,
            description,
            space,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let plan = client
                .create_plan(&title, description.as_deref(), space.as_deref(), &[])
                .await?;
            output::render(
                mode,
                output::CommandOutput {
                    human: format!("{} {} {}", "created".green(), plan.id.dimmed(), plan.title),
                    quiet: vec![plan.id.clone()],
                    data: plan,
                },
            )?;
        }
        PlanCommand::Status { id, status } => {
            let plan = client.update_plan_status(&id, &status).await?;
            output::render(
                mode,
                output::CommandOutput {
                    human: format!(
                        "{} {} → {}",
                        "updated".green(),
                        plan.id.dimmed(),
                        plan.status
                    ),
                    quiet: vec![plan.id.clone()],
                    data: plan,
                },
            )?;
        }
        PlanCommand::AddTask {
            plan_id,
            title,
            description,
        } => {
            let task = client
                .create_plan_task(&plan_id, &title, description.as_deref())
                .await?;
            output::render(
                mode,
                output::CommandOutput {
                    human: format!("{} {} {}", "created".green(), task.id.dimmed(), task.title),
                    quiet: vec![task.id.clone()],
                    data: task,
                },
            )?;
        }
        PlanCommand::TaskDone { plan_id, task_id } => {
            let task = client
                .update_task_status(&plan_id, &task_id, "done")
                .await?;
            output::render(
                mode,
                output::CommandOutput {
                    human: format!("{} {}", "completed".green(), task.id.dimmed()),
                    quiet: vec![task.id.clone()],
                    data: task,
                },
            )?;
        }
        PlanCommand::LinkRequirement {
            plan_id,
            requirement_id,
        } => {
            let linked = client
                .link_plan_requirement(&plan_id, &requirement_id)
                .await?;
            output::render(
                mode,
                output::CommandOutput {
                    human: format!(
                        "{} {} → {}",
                        "linked".green(),
                        requirement_id.dimmed(),
                        plan_id
                    ),
                    quiet: vec![requirement_id],
                    data: linked,
                },
            )?;
        }
    }
    Ok(())
}

fn render_table(plans: &[Plan]) -> String {
    if plans.is_empty() {
        return "No plans found.".into();
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["ID", "Status", "Progress", "Title", "Next action"]);
    for plan in plans {
        table.add_row(vec![
            plan.id.clone(),
            plan.status.clone(),
            format!("{}/{}", plan.task_done, plan.task_total),
            plan.title.clone(),
            plan.next_action.clone().unwrap_or_default(),
        ]);
    }
    table.to_string()
}
