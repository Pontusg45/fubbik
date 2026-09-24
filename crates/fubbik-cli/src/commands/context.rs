use std::path::Path;

use anyhow::{Context, Result, bail};
use owo_colors::OwoColorize;

use crate::ContextCommand;
use crate::client::{ClaudeMdResponse, Client};
use crate::config;
use crate::output::{self, OutputMode};

pub async fn run(client: &Client, command: ContextCommand, mode: OutputMode) -> Result<()> {
    match command {
        ContextCommand::About {
            concept,
            max_tokens,
            space,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let format = if mode == OutputMode::Json {
                "structured-json"
            } else {
                "structured-md"
            };
            let value = client
                .context_about(
                    &concept,
                    space.as_deref(),
                    checked_budget(max_tokens)?,
                    format,
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::Export {
            max_tokens,
            space,
            format,
            for_path,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let value = client
                .export_context(
                    space.as_deref(),
                    checked_budget(max_tokens.unwrap_or(settings.context.max_tokens))?,
                    &format,
                    for_path.as_deref(),
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::For {
            path,
            space,
            max_tokens,
            format,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let value = client
                .context_for_file(
                    &path.to_string_lossy(),
                    space.as_deref(),
                    checked_budget(max_tokens.unwrap_or(settings.context.max_tokens))?,
                    &format,
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::ForPlan {
            plan_id,
            max_tokens,
            space: _,
        } => {
            let format = context_format(mode);
            let value = client
                .context_for_plan(&plan_id, checked_budget(max_tokens)?, format)
                .await?;
            render_context(value, mode)
        }
        ContextCommand::ForDiff {
            staged,
            max_tokens,
            space,
        } => {
            let paths = changed_files(staged)?;
            if paths.is_empty() {
                return match mode {
                    OutputMode::Json => {
                        output::json(&serde_json::json!({ "files": [], "chunks": [] }))
                    }
                    _ => {
                        println!("No changed files.");
                        Ok(())
                    }
                };
            }
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let value = client
                .context_for_files(
                    &paths,
                    space.as_deref(),
                    checked_budget(max_tokens)?,
                    context_format(mode),
                )
                .await?;
            render_context(value, mode)
        }
        ContextCommand::ClaudeMd {
            space,
            tag,
            max_tokens,
            output: path,
        } => {
            let (settings, _) = config::load()?;
            let space = client
                .resolve_space(space.as_deref().or(settings.space.as_deref()))
                .await?;
            let response = client
                .claude_md(
                    space.as_deref(),
                    tag.as_deref().or(Some(settings.claude_md.tag.as_str())),
                    checked_budget(max_tokens.unwrap_or(settings.claude_md.max_tokens))?,
                )
                .await?;
            if let Some(path) = path {
                write_generated(&path, &response.content)?;
                render_written(&path, &response, mode)
            } else {
                render_claude_md(&response, mode)
            }
        }
    }
}

pub async fn sync(
    client: &Client,
    output_path: Option<&Path>,
    space: Option<&str>,
    tag: Option<&str>,
    max_tokens: Option<usize>,
    dry_run: bool,
    mode: OutputMode,
) -> Result<()> {
    let (settings, _) = config::load()?;
    let path = output_path.unwrap_or(&settings.claude_md.output);
    let space = client
        .resolve_space(space.or(settings.space.as_deref()))
        .await?;
    let response = client
        .claude_md(
            space.as_deref(),
            tag.or(Some(settings.claude_md.tag.as_str())),
            checked_budget(max_tokens.unwrap_or(settings.claude_md.max_tokens))?,
        )
        .await?;
    if dry_run {
        return match mode {
            OutputMode::Json => output::json(&serde_json::json!({
                "dryRun": true, "output": path, "chunks": response.chunks,
                "bytes": response.content.len(), "content": response.content,
            })),
            _ => {
                print!("{}", response.content);
                if !response.content.ends_with('\n') {
                    println!();
                }
                Ok(())
            }
        };
    }
    write_generated(path, &response.content)?;
    render_written(path, &response, mode)
}

fn checked_budget(value: usize) -> Result<usize> {
    if value == 0 {
        bail!("max tokens must be greater than zero");
    }
    Ok(value)
}

fn context_format(mode: OutputMode) -> &'static str {
    if mode == OutputMode::Json {
        "structured-json"
    } else {
        "structured-md"
    }
}

fn changed_files(staged: bool) -> Result<Vec<String>> {
    let mut command = std::process::Command::new("git");
    command.args(["diff", "--name-only"]);
    if staged {
        command.arg("--staged");
    }
    let result = command.output().context("could not run git diff")?;
    if !result.status.success() {
        bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(String::from_utf8(result.stdout)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn render_context(value: serde_json::Value, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&value);
    }
    if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
        print!("{content}");
        if !content.ends_with('\n') {
            println!();
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}

fn render_claude_md(response: &ClaudeMdResponse, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(response);
    }
    print!("{}", response.content);
    if !response.content.ends_with('\n') {
        println!();
    }
    Ok(())
}

fn render_written(path: &Path, response: &ClaudeMdResponse, mode: OutputMode) -> Result<()> {
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({
            "output": path, "chunks": response.chunks, "bytes": response.content.len()
        }))?,
        OutputMode::Quiet => println!("{}", path.display()),
        OutputMode::Human => println!(
            "{} {} ({} chunks)",
            "wrote".green(),
            path.display(),
            response.chunks
        ),
    }
    Ok(())
}

fn write_generated(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temporary = path.with_extension(format!("fubbik.{}.tmp", std::process::id()));
    std::fs::write(&temporary, content)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}
