//! Is a worktree's generated CLAUDE.md current? Mirrors `fubbik sync`: the
//! repo's `fubbik.config.json` names the output (`claudeMd.output`, default
//! `.claude/CLAUDE.md`), the tag (`claudeMd.tag`, default `claude-context`)
//! and the budget (`claudeMd.maxTokens`, default 32000); the server's
//! `/api/chunks/export/claude-md` content is written verbatim.

use std::path::{Path, PathBuf};

use serde_json::Value;

pub const CONFIG_FILE: &str = "fubbik.config.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeMdConfig {
    /// Relative to the repo (or worktree) root.
    pub output: PathBuf,
    pub tag: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdState {
    InSync,
    Stale,
    Missing,
}

impl MdState {
    pub fn label(self) -> &'static str {
        match self {
            MdState::InSync => "in sync",
            MdState::Stale => "stale",
            MdState::Missing => "not generated",
        }
    }
}

/// `None` when the repo has no `claudeMd` setup in `fubbik.config.json`.
pub fn read_config(repo: &Path) -> Option<ClaudeMdConfig> {
    let text = std::fs::read_to_string(repo.join(CONFIG_FILE)).ok()?;
    parse_config(&text)
}

pub fn parse_config(text: &str) -> Option<ClaudeMdConfig> {
    let value: Value = serde_json::from_str(text).ok()?;
    let md = value.get("claudeMd")?;
    Some(ClaudeMdConfig {
        output: PathBuf::from(
            md.get("output")
                .and_then(Value::as_str)
                .unwrap_or(".claude/CLAUDE.md"),
        ),
        tag: md
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or("claude-context")
            .to_string(),
        max_tokens: md.get("maxTokens").and_then(Value::as_u64).unwrap_or(32000) as u32,
    })
}

pub fn state(root: &Path, config: &ClaudeMdConfig, generated: &str) -> MdState {
    match std::fs::read_to_string(root.join(&config.output)) {
        Ok(current) if current == generated => MdState::InSync,
        Ok(_) => MdState::Stale,
        Err(_) => MdState::Missing,
    }
}

/// Atomic, like the CLI: write a temp file next to it, then rename.
pub fn write(root: &Path, config: &ClaudeMdConfig, generated: &str) -> Result<PathBuf, String> {
    let path = root.join(&config.output);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("md.tmp");
    std::fs::write(&temp, generated).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fill_missing_keys() {
        // Given
        let text = r#"{"space": "fubbik", "claudeMd": {"tag": "agents"}}"#;

        // When
        let config = parse_config(text).unwrap();

        // Then
        assert_eq!(config.output, PathBuf::from(".claude/CLAUDE.md"));
        assert_eq!(config.tag, "agents");
        assert_eq!(config.max_tokens, 32000);
    }

    #[test]
    fn no_claude_md_section_is_none() {
        // Given
        let text = r#"{"serverUrl": "http://localhost:3000"}"#;

        // When
        let config = parse_config(text);

        // Then
        assert!(config.is_none());
    }

    #[test]
    fn state_follows_file_content() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let config = parse_config(r#"{"claudeMd": {}}"#).unwrap();

        // When
        let missing = state(dir.path(), &config, "A");
        write(dir.path(), &config, "A").unwrap();
        let in_sync = state(dir.path(), &config, "A");
        let stale = state(dir.path(), &config, "B");

        // Then
        assert_eq!(
            [missing, in_sync, stale],
            [MdState::Missing, MdState::InSync, MdState::Stale]
        );
    }
}
