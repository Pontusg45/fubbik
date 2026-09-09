use anyhow::{Result, bail};
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;

use crate::RequirementCommand;
use crate::client::{Client, Requirement};
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: RequirementCommand, mode: OutputMode) -> Result<()> {
    match command {
        RequirementCommand::List {
            status,
            space,
            priority,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            render_list(
                client
                    .list_requirements(space.as_deref(), status.as_deref(), priority.as_deref())
                    .await?,
                mode,
            )
        }
        RequirementCommand::Add {
            title,
            step,
            space,
            priority,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let steps = step
                .iter()
                .map(|raw| parse_step(raw))
                .collect::<Result<Vec<_>>>()?;
            let requirement = client
                .create_requirement(&title, &steps, space.as_deref(), priority.as_deref())
                .await?;
            render_one(&requirement, "created", mode)
        }
        RequirementCommand::Status { id, status } => {
            let requirement = client.update_requirement_status(&id, &status).await?;
            render_one(&requirement, "updated", mode)
        }
        RequirementCommand::Export { format, space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let text = client
                .export_requirements(&format, space.as_deref())
                .await?;
            if mode == OutputMode::Json {
                output::json(&serde_json::json!({"format": format, "content": text}))
            } else {
                print!("{text}");
                Ok(())
            }
        }
        RequirementCommand::Verify { space } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let requirements = client
                .list_requirements(space.as_deref(), None, None)
                .await?;
            let issues: Vec<_> = requirements
                .iter()
                .filter(|item| item.status != "passing")
                .collect();
            let result = serde_json::json!({"total": requirements.len(), "issues": issues.len()});
            match mode {
                OutputMode::Json => output::json(&result),
                OutputMode::Quiet => {
                    println!("{}", issues.len());
                    Ok(())
                }
                OutputMode::Human => {
                    println!(
                        "{} requirement(s), {} not passing",
                        requirements.len(),
                        issues.len()
                    );
                    Ok(())
                }
            }
        }
    }
}

fn parse_step(raw: &str) -> Result<serde_json::Value> {
    let Some((keyword, text)) = raw.split_once(':') else {
        bail!("invalid step {raw:?}; expected 'keyword: text'");
    };
    let keyword = keyword.trim().to_ascii_lowercase();
    let text = text.trim();
    if keyword.is_empty() || text.is_empty() {
        bail!("invalid step {raw:?}; keyword and text are required");
    }
    Ok(serde_json::json!({"keyword": keyword, "text": text}))
}

fn render_list(requirements: Vec<Requirement>, mode: OutputMode) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(&requirements),
        OutputMode::Quiet => {
            for requirement in requirements {
                println!("{}", requirement.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            if requirements.is_empty() {
                println!("No requirements found.");
                return Ok(());
            }
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec!["ID", "Status", "Priority", "Title"]);
            for requirement in requirements {
                table.add_row(vec![
                    requirement.id,
                    requirement.status,
                    requirement.priority.unwrap_or_default(),
                    requirement.title,
                ]);
            }
            println!("{table}");
            Ok(())
        }
    }
}

fn render_one(requirement: &Requirement, verb: &str, mode: OutputMode) -> Result<()> {
    if !output::id_or_json(mode, &requirement.id, requirement)? {
        println!(
            "{} {} {}",
            verb.green(),
            requirement.id.dimmed(),
            requirement.title
        );
    }
    Ok(())
}
