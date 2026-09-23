use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;

use crate::PlanCommand;
use crate::client::{Client, Plan};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: PlanCommand, mode: OutputMode) -> Result<()> {
    match command {
        PlanCommand::List { status, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let plans = client
                .list_plans(status.as_deref(), space.as_deref())
                .await?;
            if mode == OutputMode::Json {
                return output::json(&plans);
            }
            if mode == OutputMode::Quiet {
                for plan in plans {
                    println!("{}", plan.id);
                }
                return Ok(());
            }
            render_table(&plans);
        }
        PlanCommand::Show { id } => {
            let detail = client.get_plan(&id).await?;
            if output::id_or_json(mode, &detail.plan.id, &detail)? {
                return Ok(());
            }
            println!("{} {}", detail.plan.title.bold(), detail.plan.id.dimmed());
            println!("{} {}", "status:".dimmed(), detail.plan.status);
            if let Some(description) = detail.plan.description {
                println!("\n{description}");
            }
            if !detail.tasks.is_empty() {
                println!("\n{}", "Tasks".bold());
                for task in detail.tasks {
                    println!("  {:<12} {} {}", task.status, task.title, task.id.dimmed());
                }
            }
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
            if !output::id_or_json(mode, &plan.id, &plan)? {
                println!("{} {} {}", "created".green(), plan.id.dimmed(), plan.title);
            }
        }
        PlanCommand::Status { id, status } => {
            let plan = client.update_plan_status(&id, &status).await?;
            if !output::id_or_json(mode, &plan.id, &plan)? {
                println!(
                    "{} {} → {}",
                    "updated".green(),
                    plan.id.dimmed(),
                    plan.status
                );
            }
        }
        PlanCommand::AddTask {
            plan_id,
            title,
            description,
        } => {
            let task = client
                .create_plan_task(&plan_id, &title, description.as_deref())
                .await?;
            if !output::id_or_json(mode, &task.id, &task)? {
                println!("{} {} {}", "created".green(), task.id.dimmed(), task.title);
            }
        }
        PlanCommand::TaskDone { plan_id, task_id } => {
            let task = client
                .update_task_status(&plan_id, &task_id, "done")
                .await?;
            if !output::id_or_json(mode, &task.id, &task)? {
                println!("{} {}", "completed".green(), task.id.dimmed());
            }
        }
        PlanCommand::LinkRequirement {
            plan_id,
            requirement_id,
        } => {
            let linked = client
                .link_plan_requirement(&plan_id, &requirement_id)
                .await?;
            match mode {
                OutputMode::Json => output::json(&linked)?,
                OutputMode::Quiet => println!("{requirement_id}"),
                OutputMode::Human => println!(
                    "{} {} → {}",
                    "linked".green(),
                    requirement_id.dimmed(),
                    plan_id
                ),
            }
        }
    }
    Ok(())
}

fn render_table(plans: &[Plan]) {
    if plans.is_empty() {
        println!("No plans found.");
        return;
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
    println!("{table}");
}
