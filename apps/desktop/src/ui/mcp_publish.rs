//! The JSON view of the workspace that the MCP server answers from
//! (`crate::mcp`), published after every poll.

use std::time::SystemTime;

use serde_json::{Value, json};

use super::today::Section;
use super::*;

fn seconds_ago(at: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(at)
        .map_or(0, |d| d.as_secs())
}

impl Workspace {
    pub(super) fn publish_for_mcp(&self) {
        let repos = self
            .repos
            .iter()
            .map(|view| {
                let mut repo = repo_json(view, &self.supervisor);
                repo["tests"] = self.tests_json(&view.entry.path);
                repo
            })
            .collect();
        let today = self
            .today_rows()
            .into_iter()
            .map(|row| {
                json!({
                    "section": match row.section {
                        Section::Needs => "needs_you",
                        Section::Flight => "in_flight",
                        Section::Next => "next_up",
                    },
                    "repo": row.repo,
                    "tag": row.tag.map(|(tag, _)| tag),
                    "title": row.title,
                    "detail": row.detail,
                })
            })
            .collect();
        crate::mcp::publish(repos, today);
    }
}

fn repo_json(view: &RepoView, supervisor: &crate::supervisor::Supervisor) -> Value {
    let name = view.entry.name();
    let path = view.entry.path.display().to_string();
    let Some(snapshot) = &view.snapshot else {
        return json!({
            "name": name, "path": path,
            "summary": { "name": name, "path": path, "status": "not polled yet" },
        });
    };
    let git = match &snapshot.git {
        Some(Ok(git)) => json!({
            "branch": git.branch, "head": git.head, "changed_files": git.changed_files,
            "ahead": git.ahead, "behind": git.behind, "last_commit": git.last_commit,
        }),
        Some(Err(error)) => json!({ "error": error }),
        None => Value::Null,
    };
    let processes: Vec<Value> = snapshot
        .processes
        .iter()
        .map(|p| {
            json!({
                "name": p.spec.name,
                "command": p.spec.command,
                "cwd": p.spec.cwd(&view.entry.path),
                "key": p.key,
                "tmux_session": supervisor.tmux_session(&p.key),
                "status": p.status.label(),
                "up": p.status.is_up(),
                "port": p.spec.port,
                "pid": p.pid,
                "started_by_app": p.owned,
                "memory_mb": p.rss_kb.map(|kb| kb / 1024),
            })
        })
        .collect();
    let databases: Vec<Value> = snapshot
        .databases
        .iter()
        .map(|d| json!({ "name": d.spec.name, "engine": d.spec.engine, "url": d.spec.url, "status": d.status.label(), "up": d.status.is_up() }))
        .collect();
    let prs = match &view.prs.by_branch {
        Some(Ok(map)) => Some(map),
        _ => None,
    };
    let checked_out: Vec<String> = match &snapshot.worktrees {
        Some(Ok(list)) => list.iter().filter_map(|w| w.branch.clone()).collect(),
        _ => Vec::new(),
    };
    let pr_json = |branch: &str, pr: &crate::github::PrStatus| {
        json!({
            "branch": branch,
            "number": pr.number,
            "state": pr.state,
            "draft": pr.draft,
            "url": pr.url,
            "review": pr.review,
            "checks": { "passed": pr.checks.passed, "failed": pr.checks.failed, "pending": pr.checks.pending },
            "conflicting": pr.conflicting,
            "needs_you": pr.needs_you(),
            "ready_to_merge": pr.ready(),
            "summary": pr.summary(),
            "checked_out": checked_out.iter().any(|b| b == branch),
        })
    };
    let worktrees: Vec<Value> = match &snapshot.worktrees {
        Some(Ok(list)) => list
            .iter()
            .map(|w| {
                let name = w.name();
                json!({
                    "path": w.path, "branch": w.branch, "head": w.head, "main": w.main, "missing": w.missing,
                    "changed_files": w.changed_files, "ahead": w.ahead, "behind": w.behind,
                    "merged": w.merged(),
                    "sessions": snapshot.sessions.iter().filter(|s| s.cwd.starts_with(&w.path)).map(|s| json!({
                        "agent": s.agent.label(), "title": s.title, "state": s.state.label(),
                    })).collect::<Vec<_>>(),
                    "overlaps": snapshot.overlaps.iter().filter(|o| o.a == name || o.b == name).map(|o| json!({
                        "with": if o.a == name { &o.b } else { &o.a }, "files": o.files,
                    })).collect::<Vec<_>>(),
                    "pull_request": w.branch.as_deref().and_then(|b| Some(pr_json(b, prs?.get(b)?))),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    // Null until gh has answered once (the change feed waits for a baseline).
    let pull_requests: Value = match prs {
        Some(map) => {
            let mut list: Vec<(&String, &crate::github::PrStatus)> = map.iter().collect();
            list.sort_by_key(|(_, pr)| std::cmp::Reverse(pr.number));
            Value::Array(list.into_iter().map(|(b, pr)| pr_json(b, pr)).collect())
        }
        None => Value::Null,
    };
    let sessions: Vec<Value> = snapshot
        .sessions
        .iter()
        .map(|s| {
            json!({
                "agent": s.agent.label(),
                "id": s.id,
                "title": s.title,
                "state": s.state.label(),
                "live": s.state.is_live(),
                "cwd": s.cwd,
                "branch": s.branch,
                "pid": s.pid,
                "tokens": s.tokens,
                "idle_for_seconds": seconds_ago(s.last_activity),
                "in_tmux": s.pane.is_some(),
            })
        })
        .collect();
    let overlaps: Vec<Value> = snapshot
        .overlaps
        .iter()
        .map(|o| json!({ "a": o.a, "b": o.b, "files": o.files }))
        .collect();
    let scripts: Vec<Value> = snapshot
        .scripts
        .iter()
        .map(|s| json!({ "tool": s.tool, "name": s.name, "command": s.command, "group": s.group }))
        .collect();
    let knowledge = match &view.knowledge {
        Some(Ok(k)) => json!({
            "space": k.space.as_ref().map(|s| json!({ "id": s.id, "name": s.name })),
            "chunks": k.chunks,
            "stale": k.stale,
            "orphans": k.orphans,
            "requirements": { "total": k.requirements.total, "passing": k.requirements.passing, "failing": k.requirements.failing, "untested": k.requirements.untested },
            "open_plans": k.plan_details.iter().map(|p| json!({
                "id": p.id, "title": p.title, "status": p.status,
                "tasks": p.tasks.iter().map(|t| json!({ "id": t.id, "title": t.title, "status": t.status })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "proposals_pending": k.proposals_pending,
        }),
        Some(Err(error)) => json!({ "error": error }),
        None => Value::Null,
    };
    let live = snapshot
        .sessions
        .iter()
        .filter(|s| s.state.is_live())
        .count();
    let summary = json!({
        "name": name,
        "path": path,
        "group": view.entry.group,
        "status": snapshot.status().label(),
        "missing": snapshot.missing,
        "branch": snapshot.git.as_ref().and_then(|g| g.as_ref().ok()).map(|g| g.branch.clone()),
        "changed_files": snapshot.git.as_ref().and_then(|g| g.as_ref().ok()).map(|g| g.changed_files),
        "processes_up": processes.iter().filter(|p| p["up"] == true).count(),
        "processes": processes.len(),
        "live_sessions": live,
        "worktrees": worktrees.len(),
        "overlaps": overlaps.len(),
    });
    json!({
        "name": name,
        "path": path,
        "summary": summary,
        "status": snapshot.status().label(),
        "services_error": snapshot.services_error,
        "git": git,
        "remote": snapshot.remote,
        "processes": processes,
        "databases": databases,
        "worktrees": worktrees,
        "sessions": sessions,
        "overlaps": overlaps,
        "scripts": scripts,
        "knowledge": knowledge,
        "pull_requests": pull_requests,
        "pull_requests_error": match &view.prs.by_branch {
            Some(Err(error)) => Some(error.clone()),
            _ => None,
        },
    })
}

impl Workspace {
    /// The repo's last test run per runner, for `get_test_results`.
    fn tests_json(&self, repo: &std::path::Path) -> Value {
        let runs: Vec<Value> = self
            .tests
            .runs
            .iter()
            .filter(|((path, _), _)| path == repo)
            .map(|((_, runner), run)| {
                json!({
                    "runner": runner.label(),
                    "running": run.running.is_some(),
                    "finished_seconds_ago": run.finished.map(|at| at.elapsed().as_secs()),
                    "only_failed_rerun": run.partial,
                    "report": run.report.as_ref().map(|r| json!({
                        "passed": r.passed,
                        "failed": r.failed,
                        "ignored": r.ignored,
                        "error": r.error,
                        "failures": r.failures.iter().map(|f| json!({
                            "name": f.name,
                            "message": f.message,
                            "file": f.file,
                            "location": f.location().map(|(file, line)| format!("{file}:{line}")),
                        })).collect::<Vec<_>>(),
                    })),
                })
            })
            .collect();
        Value::Array(runs)
    }
}
