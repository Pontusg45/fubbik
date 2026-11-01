//! The MCP tools for an agent that keeps track of all the projects: the
//! change feed, dependencies across repos, the toolchain, pull requests,
//! worktrees, and a checkout's changes and diffs. Reads only.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The toolchain check takes seconds: answered from a cache this fresh.
const TOOLCHAIN_FRESH: Duration = Duration::from_secs(120);
/// Diff lines returned at most.
const DIFF_LINES: usize = 3000;

pub fn definitions() -> Vec<Value> {
    let mut all = base_definitions();
    all.extend(crate::tracker::mcp_definitions());
    all
}

fn base_definitions() -> Vec<Value> {
    let repo =
        json!({ "type": "string", "description": "Repo name (as list_repos shows) or its path" });
    let worktree = json!({ "type": "string", "description": "A worktree's path or branch (see list_worktrees); default: the repo's main checkout" });
    vec![
        json!({
            "name": "get_changes_since",
            "description": "What changed across the projects since a cursor: commits and branch switches, processes up or down, agents started / waiting for the user / ended, worktrees added or removed, pull requests opened / merged / needing the user (failing checks, changes requested, conflicts), terminal bells, dependencies that start or stop disagreeing by a major version across repos (checked every 10 min), and tools Homebrew upgraded (every 30 min). Call without a cursor to get recent events and a cursor; pass that cursor next time to get only what is new. `dropped: true` means events between the cursor and the oldest kept one were lost (the feed keeps 2000).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cursor": { "type": "integer", "description": "The cursor from the last call" },
                    "repo": repo.clone(),
                    "limit": { "type": "integer", "description": "At most this many events (default 200, max 1000)" }
                }
            },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_dependencies",
            "description": "Dependencies across all registered repos, from package.json (pnpm catalogs resolved), Cargo.toml and toolchain pins (packageManager, engines.node, .nvmrc, .tool-versions, mise.toml, rust-toolchain, rust-version, edition). One row per dependency used by two or more repos: each repo's declared requirement and how far it is behind the newest any repo uses (`breaking`: a major behind, 0.x minors count; `drift`: a minor behind), with the manifest files that declare it. Declared requirements, not lockfiles.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filter": { "type": "string", "enum": ["breaking", "misaligned", "shared"], "description": "breaking (default): different majors; misaligned: any difference; shared: everything used by 2+ repos" },
                    "ecosystem": { "type": "string", "enum": ["npm", "cargo", "toolchain"] },
                    "name": { "type": "string", "description": "Only this dependency (exact name)" }
                }
            },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_toolchain",
            "description": "The machine's developer tools (git, gh, tmux, just, node, pnpm, mise, bun, cargo, rustc, zig, docker, psql, ollama, claude, codex, libgit2): version, path, missing, the newer version Homebrew offers, when Homebrew last upgraded it; and services (gh signed in, Docker/OrbStack, Ollama, the app's tmux server). Cached for 2 minutes.",
            "inputSchema": { "type": "object", "properties": { "refresh": { "type": "boolean", "description": "Check again now" } } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_pull_requests",
            "description": "Pull requests per branch (GitHub repos, through the gh CLI, read every 3 min): state, draft, checks passed/failed/pending, review decision, conflicts, url, and whether one needs the user. `checked_out` marks branches with a local worktree. `errors` lists repos whose PRs could not be read (gh not signed in, for example).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo": repo.clone(),
                    "open_only": { "type": "boolean", "description": "Only open PRs (default true)" }
                }
            },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "list_worktrees",
            "description": "A repo's worktrees: path, branch, changed files, ahead/behind main, merged into main, the AI sessions in it, worktrees it overlaps (both change the same files) and its pull request.",
            "inputSchema": { "type": "object", "properties": { "repo": repo.clone() }, "required": ["repo"] },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_changes",
            "description": "A checkout's changed files right now (git status): path, staged and unstaged status letters, untracked, renamed from.",
            "inputSchema": { "type": "object", "properties": { "repo": repo.clone(), "worktree": worktree.clone() }, "required": ["repo"] },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_diff",
            "description": "One file's diff in a checkout: staged (index against HEAD) or not (work tree against the index); an untracked file shows whole.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo": repo,
                    "worktree": worktree,
                    "path": { "type": "string", "description": "File path relative to the checkout, as get_changes lists it" },
                    "staged": { "type": "boolean", "description": "The staged side (default false)" }
                },
                "required": ["repo", "path"]
            },
            "annotations": { "readOnlyHint": true }
        }),
    ]
}

fn find_repo<'a>(repos: &'a [Value], wanted: &str) -> Result<&'a Value, String> {
    repos
        .iter()
        .find(|r| r["name"] == wanted || r["path"] == wanted)
        .ok_or_else(|| {
            let names: Vec<&str> = repos.iter().filter_map(|r| r["name"].as_str()).collect();
            format!("no repo \"{wanted}\"; registered: {}", names.join(", "))
        })
}

/// The checkout to read: the repo's own, or one of its worktrees (by path or
/// branch). Only checkouts the app lists are allowed.
fn checkout(repo: &Value, worktree: Option<&str>) -> Result<PathBuf, String> {
    let root = PathBuf::from(repo["path"].as_str().unwrap_or_default());
    let Some(wanted) = worktree else {
        return Ok(root);
    };
    repo["worktrees"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|w| w["path"] == wanted || w["branch"] == wanted)
        .and_then(|w| w["path"].as_str().map(PathBuf::from))
        .ok_or_else(|| format!("no worktree \"{wanted}\" in this repo; see list_worktrees"))
}

fn toolchain(refresh: bool) -> Value {
    static CACHE: OnceLock<Mutex<Option<(Instant, Value)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if !refresh
        && let Some((at, value)) = cache.lock().unwrap().as_ref()
        && at.elapsed() < TOOLCHAIN_FRESH
    {
        return value.clone();
    }
    let report = crate::tools::report();
    let now = std::time::SystemTime::now();
    let value = json!({
        "tools": report.tools.iter().map(|t| json!({
            "name": t.name,
            "version": t.version,
            "path": t.path,
            "missing": t.path.is_none(),
            "for": t.why,
            "homebrew": t.brew,
            "upgrade_available": t.outdated,
            "homebrew_changed": t.brewed_at.map(|at| crate::tools::age(at, now)),
        })).collect::<Vec<_>>(),
        "services": report.services.iter().map(|s| json!({ "name": s.name, "ok": s.ok, "detail": s.detail })).collect::<Vec<_>>(),
        "homebrew_error": report.brew_error,
    });
    *cache.lock().unwrap() = Some((Instant::now(), value.clone()));
    value
}

fn dependencies(repos: &[Value], args: &Value) -> Result<Value, String> {
    use crate::deps::{Align, Eco};
    let paths: Vec<(String, PathBuf)> = repos
        .iter()
        .filter_map(|r| {
            Some((
                r["name"].as_str()?.to_string(),
                PathBuf::from(r["path"].as_str()?),
            ))
        })
        .collect();
    if paths.len() < 2 {
        return Err("fewer than two repos are registered: nothing to compare".into());
    }
    let scans: Vec<_> = paths.iter().map(|(_, p)| crate::deps::scan(p)).collect();
    let filter = args["filter"].as_str().unwrap_or("breaking");
    let eco = match args["ecosystem"].as_str() {
        Some("npm") => Some(Eco::Npm),
        Some("cargo") => Some(Eco::Cargo),
        Some("toolchain") => Some(Eco::Toolchain),
        Some(other) => {
            return Err(format!(
                "ecosystem must be npm, cargo or toolchain, not \"{other}\""
            ));
        }
        None => None,
    };
    let lag = |a: Align| match a {
        Align::Breaking => "breaking",
        Align::Drift => "drift",
        Align::Unknown => "unreadable",
        Align::Aligned => "current",
    };
    let rows: Vec<Value> = crate::deps::compare(&scans, 2)
        .into_iter()
        .filter(|r| match filter {
            "misaligned" => matches!(r.align, Align::Breaking | Align::Drift),
            "shared" => true,
            _ => r.align == Align::Breaking,
        })
        .filter(|r| eco.is_none_or(|e| r.eco == e))
        .filter(|r| args["name"].as_str().is_none_or(|n| r.name == n))
        .map(|r| {
            let repos: Vec<Value> = paths
                .iter()
                .enumerate()
                .filter(|(i, _)| !r.per_repo[*i].is_empty())
                .map(|(i, (name, root))| {
                    json!({
                        "repo": name,
                        "requirement": r.specs(i),
                        "status": lag(r.lag(i)),
                        "files": r.per_repo[i].iter().map(|d| json!({
                            "file": root.join(&d.file), "requirement": d.spec, "dev": d.dev,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            json!({
                "name": r.name,
                "ecosystem": r.eco.label(),
                "alignment": r.align.label(),
                "newest": r.newest.map(|(major, minor)| format!("{major}.{minor}")),
                "repos": repos,
            })
        })
        .collect();
    Ok(json!({ "repos": paths.iter().map(|(n, _)| n).collect::<Vec<_>>(), "dependencies": rows }))
}

/// Answers the tools defined here; None for any other name.
pub fn call(repos: &[Value], name: &str, args: &Value) -> Option<Result<Value, String>> {
    let arg = |key: &str| args.get(key).and_then(Value::as_str);
    let result = match name {
        "get_changes_since" => {
            let limit = args["limit"].as_u64().unwrap_or(200).clamp(1, 1000) as usize;
            let (events, cursor, dropped) =
                crate::mcp_events::since(args["cursor"].as_u64(), arg("repo"), limit);
            Ok(json!({
                "cursor": cursor,
                "dropped": dropped,
                "more": events.len() == limit,
                "events": events.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
            }))
        }
        "get_dependencies" => dependencies(repos, args),
        "get_toolchain" => Ok(toolchain(args["refresh"].as_bool().unwrap_or(false))),
        "get_pull_requests" => {
            let open_only = args["open_only"].as_bool().unwrap_or(true);
            let chosen: Result<Vec<&Value>, String> = match arg("repo") {
                Some(wanted) => find_repo(repos, wanted).map(|r| vec![r]),
                None => Ok(repos.iter().collect()),
            };
            chosen.map(|chosen| {
                // A repo whose PRs could not be read says why (gh not signed
                // in, not a GitHub repo…), so "none" and "unknown" differ.
                let errors: Vec<Value> = chosen
                    .iter()
                    .filter_map(|r| {
                        let error = r["pull_requests_error"].as_str()?;
                        Some(json!({ "repo": r["name"], "error": error }))
                    })
                    .collect();
                let prs: Vec<Value> = chosen
                    .into_iter()
                    .flat_map(|r| {
                        let name = r["name"].clone();
                        r["pull_requests"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(move |mut pr| {
                                pr["repo"] = name.clone();
                                pr
                            })
                    })
                    .filter(|pr| !open_only || pr["state"] == "OPEN")
                    .collect();
                json!({ "pull_requests": prs, "errors": errors })
            })
        }
        "list_worktrees" => arg("repo")
            .ok_or_else(|| "repo is required".to_string())
            .and_then(|wanted| find_repo(repos, wanted))
            .map(|r| r["worktrees"].clone()),
        "get_changes" => arg("repo")
            .ok_or_else(|| "repo is required".to_string())
            .and_then(|wanted| find_repo(repos, wanted))
            .and_then(|r| checkout(r, arg("worktree")))
            .and_then(|dir| {
                let files = crate::git::changes(&dir)?;
                Ok(json!({
                    "checkout": dir,
                    "files": files.iter().map(|f| json!({
                        "path": f.path,
                        "staged": f.staged.map(String::from),
                        "unstaged": f.unstaged.map(String::from),
                        "untracked": f.untracked,
                        "renamed_from": f.from,
                    })).collect::<Vec<_>>(),
                }))
            }),
        "get_diff" => (|| {
            let repo = find_repo(repos, arg("repo").ok_or("repo is required")?)?;
            let dir = checkout(repo, arg("worktree"))?;
            let path = arg("path").ok_or("path is required")?;
            // Stay inside the checkout.
            if Path::new(path).is_absolute() || path.split('/').any(|p| p == "..") {
                return Err("path must be relative to the checkout".to_string());
            }
            let staged = args["staged"].as_bool().unwrap_or(false);
            let untracked = crate::git::changes(&dir)?
                .iter()
                .any(|f| f.path == path && f.untracked);
            let diff = crate::git::diff(&dir, path, staged, untracked)?;
            let lines: Vec<&str> = diff.lines().collect();
            Ok(json!({
                "path": path,
                "staged": staged,
                "diff": lines[..lines.len().min(DIFF_LINES)].join("\n"),
                "truncated": lines.len() > DIFF_LINES,
            }))
        })(),
        _ => return crate::tracker::mcp_call(name, args),
    };
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_stay_inside_the_checkout_and_worktrees_must_be_listed() {
        // Given: one repo with one listed worktree
        let repos = vec![json!({
            "name": "app", "path": "/tmp/app",
            "worktrees": [{ "path": "/tmp/app-feat", "branch": "feat" }],
        })];

        // When
        let escape = call(
            &repos,
            "get_diff",
            &json!({ "repo": "app", "path": "../secrets" }),
        );
        let absolute = call(
            &repos,
            "get_diff",
            &json!({ "repo": "app", "path": "/etc/passwd" }),
        );
        let unknown = call(
            &repos,
            "get_changes",
            &json!({ "repo": "app", "worktree": "/tmp/other" }),
        );
        let by_branch = checkout(&repos[0], Some("feat"));

        // Then
        assert!(escape.unwrap().unwrap_err().contains("relative"));
        assert!(absolute.unwrap().unwrap_err().contains("relative"));
        assert!(unknown.unwrap().unwrap_err().contains("no worktree"));
        assert_eq!(by_branch.unwrap(), PathBuf::from("/tmp/app-feat"));
        assert!(call(&repos, "not_mine", &json!({})).is_none());
    }

    #[test]
    fn open_pull_requests_by_default() {
        // Given
        let repos = vec![json!({
            "name": "app", "path": "/x",
            "pull_requests": [
                { "branch": "a", "state": "OPEN", "number": 1 },
                { "branch": "b", "state": "MERGED", "number": 2 },
            ],
        })];

        // When
        let open = call(&repos, "get_pull_requests", &json!({}))
            .unwrap()
            .unwrap();
        let all = call(&repos, "get_pull_requests", &json!({ "open_only": false }))
            .unwrap()
            .unwrap();

        // Then
        assert_eq!(open["pull_requests"].as_array().unwrap().len(), 1);
        assert_eq!(open["pull_requests"][0]["repo"], "app");
        assert_eq!(all["pull_requests"].as_array().unwrap().len(), 2);
        assert_eq!(open["errors"], json!([]));
    }
}
