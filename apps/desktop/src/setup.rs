//! Local setup a repo needs for fubbik's agent tools: the Rust `fubbik`
//! binary, the MCP server in Claude Code's config, and the pre-commit hook.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

/// The marker `fubbik hooks install` writes (crates/fubbik-cli/src/commands/hooks.rs).
pub const HOOK_MARKER: &str = "# Installed by fubbik";
pub const HOOK_SCRIPT: &str = "#!/bin/sh\n# Installed by fubbik — chunk-aware pre-commit hook\nfubbik check-files --staged 2>&1 || true\n";

/// The Rust CLI prints `fubbik <version>` (clap); the legacy TypeScript CLI
/// prints a bare version number and has no `mcp` command.
pub fn is_rust_cli(version_output: &str) -> bool {
    version_output.trim().starts_with("fubbik ")
}

fn version_of(bin: &Path) -> Option<String> {
    let output = Command::new(bin).arg("--version").output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The first working Rust `fubbik`: the configured one, a build inside a
/// registered fubbik checkout, `~/.cargo/bin`, then `PATH`.
pub fn resolve_fubbik_bin(configured: Option<&Path>, repos: &[PathBuf]) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut candidates: Vec<PathBuf> = configured.into_iter().map(Path::to_path_buf).collect();
    for repo in repos.iter().filter(|r| is_fubbik_checkout(r)) {
        candidates.push(repo.join("target/release/fubbik"));
        candidates.push(repo.join("target/debug/fubbik"));
    }
    candidates.push(home.join(".cargo/bin/fubbik"));
    candidates.push(PathBuf::from("fubbik"));
    candidates
        .into_iter()
        .find(|bin| version_of(bin).is_some_and(|v| is_rust_cli(&v)))
}

pub fn is_fubbik_checkout(repo: &Path) -> bool {
    repo.join("crates/fubbik/Cargo.toml").is_file()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpStatus {
    /// `<repo>/.mcp.json` has a `fubbik` server.
    Project,
    /// `~/.claude.json` has it, globally or for this project.
    User,
    Missing,
}

fn has_fubbik_server(value: &Value) -> bool {
    value
        .get("mcpServers")
        .and_then(|s| s.get("fubbik"))
        .is_some()
}

pub fn mcp_status(repo: &Path, claude_json: &Path) -> McpStatus {
    let read = |path: &Path| -> Option<Value> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    };
    if read(&repo.join(".mcp.json")).is_some_and(|v| has_fubbik_server(&v)) {
        return McpStatus::Project;
    }
    if let Some(user) = read(claude_json) {
        let project = user
            .get("projects")
            .and_then(|p| p.get(repo.to_string_lossy().as_ref()));
        if has_fubbik_server(&user) || project.is_some_and(has_fubbik_server) {
            return McpStatus::User;
        }
    }
    McpStatus::Missing
}

pub fn claude_json() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".claude.json")
}

/// Adds a `fubbik` server to `<repo>/.mcp.json`, keeping every other entry.
pub fn install_mcp(repo: &Path, bin: &Path, server_url: &str) -> Result<PathBuf, String> {
    let path = repo.join(".mcp.json");
    let mut config: Value = match std::fs::read_to_string(&path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!(".mcp.json is not valid JSON: {e}"))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.to_string()),
    };
    let servers = config
        .as_object_mut()
        .ok_or(".mcp.json is not a JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    servers
        .as_object_mut()
        .ok_or("mcpServers is not an object")?
        .insert(
            "fubbik".into(),
            json!({
                "command": bin.to_string_lossy(),
                "args": ["mcp"],
                "env": { "FUBBIK_SERVER_URL": server_url },
            }),
        );
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookStatus {
    Installed,
    /// A pre-commit hook exists that fubbik did not write.
    Other,
    Missing,
}

/// `git rev-parse --git-path hooks/pre-commit`: right for worktrees and
/// `core.hooksPath` too.
pub fn hook_path(dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "hooks/pre-commit",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

pub fn hook_status(dir: &Path) -> HookStatus {
    match hook_path(dir).and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string())) {
        Ok(text) if text.contains(HOOK_MARKER) => HookStatus::Installed,
        Ok(_) => HookStatus::Other,
        Err(_) => HookStatus::Missing,
    }
}

/// Writes the same hook as `fubbik hooks install`; refuses to replace a
/// hook fubbik did not write.
pub fn install_hook(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let path = hook_path(dir)?;
    if path.exists() && hook_status(dir) == HookStatus::Other {
        return Err(format!(
            "{} already exists and was not written by fubbik",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, HOOK_SCRIPT).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["init", "-q"])
                .status()
                .unwrap()
                .success()
        );
        dir
    }

    #[test]
    fn tells_rust_cli_from_legacy() {
        // Given
        let outputs = ["fubbik 0.1.0\n", "0.0.1\n"];

        // When
        let rust: Vec<bool> = outputs.iter().map(|o| is_rust_cli(o)).collect();

        // Then
        assert_eq!(rust, [true, false]);
    }

    #[test]
    fn install_mcp_keeps_other_servers() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"other":{"command":"x"}}}"#,
        )
        .unwrap();
        let claude_json = dir.path().join("claude.json");

        // When
        let before = mcp_status(dir.path(), &claude_json);
        install_mcp(
            dir.path(),
            Path::new("/bin/fubbik"),
            "http://localhost:3000",
        )
        .unwrap();
        let after = mcp_status(dir.path(), &claude_json);

        // Then
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(before, McpStatus::Missing);
        assert_eq!(after, McpStatus::Project);
        assert_eq!(written["mcpServers"]["other"]["command"], "x");
        assert_eq!(written["mcpServers"]["fubbik"]["args"][0], "mcp");
        assert_eq!(
            written["mcpServers"]["fubbik"]["env"]["FUBBIK_SERVER_URL"],
            "http://localhost:3000"
        );
    }

    #[test]
    fn user_level_mcp_config_counts() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let claude_json = dir.path().join("claude.json");
        let repo = dir.path().join("repo");
        std::fs::write(
            &claude_json,
            json!({"projects": {repo.to_string_lossy(): {"mcpServers": {"fubbik": {}}}}})
                .to_string(),
        )
        .unwrap();

        // When
        let status = mcp_status(&repo, &claude_json);

        // Then
        assert_eq!(status, McpStatus::User);
    }

    #[test]
    fn hook_install_refuses_foreign_hook() {
        // Given
        let repo = git_repo();
        let path = hook_path(repo.path()).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "#!/bin/sh\nlint\n").unwrap();

        // When
        let result = install_hook(repo.path());

        // Then
        assert!(result.is_err());
        assert_eq!(hook_status(repo.path()), HookStatus::Other);
    }

    #[test]
    fn hook_install_writes_marker() {
        // Given
        let repo = git_repo();

        // When
        install_hook(repo.path()).unwrap();

        // Then
        assert_eq!(hook_status(repo.path()), HookStatus::Installed);
    }
}

/// The folder a tracker agent works from: the registered repos' common
/// parent (`~/projects`), else the home folder.
pub fn tracker_dir(repos: &[PathBuf]) -> PathBuf {
    let parents: Vec<&Path> = repos.iter().filter_map(|r| r.parent()).collect();
    match parents.first() {
        Some(first) if parents.iter().all(|p| p == first) => first.to_path_buf(),
        _ => std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/")),
    }
}

/// What a tracker agent is told (the CLAUDE.md written for it).
pub const TRACKER_CLAUDE_MD: &str = r#"# Projects tracker

You keep track of every project in this folder. You do not write features here: you watch, summarize, and point
at what needs attention. The fubbik desktop app does the watching; you read it through two MCP servers (`.mcp.json`):

- `fubbik-desktop` — the app (it must be running): repos, processes, agents, worktrees, git changes, pull requests,
  dependencies across repos, the toolchain, and a feed of what changed.
- `fubbik` — the knowledge base: chunks, plans, requirements, agent boards.

## Each check

1. `get_changes_since` with the cursor from last time (kept in `.tracker-cursor` in this folder; none on the first
   run). Save the new cursor. If `dropped` is true, read `get_today` and `list_repos` in full instead.
2. `get_today` and `list_alerts`: what needs the user (including alerts from your watches), what is in flight, what
   is next (including open todos).
3. Report briefly, most urgent first: agents waiting, processes down, pull requests needing the user (failing
   checks, changes requested, conflicts), worktrees that overlap, new dependency misalignments, tools Homebrew
   upgraded (these often explain "it worked yesterday").

## Weekly, or when asked

- `get_dependencies` (filter `breaking`): which repos are a major behind, and the files to change. Propose an order
  (shared tooling first: TypeScript, vitest, vite, pnpm).
- `get_toolchain`: missing tools, available upgrades, services that are off.
- `list_worktrees` per repo: merged worktrees that can go, stale ones, overlaps.

## Rules

- Read-only by default. Start, stop or restart processes and run scripts only when the user asks.
- Never commit, push, merge, delete branches or worktrees, or upgrade tools yourself: say what you would do and why.
- Keep reports short: a line per item, the repo first, and what to do about it.
- Things to follow up go in the app: `add_note` (kind `todo` shows on Today until `complete_note`; `decision` for
  choices made). Knowledge worth keeping with the code goes in fubbik chunks (the `fubbik` server) instead.
- Instead of polling for something specific, set a watch: `add_watch` with `event` (a feed event kind and/or a phrase,
  e.g. `dependency_misaligned` + "vitest"), `process_down` (a process down N minutes) or `agent_waiting` (N minutes).
  The app checks it while it runs and raises an alert (`list_alerts`, also on Today). Check `list_alerts` each time;
  `dismiss_alert` once handled.
"#;

/// Makes `dir` a workspace for a tracker agent: `.mcp.json` with both
/// servers (merged into an existing one) and a CLAUDE.md (only if there is
/// none). Returns what it did, one line per file.
pub fn install_tracker(
    dir: &Path,
    fubbik_bin: Option<&Path>,
    server_url: &str,
    desktop_mcp_url: &str,
) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    let mcp = dir.join(".mcp.json");
    if let Some(bin) = fubbik_bin {
        install_mcp(dir, bin, server_url)?;
    }
    let mut config: Value = match std::fs::read_to_string(&mcp) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!(".mcp.json is not valid JSON: {e}"))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.to_string()),
    };
    config
        .as_object_mut()
        .ok_or(".mcp.json is not a JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("mcpServers is not an object")?
        .insert(
            "fubbik-desktop".into(),
            json!({ "type": "http", "url": desktop_mcp_url }),
        );
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&mcp, text).map_err(|e| e.to_string())?;
    done.push(format!(
        "{} — fubbik-desktop{}",
        mcp.display(),
        if fubbik_bin.is_some() {
            " and fubbik"
        } else {
            " (fubbik binary not found: knowledge server left out)"
        }
    ));
    let claude = dir.join("CLAUDE.md");
    if claude.exists() {
        done.push(format!(
            "{} — kept (it exists; the tracker text was not added)",
            claude.display()
        ));
    } else {
        std::fs::write(&claude, TRACKER_CLAUDE_MD).map_err(|e| e.to_string())?;
        done.push(format!("{} — written", claude.display()));
    }
    Ok(done)
}

#[cfg(test)]
mod tracker_tests {
    use super::*;

    #[test]
    fn tracker_dir_is_the_common_parent() {
        // Given
        let same = [
            PathBuf::from("/u/projects/a"),
            PathBuf::from("/u/projects/b"),
        ];
        let mixed = [PathBuf::from("/u/projects/a"), PathBuf::from("/u/work/b")];

        // When
        let common = tracker_dir(&same);
        let fallback = tracker_dir(&mixed);

        // Then
        assert_eq!(common, PathBuf::from("/u/projects"));
        assert_ne!(fallback, PathBuf::from("/u/projects"));
    }

    #[test]
    fn install_tracker_merges_mcp_json_and_keeps_an_existing_claude_md() {
        // Given: a folder with another MCP server and its own CLAUDE.md
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers": {"other": {"command": "x"}}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "mine\n").unwrap();

        // When
        let done = install_tracker(
            dir.path(),
            Some(Path::new("/bin/fubbik")),
            "http://localhost:3000",
            "http://127.0.0.1:3100/mcp",
        )
        .unwrap();
        let again = install_tracker(dir.path(), None, "u", "http://127.0.0.1:3100/mcp");

        // Then
        let config: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(".mcp.json")).unwrap())
                .unwrap();
        let servers = config["mcpServers"].as_object().unwrap();
        assert!(servers.contains_key("other"));
        assert_eq!(
            servers["fubbik-desktop"]["url"],
            "http://127.0.0.1:3100/mcp"
        );
        assert_eq!(servers["fubbik"]["args"][0], "mcp");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "mine\n"
        );
        assert!(done[1].contains("kept"));
        assert!(again.is_ok(), "running it twice is fine");
    }

    #[test]
    fn install_tracker_writes_claude_md_when_missing() {
        // Given
        let dir = tempfile::tempdir().unwrap();

        // When
        install_tracker(dir.path(), None, "u", "http://127.0.0.1:3100/mcp").unwrap();

        // Then
        let text = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(text.contains("get_changes_since"));
    }
}
