use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::output::OutputMode;

pub const PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub command: String,
    pub path: PathBuf,
}

fn plugin_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(value) = std::env::var_os("FUBBIK_PLUGIN_PATH") {
        directories.extend(std::env::split_paths(&value));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.push(home.join(".fubbik/plugins"));
        directories.push(home.join(".local/share/fubbik/plugins"));
    }
    if let Some(value) = std::env::var_os("PATH") {
        directories.extend(std::env::split_paths(&value));
    }
    directories
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(OsStr::to_str)
            .map(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "exe" | "cmd" | "bat" | "com"
                )
            })
            .unwrap_or(false)
}

fn command_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?.to_owned();
    #[cfg(windows)]
    let name = match name.rsplit_once('.') {
        Some((stem, extension))
            if matches!(
                extension.to_ascii_lowercase().as_str(),
                "exe" | "cmd" | "bat" | "com"
            ) =>
        {
            stem.to_owned()
        }
        _ => name,
    };
    name.strip_prefix("fubbik-")
        .filter(|command| !command.is_empty())
        .map(str::to_owned)
}

pub fn discover() -> Vec<InstalledPlugin> {
    let mut seen = HashSet::new();
    let mut plugins = Vec::new();
    for directory in plugin_directories() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(command) = command_name(&path) else {
                continue;
            };
            if !is_executable(&path) || !seen.insert(command.clone()) {
                continue;
            }
            plugins.push(InstalledPlugin { command, path });
        }
    }
    plugins.sort_by(|a, b| a.command.cmp(&b.command));
    plugins
}

fn resolve(command: &OsStr) -> Option<PathBuf> {
    discover()
        .into_iter()
        .find(|plugin| OsStr::new(&plugin.command) == command)
        .map(|plugin| plugin.path)
}

pub async fn execute(args: Vec<OsString>, base_url: &str, output: OutputMode) -> Result<()> {
    let Some(command) = args.first() else {
        bail!("missing plugin command");
    };
    let executable = resolve(command).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown command {:?}; no executable named fubbik-{:?} was found in FUBBIK_PLUGIN_PATH or PATH",
            command,
            command
        )
    })?;

    let status = tokio::process::Command::new(&executable)
        .args(&args)
        .env("FUBBIK_URL", base_url)
        .env("FUBBIK_PLUGIN_PROTOCOL", PROTOCOL_VERSION)
        .env("FUBBIK_VERSION", env!("CARGO_PKG_VERSION"))
        .env("FUBBIK_OUTPUT", output.as_env())
        .status()
        .await
        .with_context(|| format!("failed to execute plugin {}", executable.display()))?;

    if !status.success() {
        bail!("plugin {} exited with {status}", executable.display());
    }
    Ok(())
}
