//! The app's own settings: which repos are registered, and where fubbik runs.
//!
//! Stored at `$HOME/.config/fubbik/desktop.toml`, or `FUBBIK_DESKTOP_CONFIG`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fubbik_url: Option<String>,
    /// The fubbik web app; else `FUBBIK_WEB_URL`, else `http://localhost:3001`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    /// The Rust `fubbik` binary, when it is not found on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fubbik_bin: Option<PathBuf>,
    /// Where AI sessions run: `"embedded"` (the default, the Terminal tab) or
    /// `"external"` (Terminal.app).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    /// `"tmux"` (the default when tmux is installed): terminal tabs are
    /// sessions on a private tmux server and survive quitting the app.
    /// `"none"`: plain shells that end with the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_persistence: Option<String>,
    /// Most AI agents running at once across all repos; more launches wait
    /// in a queue. Unset: no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_agents: Option<usize>,
    /// Colors: unset for the built-in look, `"follow"` for your Ghostty
    /// config (its theme, its colors, its font), or a Ghostty theme name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ghostty_theme: Option<String>,
    /// `"translucent"` or `"opaque"` (the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The system-wide key that shows or hides the app, like
    /// `"ctrl-alt-f"` (the default when unset); `""` or `"none"` turns it off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_hotkey: Option<String>,
    /// Where processes and scripts start without asking: `"background"`,
    /// `"tmux"`, `"tab"` or `"terminal"`. Unset: the app asks each time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_in: Option<String>,
    /// The editor that opens files and folders: `"zed"`, `"cursor"` or
    /// `"code"`. Unset: `$VISUAL` / `$EDITOR`, else the first installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// `"embedded"` (the default): the app runs the fubbik API and its own
    /// PostgreSQL; `"off"`: it starts neither (run `fubbik serve` yourself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, rename = "repo")]
    pub repos: Vec<RepoEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoEntry {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl RepoEntry {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

pub fn path() -> PathBuf {
    if let Some(path) = std::env::var_os("FUBBIK_DESKTOP_CONFIG") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config/fubbik/desktop.toml")
}

impl AppConfig {
    pub fn embedded_backend(&self) -> bool {
        self.backend.as_deref() != Some("off")
    }

    /// The summon key, or None when turned off.
    pub fn global_hotkey(&self) -> Option<&str> {
        match self.global_hotkey.as_deref() {
            None => Some(crate::global_hotkey::DEFAULT),
            Some("" | "none") => None,
            Some(keys) => Some(keys),
        }
    }

    pub fn load_from(path: &Path) -> Result<AppConfig, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Adds a repo; false when it is already registered.
    pub fn add_repo(&mut self, path: PathBuf) -> bool {
        if self.repos.iter().any(|repo| repo.path == path) {
            return false;
        }
        self.repos.push(RepoEntry { path, group: None });
        true
    }

    pub fn remove_repo(&mut self, path: &Path) {
        self.repos.retain(|repo| repo.path != path);
    }

    pub fn fubbik_url(&self) -> String {
        self.fubbik_url
            .clone()
            .or_else(|| std::env::var("FUBBIK_URL").ok())
            .unwrap_or_else(|| "http://localhost:3000".into())
    }

    pub fn tmux_persistence(&self) -> bool {
        self.terminal_persistence.as_deref() != Some("none")
    }

    pub fn external_terminal(&self) -> bool {
        self.terminal.as_deref() == Some("external")
    }

    pub fn translucent(&self) -> bool {
        self.theme.as_deref() == Some("translucent")
    }

    pub fn web_url(&self) -> String {
        self.web_url
            .clone()
            .or_else(|| std::env::var("FUBBIK_WEB_URL").ok())
            .unwrap_or_else(|| "http://localhost:3001".into())
            .trim_end_matches('/')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_repos() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/desktop.toml");
        let mut config = AppConfig::default();
        config.add_repo("/code/a".into());
        config.repos.push(RepoEntry {
            path: "/code/b".into(),
            group: Some("work".into()),
        });

        // When
        config.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path).unwrap();

        // Then
        assert_eq!(loaded, config);
    }

    #[test]
    fn missing_file_is_default() {
        // Given
        let dir = tempfile::tempdir().unwrap();

        // When
        let loaded = AppConfig::load_from(&dir.path().join("none.toml")).unwrap();

        // Then
        assert_eq!(loaded, AppConfig::default());
    }

    #[test]
    fn add_repo_dedupes() {
        // Given
        let mut config = AppConfig::default();

        // When
        let first = config.add_repo("/code/a".into());
        let second = config.add_repo("/code/a".into());

        // Then
        assert!(first);
        assert!(!second);
        assert_eq!(config.repos.len(), 1);
    }

    #[test]
    fn remove_repo_drops_only_that_path() {
        // Given
        let mut config = AppConfig::default();
        config.add_repo("/code/a".into());
        config.add_repo("/code/b".into());

        // When
        config.remove_repo(Path::new("/code/a"));

        // Then
        assert_eq!(config.repos.len(), 1);
        assert_eq!(config.repos[0].path, PathBuf::from("/code/b"));
    }
}
