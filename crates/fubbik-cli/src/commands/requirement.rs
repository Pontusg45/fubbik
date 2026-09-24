use std::path::Path;

use anyhow::{Context, Result, bail};
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
                .create_requirement(&title, None, &steps, space.as_deref(), priority.as_deref())
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
        RequirementCommand::Import {
            file,
            space,
            priority,
        } => {
            let space = client.resolve_space(space.as_deref()).await?;
            let content = std::fs::read_to_string(&file)
                .with_context(|| format!("could not read file: {}", file.display()))?;
            let parsed = parse_gherkin(&content);
            if parsed.is_empty() {
                bail!("no scenarios found in {}", file.display());
            }

            let mut created = Vec::new();
            let mut failed = Vec::new();
            for requirement in parsed {
                match client
                    .create_requirement(
                        &requirement.title,
                        Some(&requirement.description),
                        &requirement.steps,
                        space.as_deref(),
                        Some(&priority),
                    )
                    .await
                {
                    Ok(value) => created.push(value),
                    Err(_) => failed.push(requirement.title),
                }
            }
            render_import(&file, created, failed, mode)
        }
    }
}

struct ParsedRequirement {
    title: String,
    description: String,
    steps: Vec<serde_json::Value>,
}

fn parse_gherkin(content: &str) -> Vec<ParsedRequirement> {
    let mut requirements = Vec::new();
    let mut feature_description = String::new();
    let mut current: Option<ParsedRequirement> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if let Some(description) = line.strip_prefix("Feature:") {
            feature_description = description.trim().to_owned();
        } else if let Some(title) = line
            .strip_prefix("Scenario Outline:")
            .or_else(|| line.strip_prefix("Scenario:"))
        {
            if let Some(previous) = current.take() {
                requirements.push(previous);
            }
            current = Some(ParsedRequirement {
                title: title.trim().to_owned(),
                description: feature_description.clone(),
                steps: Vec::new(),
            });
        } else if let Some(requirement) = current.as_mut()
            && let Some((keyword, text)) = parse_gherkin_step(line)
        {
            requirement
                .steps
                .push(serde_json::json!({ "keyword": keyword, "text": text }));
        }
    }
    if let Some(requirement) = current {
        requirements.push(requirement);
    }
    requirements
}

fn parse_gherkin_step(line: &str) -> Option<(String, String)> {
    let (keyword, text) = line.split_once(char::is_whitespace)?;
    if !["given", "when", "then", "and", "but"].contains(&keyword.to_ascii_lowercase().as_str()) {
        return None;
    }
    Some((keyword.to_ascii_lowercase(), text.trim_start().to_owned()))
}

fn render_import(
    file: &Path,
    created: Vec<Requirement>,
    failed: Vec<String>,
    mode: OutputMode,
) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({
            "total": created.len() + failed.len(),
            "created": created,
            "failed": failed,
        })),
        OutputMode::Quiet => {
            for requirement in created {
                println!("{}", requirement.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            println!(
                "Imported {} requirement(s) from {}",
                created.len(),
                file.display()
            );
            if !failed.is_empty() {
                println!("Failed to import {} requirement(s)", failed.len());
            }
            Ok(())
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

#[cfg(test)]
mod tests {
    use super::parse_gherkin;

    #[test]
    fn gherkin_parser_turns_scenarios_and_steps_into_requirements() {
        // Given a feature containing a scenario and a scenario outline
        let source = r#"
            Feature: Authentication
            Scenario: Sign in
              Given a registered user
              When they submit valid credentials
              Then access is granted

            Scenario Outline: Reject invalid credentials
              GIVEN a registered user
              But the password is invalid
        "#;

        // When the feature is parsed for import
        let requirements = parse_gherkin(source);

        // Then each scenario inherits the feature and preserves normalized steps
        assert_eq!(requirements.len(), 2);
        assert_eq!(requirements[0].title, "Sign in");
        assert_eq!(requirements[0].description, "Authentication");
        assert_eq!(requirements[0].steps[0]["keyword"], "given");
        assert_eq!(requirements[1].title, "Reject invalid credentials");
        assert_eq!(requirements[1].steps[1]["keyword"], "but");
    }

    #[test]
    fn gherkin_parser_ignores_steps_before_the_first_scenario() {
        // Given a feature with prose that resembles a step before any scenario
        let source =
            "Feature: Search\nGiven background prose\nScenario: Find\nThen a result appears";

        // When the feature is parsed
        let requirements = parse_gherkin(source);

        // Then only the scenario's step is retained
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].steps.len(), 1);
        assert_eq!(requirements[0].steps[0]["keyword"], "then");
    }
}
