//! The machine's developer tools: which are installed, their versions and
//! paths, what Homebrew can upgrade, and which Homebrew upgraded recently
//! (a silent `brew upgrade` of tmux or libgit2 is a common cause of "it
//! worked yesterday"). Plus whether the services the app leans on are up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// A tool the app or its repos use.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    /// The command whose `version_args` print the version.
    pub bin: &'static str,
    pub version_args: &'static [&'static str],
    /// The Homebrew formula (or `cask:<name>`), for install and upgrade.
    pub brew: Option<&'static str>,
    /// What it is for, in this setup.
    pub why: &'static str,
}

pub const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "git",
        bin: "git",
        version_args: &["--version"],
        brew: Some("git"),
        why: "everything",
    },
    ToolSpec {
        name: "gh",
        bin: "gh",
        version_args: &["--version"],
        brew: Some("gh"),
        why: "pull requests and CI status",
    },
    ToolSpec {
        name: "tmux",
        bin: "tmux",
        version_args: &["-V"],
        brew: Some("tmux"),
        why: "terminal tabs, runs, agent panes",
    },
    ToolSpec {
        name: "just",
        bin: "just",
        version_args: &["--version"],
        brew: Some("just"),
        why: "recipes",
    },
    ToolSpec {
        name: "node",
        bin: "node",
        version_args: &["--version"],
        brew: Some("node"),
        why: "web app, scripts",
    },
    ToolSpec {
        name: "pnpm",
        bin: "pnpm",
        version_args: &["--version"],
        brew: Some("pnpm"),
        why: "TypeScript workspaces",
    },
    ToolSpec {
        name: "mise",
        bin: "mise",
        version_args: &["--version"],
        brew: Some("mise"),
        why: "node, pnpm and bun versions",
    },
    ToolSpec {
        name: "bun",
        bin: "bun",
        version_args: &["--version"],
        brew: None,
        why: "db scripts",
    },
    ToolSpec {
        name: "cargo",
        bin: "cargo",
        version_args: &["--version"],
        brew: None,
        why: "Rust builds (rustup)",
    },
    ToolSpec {
        name: "rustc",
        bin: "rustc",
        version_args: &["--version"],
        brew: None,
        why: "Rust builds (rustup)",
    },
    ToolSpec {
        name: "zig",
        bin: "zig",
        version_args: &["version"],
        brew: Some("zig"),
        why: "builds (libghostty-vt wants zig@0.15)",
    },
    ToolSpec {
        name: "docker",
        bin: "docker",
        version_args: &["--version"],
        brew: Some("cask:orbstack"),
        why: "compose databases (OrbStack)",
    },
    ToolSpec {
        name: "psql",
        bin: "psql",
        version_args: &["--version"],
        brew: Some("libpq"),
        why: "database shells",
    },
    ToolSpec {
        name: "ollama",
        bin: "ollama",
        version_args: &["--version"],
        brew: Some("ollama"),
        why: "enrichment, embeddings",
    },
    ToolSpec {
        name: "claude",
        bin: "claude",
        version_args: &["--version"],
        brew: None,
        why: "Claude Code agents",
    },
    ToolSpec {
        name: "codex",
        bin: "codex",
        version_args: &["--version"],
        brew: Some("codex"),
        why: "Codex agents",
    },
    ToolSpec {
        name: "libgit2",
        bin: "",
        version_args: &[],
        brew: Some("libgit2"),
        why: "linked by the app's git code",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStatus {
    pub name: &'static str,
    pub why: &'static str,
    pub brew: Option<&'static str>,
    /// None: not found on PATH (or, for a library, not installed).
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    /// The newer version Homebrew offers.
    pub outdated: Option<String>,
    /// When Homebrew last installed or upgraded it.
    pub brewed_at: Option<SystemTime>,
}

/// A service the app depends on, and whether it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatus {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub tools: Vec<ToolStatus>,
    pub services: Vec<ServiceStatus>,
    /// `brew outdated` failed or Homebrew is missing.
    pub brew_error: Option<String>,
}

/// The first thing in `text` that looks like a version (`3.8`, `v22.1.0`,
/// `2.103.3`), without a leading `v`.
pub fn version_in(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
        .map(|w| w.trim_start_matches('v'))
        .find(|w| {
            let mut parts = w.split('.');
            parts
                .next()
                .is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
                && w.contains('.')
        })
        .map(|w| {
            w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_string()
        })
}

/// Parses `brew outdated --json=v2`: formula or cask name → newest version.
pub fn parse_outdated(json: &str) -> HashMap<String, String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (kind, prefix) in [("formulae", ""), ("casks", "cask:")] {
        for item in value[kind].as_array().into_iter().flatten() {
            if let (Some(name), Some(current)) =
                (item["name"].as_str(), item["current_version"].as_str())
            {
                out.insert(format!("{prefix}{name}"), current.to_string());
            }
        }
    }
    out
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| candidate.is_file())
}

fn run(bin: &Path, args: &[&str]) -> Option<String> {
    // From home: a repo's pinned version (packageManager, .tool-versions)
    // would answer for itself, not for the machine.
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/".into());
    let output = Command::new(bin)
        .args(args)
        .current_dir(home)
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Some(text)
}

/// `<brew prefix>/Cellar/<formula>` or `Caskroom/<cask>`: its newest entry's
/// modification time, i.e. the last install or upgrade.
fn brewed_at(prefix: &Path, brew: &str) -> Option<SystemTime> {
    let dir = match brew.strip_prefix("cask:") {
        Some(cask) => prefix.join("Caskroom").join(cask),
        None => prefix.join("Cellar").join(brew),
    };
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

/// The installed version of a formula, from its Cellar folder names.
fn cellar_version(prefix: &Path, formula: &str) -> Option<String> {
    let mut versions: Vec<String> = std::fs::read_dir(prefix.join("Cellar").join(formula))
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    versions.sort();
    versions.pop()
}

fn service(name: &'static str, bin: &str, args: &[&str], ok_detail: &str) -> ServiceStatus {
    match which(bin) {
        None => ServiceStatus {
            name,
            ok: false,
            detail: format!("{bin} is not installed"),
        },
        Some(path) => match Command::new(path).args(args).output() {
            Ok(out) if out.status.success() => ServiceStatus {
                name,
                ok: true,
                detail: ok_detail.to_string(),
            },
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stderr);
                let first = text
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("not running");
                ServiceStatus {
                    name,
                    ok: false,
                    detail: first.trim().chars().take(160).collect(),
                }
            }
            Err(e) => ServiceStatus {
                name,
                ok: false,
                detail: e.to_string(),
            },
        },
    }
}

/// Everything, read in parallel (a few seconds at most).
pub fn report() -> Report {
    let brew = which("brew");
    let prefix = brew
        .as_ref()
        .and_then(|b| run(b, &["--prefix"]))
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from("/opt/homebrew"));
    std::thread::scope(|scope| {
        let outdated = scope.spawn(|| -> Result<HashMap<String, String>, String> {
            let brew = brew.as_ref().ok_or("Homebrew is not installed")?;
            let output = Command::new(brew)
                .args(["outdated", "--json=v2"])
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .output()
                .map_err(|e| e.to_string())?;
            Ok(parse_outdated(&String::from_utf8_lossy(&output.stdout)))
        });
        let services = scope.spawn(|| {
            vec![
                service("GitHub (gh auth)", "gh", &["auth", "status"], "signed in"),
                service(
                    "Docker / OrbStack",
                    "docker",
                    &["info", "--format", "{{.ServerVersion}}"],
                    "running",
                ),
                service("Ollama", "ollama", &["list"], "running"),
                service(
                    "tmux server (fubbik)",
                    "tmux",
                    &["-L", crate::tmux::SERVER, "has-session"],
                    "running",
                ),
            ]
        });
        let probes: Vec<_> = TOOLS
            .iter()
            .map(|spec| {
                let prefix = &prefix;
                scope.spawn(move || {
                    if spec.bin.is_empty() {
                        // A library: installed means its Cellar folder exists.
                        let formula = spec.brew.unwrap_or(spec.name);
                        let version = cellar_version(prefix, formula);
                        return ToolStatus {
                            name: spec.name,
                            why: spec.why,
                            brew: spec.brew,
                            path: version.as_ref().map(|_| prefix.join("opt").join(formula)),
                            version,
                            outdated: None,
                            brewed_at: brewed_at(prefix, formula),
                        };
                    }
                    let path = which(spec.bin);
                    let version = path
                        .as_ref()
                        .and_then(|p| run(p, spec.version_args))
                        .and_then(|text| version_in(&text));
                    // Homebrew's facts only for Homebrew's copy: a node from
                    // mise is not upgraded by `brew upgrade node`.
                    let from_brew = |p: &PathBuf| {
                        p.starts_with(prefix)
                            || std::fs::canonicalize(p).is_ok_and(|real| real.starts_with(prefix))
                    };
                    let brew = match &path {
                        Some(p) if !from_brew(p) => None,
                        _ => spec.brew,
                    };
                    let brew = match (spec.brew, &path) {
                        // A cask installs outside the prefix (OrbStack's docker).
                        (Some(b), Some(_)) if b.starts_with("cask:") => Some(b),
                        _ => brew,
                    };
                    ToolStatus {
                        name: spec.name,
                        why: spec.why,
                        brew,
                        path,
                        version,
                        outdated: None,
                        brewed_at: brew.and_then(|b| brewed_at(prefix, b)),
                    }
                })
            })
            .collect();
        let mut tools: Vec<ToolStatus> = probes.into_iter().filter_map(|p| p.join().ok()).collect();
        let (outdated, brew_error) = match outdated.join() {
            Ok(Ok(map)) => (map, None),
            Ok(Err(e)) => (HashMap::new(), Some(e)),
            Err(_) => (HashMap::new(), Some("brew outdated crashed".into())),
        };
        for tool in &mut tools {
            tool.outdated = tool.brew.and_then(|b| outdated.get(b).cloned());
        }
        Report {
            tools,
            services: services.join().unwrap_or_default(),
            brew_error,
        }
    })
}

/// "today", "3 days ago": for "upgraded …".
pub fn age(at: SystemTime, now: SystemTime) -> String {
    let days = now.duration_since(at).unwrap_or(Duration::ZERO).as_secs() / 86_400;
    match days {
        0 => "today".into(),
        1 => "yesterday".into(),
        n => format!("{n} days ago"),
    }
}

/// The shell command that upgrades (or installs) a tool through Homebrew.
pub fn brew_command(brew: &str, install: bool) -> String {
    let verb = if install { "install" } else { "upgrade" };
    match brew.strip_prefix("cask:") {
        Some(cask) => format!("brew {verb} --cask {cask}"),
        None => format!("brew {verb} {brew}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_versions_in_tool_output() {
        // Given
        let outputs = [
            "tmux 3.8",
            "gh version 2.103.3 (2026-10-26)\nhttps://github.com/cli/cli/releases/tag/v2.103.3",
            "v22.11.0",
            "cargo 1.93.0 (083ac5135 2025-12-15)",
            "Docker version 28.5.1, build e180ab8",
            "git version 2.51.0",
            "no version here",
        ];

        // When
        let versions: Vec<Option<String>> = outputs.iter().map(|o| version_in(o)).collect();

        // Then
        assert_eq!(
            versions,
            [
                Some("3.8".to_string()),
                Some("2.103.3".to_string()),
                Some("22.11.0".to_string()),
                Some("1.93.0".to_string()),
                Some("28.5.1".to_string()),
                Some("2.51.0".to_string()),
                None,
            ]
        );
    }

    #[test]
    fn parses_brew_outdated_formulae_and_casks() {
        // Given
        let json = r#"{"formulae": [{"name": "tmux", "installed_versions": ["3.7"], "current_version": "3.8"}],
                       "casks": [{"name": "orbstack", "installed_versions": "1.9", "current_version": "2.0"}]}"#;

        // When
        let outdated = parse_outdated(json);

        // Then
        assert_eq!(outdated.get("tmux").map(String::as_str), Some("3.8"));
        assert_eq!(
            outdated.get("cask:orbstack").map(String::as_str),
            Some("2.0")
        );
        assert!(parse_outdated("not json").is_empty());
    }

    #[test]
    fn brew_commands_and_ages() {
        // Given
        let now = SystemTime::now();

        // When
        let formula = brew_command("tmux", false);
        let cask = brew_command("cask:orbstack", true);
        let ages = [
            age(now, now),
            age(now - Duration::from_secs(86_400 + 60), now),
            age(now - Duration::from_secs(5 * 86_400), now),
        ];

        // Then
        assert_eq!(formula, "brew upgrade tmux");
        assert_eq!(cask, "brew install --cask orbstack");
        assert_eq!(ages, ["today", "yesterday", "5 days ago"]);
    }
}

#[cfg(test)]
mod live {
    /// Prints this machine's report (`cargo test print_report -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_report() {
        // Given / When
        let started = std::time::Instant::now();
        let report = super::report();

        // Then
        for tool in &report.tools {
            println!(
                "{:8} {:12} outdated={:?} path={:?} brewed={:?}",
                tool.name,
                tool.version.clone().unwrap_or_default(),
                tool.outdated,
                tool.path,
                tool.brewed_at
                    .map(|t| super::age(t, std::time::SystemTime::now()))
            );
        }
        for service in &report.services {
            println!("{} ok={} {}", service.name, service.ok, service.detail);
        }
        println!(
            "brew_error={:?} in {:?}",
            report.brew_error,
            started.elapsed()
        );
    }
}
