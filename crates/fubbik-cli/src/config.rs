use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "fubbik.config.json";
pub const DEFAULT_SERVER_URL: &str = "http://localhost:3000";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub server_url: Option<String>,
    pub default_type: Option<String>,
    pub space: Option<String>,
    pub context: ContextConfig,
    pub claude_md: ClaudeMdConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextConfig {
    pub max_tokens: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self { max_tokens: 4000 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ClaudeMdConfig {
    pub tag: String,
    pub output: PathBuf,
    #[serde(rename = "maxTokens")]
    pub max_tokens: usize,
}

impl Default for ClaudeMdConfig {
    fn default() -> Self {
        Self {
            tag: "claude-context".into(),
            output: PathBuf::from(".claude/CLAUDE.md"),
            max_tokens: 32_000,
        }
    }
}

pub fn find_from(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn find() -> Result<Option<PathBuf>> {
    Ok(find_from(&std::env::current_dir()?))
}

pub fn load() -> Result<(Config, Option<PathBuf>)> {
    let Some(path) = find()? else {
        return Ok((Config::default(), None));
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let config = serde_json::from_str(&text)
        .with_context(|| format!("invalid configuration in {}", path.display()))?;
    Ok((config, Some(path)))
}

pub fn save(config: &Config, path: &Path) -> Result<()> {
    let text = format!("{}\n", serde_json::to_string_pretty(config)?);
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&temporary, text)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

pub fn set(config: &mut Config, key: &str, value: &str) -> Result<()> {
    match key {
        "url" | "server-url" => config.server_url = Some(value.trim_end_matches('/').into()),
        "default-type" => config.default_type = Some(value.into()),
        "space" => config.space = Some(value.into()),
        "context.max-tokens" => config.context.max_tokens = positive_usize(value)?,
        "claude-md.tag" => config.claude_md.tag = value.into(),
        "claude-md.output" => config.claude_md.output = value.into(),
        "claude-md.max-tokens" => config.claude_md.max_tokens = positive_usize(value)?,
        _ => bail!("unknown configuration key: {key}"),
    }
    Ok(())
}

fn positive_usize(value: &str) -> Result<usize> {
    let parsed = value
        .parse::<usize>()
        .context("max tokens must be a positive integer")?;
    if parsed == 0 {
        bail!("max tokens must be greater than zero");
    }
    Ok(parsed)
}

pub fn resolve_base_url(explicit: Option<&str>) -> Result<String> {
    if let Some(url) = explicit {
        return Ok(url.trim_end_matches('/').into());
    }
    if let Ok(url) = std::env::var("FUBBIK_URL") {
        return Ok(url.trim_end_matches('/').into());
    }
    let (config, _) = load()?;
    Ok(server_url_or_default(&config))
}

fn server_url_or_default(config: &Config) -> String {
    config
        .server_url
        .clone()
        .or_else(|| discovered_server().map(|d| d.url))
        .as_deref()
        .unwrap_or(DEFAULT_SERVER_URL)
        .trim_end_matches('/')
        .into()
}

/// Written by the fubbik desktop app while its built-in server runs:
/// `$XDG_CONFIG_HOME/fubbik/server.json`, else `~/.config/fubbik/server.json`.
pub fn discovery_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("fubbik/server.json"))
}

/// The desktop app's running server, as `server.json` describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiscoveredServer {
    pub url: String,
    pub pid: u32,
}

/// The server in `server.json`, if its process is still alive (a stale
/// file from a crash is ignored).
pub fn discovered_server() -> Option<DiscoveredServer> {
    parse_discovery(&std::fs::read_to_string(discovery_path()?).ok()?)
        .filter(|d| process_alive(d.pid))
}

fn parse_discovery(text: &str) -> Option<DiscoveredServer> {
    serde_json::from_str(text).ok()
}

fn process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_json_gives_url_and_pid() {
        // Given
        let text = r#"{"url": "http://127.0.0.1:3100", "pid": 4242, "owner": "fubbik-desktop"}"#;

        // When
        let parsed = parse_discovery(text);

        // Then
        assert_eq!(
            parsed,
            Some(DiscoveredServer {
                url: "http://127.0.0.1:3100".into(),
                pid: 4242
            })
        );
        assert_eq!(parse_discovery("not json"), None);
    }

    #[test]
    fn default_server_url_is_port_3000() {
        // Given a configuration with no server URL set (no env, no file read)
        let config = Config::default();
        // When the URL is resolved from that configuration
        let url = server_url_or_default(&config);
        // Then it falls back to the standard API port
        assert_eq!(url, "http://localhost:3000");
        assert_eq!(DEFAULT_SERVER_URL, "http://localhost:3000");
    }

    #[test]
    fn finds_nearest_parent_config() {
        // Given
        let root = std::env::temp_dir().join(format!("fubbik-config-{}", std::process::id()));
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        // When
        std::fs::write(root.join(FILE_NAME), "{}").unwrap();
        // Then
        assert_eq!(find_from(&nested), Some(root.join(FILE_NAME)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_keys() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(set(&mut Config::default(), "wat", "x").is_err());
    }

    #[test]
    fn rejects_zero_token_budgets() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(set(&mut Config::default(), "context.max-tokens", "0").is_err());
    }
}
