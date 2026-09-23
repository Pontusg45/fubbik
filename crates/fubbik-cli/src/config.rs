use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "fubbik.config.json";

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
    Ok(config
        .server_url
        .unwrap_or_else(|| "http://localhost:3100".into())
        .trim_end_matches('/')
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;

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
