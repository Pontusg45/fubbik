use anyhow::{Result, bail};
use owo_colors::OwoColorize;

use crate::ConfigCommand;
use crate::client::Client;
use crate::config::{self, Config};
use crate::output::{self, OutputMode};

pub fn init(server: Option<&str>, force: bool, mode: OutputMode) -> Result<()> {
    let path = std::env::current_dir()?.join(config::FILE_NAME);
    if path.exists() && !force {
        bail!(
            "{} already exists; pass --force to replace it",
            path.display()
        );
    }
    let settings = Config {
        server_url: Some(
            server
                .unwrap_or("http://localhost:3100")
                .trim_end_matches('/')
                .into(),
        ),
        default_type: Some("note".into()),
        ..Config::default()
    };
    config::save(&settings, &path)?;
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({ "path": path, "config": settings }))?,
        OutputMode::Quiet => println!("{}", path.display()),
        OutputMode::Human => println!("{} {}", "created".green(), path.display()),
    }
    Ok(())
}

pub fn run(command: ConfigCommand, mode: OutputMode) -> Result<()> {
    match command {
        ConfigCommand::Show => {
            let (settings, path) = config::load()?;
            if mode == OutputMode::Quiet {
                println!(
                    "{}",
                    path.as_deref()
                        .map_or("<defaults>".into(), |p| p.display().to_string())
                );
            } else if mode == OutputMode::Json {
                output::json(&serde_json::json!({ "path": path, "config": settings }))?;
            } else {
                println!(
                    "configuration: {}",
                    path.as_deref()
                        .map_or("defaults".into(), |p| p.display().to_string())
                );
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
        }
        ConfigCommand::Set { key, value } => {
            let (mut settings, found) = config::load()?;
            let path = found.unwrap_or(std::env::current_dir()?.join(config::FILE_NAME));
            config::set(&mut settings, &key, &value)?;
            config::save(&settings, &path)?;
            match mode {
                OutputMode::Json => {
                    output::json(&serde_json::json!({ "path": path, "key": key, "value": value }))?
                }
                OutputMode::Quiet => println!("{value}"),
                OutputMode::Human => println!("{} {key}", "updated".green()),
            }
        }
    }
    Ok(())
}

pub async fn doctor(client: &Client, mode: OutputMode) -> Result<()> {
    let loaded = config::load();
    let (config_ok, config_path, config_error) = match loaded {
        Ok((_, path)) => (true, path, None),
        Err(error) => (false, None, Some(error.to_string())),
    };
    let health = client.health().await;
    let (server_ok, health_value, server_error) = match health {
        Ok(value) => (true, Some(value), None),
        Err(error) => (false, None, Some(error.to_string())),
    };
    let report = serde_json::json!({
        "ok": config_ok && server_ok,
        "config": { "ok": config_ok, "path": config_path, "error": config_error },
        "server": { "ok": server_ok, "url": client.base_url(), "health": health_value, "error": server_error },
    });
    if mode == OutputMode::Json {
        output::json(&report)?;
    } else if mode == OutputMode::Quiet {
        println!(
            "{}",
            if config_ok && server_ok {
                "ok"
            } else {
                "failed"
            }
        );
    } else {
        println!(
            "config: {}",
            if config_ok {
                "ok".green().to_string()
            } else {
                "failed".red().to_string()
            }
        );
        println!(
            "server: {} ({})",
            if server_ok {
                "ok".green().to_string()
            } else {
                "failed".red().to_string()
            },
            client.base_url()
        );
        if let Some(error) = report["server"]["error"].as_str() {
            println!("  {error}");
        }
    }
    if !config_ok || !server_ok {
        bail!("doctor found problems");
    }
    Ok(())
}
