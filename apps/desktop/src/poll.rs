//! One poll of a repo: read its services file and git state, and probe every
//! process and database. Blocking; runs on the background executor.

use crate::config::RepoEntry;
use crate::model::{CPU_HISTORY, DatabaseState, ProcessState, RepoSnapshot, Status, process_key};
use crate::services::{DatabaseSpec, ProcessSpec, ServicesFile};
use crate::sessions::SessionIndex;
use crate::supervisor::Supervisor;
use crate::{git, probe};

pub fn snapshot(entry: &RepoEntry, sup: &Supervisor, prev: Option<&RepoSnapshot>) -> RepoSnapshot {
    let repo = &entry.path;
    if !repo.is_dir() {
        return RepoSnapshot {
            missing: true,
            ..Default::default()
        };
    }
    let mut snapshot = RepoSnapshot {
        git: Some(git::read(repo)),
        remote: git::remote(repo),
        ..Default::default()
    };
    let services = match ServicesFile::load(repo) {
        Ok(Some(services)) => {
            snapshot.has_services_file = true;
            services
        }
        Ok(None) => return snapshot,
        Err(error) => {
            snapshot.has_services_file = true;
            snapshot.services_error = Some(error);
            return snapshot;
        }
    };
    snapshot.processes = services
        .process
        .into_iter()
        .map(|spec| {
            let key = process_key(repo, &spec.name);
            let history = prev
                .and_then(|p| p.processes.iter().find(|s| s.key == key))
                .map(|s| s.cpu_history.clone())
                .unwrap_or_default();
            process_state(spec, key, sup, history)
        })
        .collect();
    snapshot.databases = services
        .database
        .into_iter()
        .map(|spec| DatabaseState {
            status: database_status(repo, &spec),
            spec,
        })
        .collect();
    snapshot
}

fn process_state(
    spec: ProcessSpec,
    key: String,
    sup: &Supervisor,
    mut cpu_history: Vec<f32>,
) -> ProcessState {
    let owned_pid = sup.pid(&key);
    let listening = spec.port.is_some_and(probe::port_listening);
    let healthy = spec.health_url.as_deref().is_none_or(probe::http_ok);
    let (status, pid) = match owned_pid {
        Some(pid) if !healthy && listening => (Status::Degraded, Some(pid)),
        Some(pid) => (Status::Running, Some(pid)),
        None if listening => {
            let owner = spec.port.and_then(probe::port_owner);
            (
                if healthy {
                    Status::External
                } else {
                    Status::Degraded
                },
                owner,
            )
        }
        None => (Status::Stopped, None),
    };
    let stats = pid.and_then(probe::process_stats);
    match stats {
        Some((cpu, _)) => {
            cpu_history.push(cpu);
            if cpu_history.len() > CPU_HISTORY {
                cpu_history.remove(0);
            }
        }
        None => cpu_history.clear(),
    }
    ProcessState {
        spec,
        key,
        status,
        pid,
        owned: owned_pid.is_some(),
        cpu_history,
        rss_kb: stats.map(|(_, rss)| rss),
    }
}

fn database_status(repo: &std::path::Path, spec: &DatabaseSpec) -> Status {
    let compose = spec
        .compose_service
        .as_deref()
        .and_then(|service| probe::compose_running(repo, service));
    if compose == Some(true) || probe::tcp_reachable(&spec.url) {
        Status::Running
    } else {
        Status::Stopped
    }
}

/// Worktrees (re-read when `refresh_worktrees`, else carried over from the
/// previous poll) and the AI sessions inside the repo or any worktree.
pub fn agents(
    entry: &RepoEntry,
    snapshot: &mut RepoSnapshot,
    prev: Option<&RepoSnapshot>,
    index: &std::sync::Mutex<SessionIndex>,
    refresh_worktrees: bool,
) {
    if snapshot.missing {
        return;
    }
    snapshot.worktrees = match prev.and_then(|p| p.worktrees.clone()) {
        Some(previous) if !refresh_worktrees => Some(previous),
        _ => Some(crate::worktree::read(&entry.path)),
    };
    let mut roots = vec![entry.path.clone()];
    for path in snapshot.worktree_paths() {
        if !roots.iter().any(|root| path.starts_with(root)) {
            roots.push(path);
        }
    }
    snapshot.sessions = index.lock().unwrap().scan(&roots);
    attach_panes(&mut snapshot.sessions);
}

/// Preview lines kept per live session.
const PREVIEW_LINES: usize = 8;

/// Finds the tmux pane each live session runs in and captures its screen.
/// A Claude session (with a pid) is in the pane whose shell is one of its
/// ancestors; a Codex session (no pid) in a pane running `codex` in its cwd.
fn attach_panes(sessions: &mut [crate::sessions::AiSession]) {
    if !sessions.iter().any(|s| s.state.is_live()) {
        return;
    }
    let panes = crate::tmux::all_panes();
    if !panes.is_empty() {
        match_panes(sessions, &panes);
    }
}

fn match_panes(sessions: &mut [crate::sessions::AiSession], panes: &[crate::tmux::Pane]) {
    for session in sessions.iter_mut().filter(|s| s.state.is_live()) {
        let pane = match session.pid {
            Some(pid) => {
                let mut lineage = vec![pid];
                lineage.extend(crate::tmux::ancestors(pid));
                panes.iter().find(|p| lineage.contains(&p.pid))
            }
            None => panes
                .iter()
                .find(|p| p.command.contains("codex") && p.path == session.cwd),
        };
        if let Some(pane) = pane {
            session.preview = crate::tmux::capture(pane, PREVIEW_LINES);
            session.pane = Some(pane.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_session_is_matched_to_its_tmux_pane_with_a_preview() {
        if !crate::tmux::available() {
            return;
        }
        // Given: an "agent" process two levels below a pane's shell, on a private server
        let server =
            crate::tmux::Server::Named(format!("fubbik-test-match-{}", std::process::id()));
        let dir = tempfile::tempdir().unwrap();
        crate::tmux::create(
            &server,
            "agent",
            dir.path(),
            "Claude",
            Some("echo agent-ready; sh -c 'sleep 30'"),
        )
        .unwrap();
        let panes = crate::tmux::list_panes(&server);
        let shell = panes[0].pid;
        let mut agent = None;
        for _ in 0..100 {
            agent = std::process::Command::new("pgrep")
                .args(["-f", "sleep 30"])
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .filter_map(|l| l.trim().parse::<u32>().ok())
                        .find(|pid| crate::tmux::ancestors(*pid).contains(&shell))
                });
            if agent.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut sessions = vec![crate::sessions::AiSession {
            agent: crate::sessions::Agent::ClaudeCode,
            id: "s1".into(),
            title: "t".into(),
            cwd: dir.path().to_path_buf(),
            branch: None,
            state: crate::sessions::SessionState::Busy,
            pid: agent,
            last_activity: std::time::SystemTime::now(),
            pane: None,
            preview: None,
            tokens: None,
        }];

        // When
        match_panes(&mut sessions, &panes);
        crate::tmux::kill_server(&server);

        // Then
        assert!(agent.is_some(), "the agent process started");
        assert_eq!(
            sessions[0].pane.as_ref().map(|p| p.session.as_str()),
            Some("agent")
        );
        assert!(
            sessions[0]
                .preview
                .as_deref()
                .unwrap_or_default()
                .contains("agent-ready")
        );
    }

    #[test]
    fn missing_repo_snapshot() {
        // Given
        let entry = RepoEntry {
            path: "/definitely/not/here".into(),
            group: None,
        };

        // When
        let snapshot = snapshot(&entry, &Supervisor::new(), None);

        // Then
        assert!(snapshot.missing);
        assert_eq!(snapshot.status(), Status::Error);
    }

    #[test]
    fn bad_services_file_reports_error() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(crate::services::FILE_NAME),
            "[[process]]\nname = 1\n",
        )
        .unwrap();
        let entry = RepoEntry {
            path: dir.path().to_path_buf(),
            group: None,
        };

        // When
        let snapshot = snapshot(&entry, &Supervisor::new(), None);

        // Then
        assert!(snapshot.services_error.is_some());
        assert_eq!(snapshot.status(), Status::Error);
    }

    #[test]
    fn external_listener_is_reported_external() {
        // Given
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(crate::services::FILE_NAME),
            format!("[[process]]\nname = \"api\"\ncommand = \"x\"\nport = {port}\n"),
        )
        .unwrap();
        let entry = RepoEntry {
            path: dir.path().to_path_buf(),
            group: None,
        };

        // When
        let snapshot = snapshot(&entry, &Supervisor::new(), None);

        // Then
        let api = &snapshot.processes[0];
        assert_eq!(api.status, Status::External);
        assert!(!api.owned);
        assert_eq!(api.pid, Some(std::process::id()));
    }

    #[test]
    fn owned_process_runs_then_stops() {
        // Given
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(crate::services::FILE_NAME),
            format!(
                "[[process]]\nname = \"web\"\ncommand = \"python3 -m http.server {port} --bind 127.0.0.1\"\nport = {port}\nhealth_url = \"http://127.0.0.1:{port}/\"\n"
            ),
        )
        .unwrap();
        let entry = RepoEntry {
            path: dir.path().to_path_buf(),
            group: None,
        };
        let sup = Supervisor::new();
        let key = crate::model::process_key(dir.path(), "web");
        let command = format!("python3 -m http.server {port} --bind 127.0.0.1");
        sup.start(&key, &command, dir.path()).unwrap();

        // When
        let mut running = None;
        for _ in 0..100 {
            let snap = snapshot(&entry, &sup, None);
            if snap.processes[0].status == Status::Running && crate::probe::port_listening(port) {
                running = Some(snap);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        sup.stop(&key);
        let stopped = snapshot(&entry, &sup, None);

        // Then
        let running = running.expect("never became Running");
        assert!(running.processes[0].owned);
        assert_eq!(running.status(), Status::Running);
        assert_eq!(stopped.processes[0].status, Status::Stopped);
        assert!(!crate::probe::port_listening(port));
    }
}
