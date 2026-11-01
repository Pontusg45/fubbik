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
        let repos = self.repos.iter().map(repo_json).collect();
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

fn repo_json(view: &RepoView) -> Value {
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
        .map(|d| json!({ "name": d.spec.name, "engine": d.spec.engine, "status": d.status.label(), "up": d.status.is_up() }))
        .collect();
    let worktrees: Vec<Value> = match &snapshot.worktrees {
        Some(Ok(list)) => list
            .iter()
            .map(|w| {
                json!({
                    "path": w.path, "branch": w.branch, "head": w.head, "main": w.main, "missing": w.missing,
                    "changed_files": w.changed_files, "ahead": w.ahead, "behind": w.behind,
                })
            })
            .collect(),
        _ => Vec::new(),
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
    })
}
