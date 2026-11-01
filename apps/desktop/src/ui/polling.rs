//! Polling: the 1 s tick, background probes, and applying their results.

use super::*;

impl Workspace {
    // ── polling ────────────────────────────────────────────────────────

    pub(super) fn on_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_multiple_of(PROBE_EVERY) {
            // While the fubbik API is down, knowledge waits out its backoff.
            let knowledge = self.tick.is_multiple_of(KNOWLEDGE_EVERY)
                && self
                    .knowledge_paused_until
                    .is_none_or(|until| Instant::now() >= until);
            self.poll_now(knowledge, cx);
        }
        self.tick += 1;
        // Redraw only for what changes with time alone; polls redraw themselves.
        let mut redraw = false;
        let before = self.toasts.len();
        // Errors stay until closed; the rest fade (alerts get longer).
        self.toasts.retain(|t| {
            t.error
                || t.at.elapsed()
                    < if t.action.is_some() {
                        TOAST_FOR * 3
                    } else {
                        TOAST_FOR
                    }
        });
        if self.toasts.len() != before {
            redraw = true;
        }
        for message in crate::backend::take_notices() {
            self.show_info(message);
            redraw = true;
        }
        let backend = crate::backend::status();
        if backend != self.backend_seen {
            if let crate::backend::Status::Failed(error) = &backend {
                self.show_error(format!(
                    "Built-in server failed: {error} (Logs → built-in server)"
                ));
            }
            self.backend_seen = backend;
            redraw = true;
        }
        // The computer's stats change every sample.
        if self.home {
            redraw = true;
        }
        if self.tab == Tab::Logs && self.log_paused.is_none() {
            if self.log_follow {
                self.log_scroll.scroll_to_bottom();
            }
            redraw = true;
        }
        if redraw {
            cx.notify();
        }
    }

    /// Re-polls as soon as a repo's top-level files (its services file among
    /// them) or Claude's live-session registry change, instead of waiting for
    /// the next tick. Called again when repos are added or removed.
    pub(super) fn watch_files(&mut self, cx: &mut Context<Self>) {
        use notify::{RecursiveMode, Watcher};
        let (tx, rx) = async_channel::unbounded::<()>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|e| !e.kind.is_access()) {
                let _ = tx.try_send(());
            }
        });
        let Ok(mut watcher) = watcher else {
            return; // polling still covers everything
        };
        let claude_sessions = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".claude/sessions");
        let dirs = self
            .repos
            .iter()
            .map(|r| r.entry.path.clone())
            .chain(std::iter::once(claude_sessions));
        for dir in dirs.filter(|d| d.is_dir()) {
            let _ = watcher.watch(&dir, RecursiveMode::NonRecursive);
        }
        self._watcher = Some(watcher);
        self._watch_task = cx.spawn(async move |this, cx| {
            while rx.recv().await.is_ok() {
                // Let a burst of writes settle, then poll once for all of them.
                cx.background_executor().timer(WATCH_DEBOUNCE).await;
                while rx.try_recv().is_ok() {}
                if this
                    .update(cx, |this, cx| this.poll_now(false, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    /// After a slow poll: back off knowledge fetches while the API is down
    /// (60 s, doubling to 5 min), and resume as soon as it answers.
    pub(super) fn note_api_status(&mut self) {
        match self.global.as_ref().map(|g| g.api.is_ok()) {
            Some(true) => {
                self.api_backoff = Duration::ZERO;
                self.knowledge_paused_until = None;
            }
            Some(false) => {
                self.api_backoff = (self.api_backoff * 2).clamp(API_BACKOFF_MIN, API_BACKOFF_MAX);
                self.knowledge_paused_until = Some(Instant::now() + self.api_backoff);
            }
            None => {}
        }
    }

    /// Probes every repo on the background executor. A call while a poll is
    /// running queues one more poll right after it.
    pub(super) fn poll_now(&mut self, with_knowledge: bool, cx: &mut Context<Self>) {
        if self.polling {
            self.poll_again = true;
            return;
        }
        self.polling = true;
        let jobs: Vec<(RepoEntry, Option<RepoSnapshot>)> = self
            .repos
            .iter()
            .map(|r| (r.entry.clone(), r.snapshot.clone()))
            .collect();
        let supervisor = self.supervisor.clone();
        let client = self.client.clone();
        let index = self.session_index.clone();
        let refresh_worktrees = with_knowledge || std::mem::take(&mut self.refresh_worktrees);
        let work = cx.background_executor().spawn(async move {
            jobs.into_iter()
                .map(|(entry, prev)| {
                    let mut snapshot = poll::snapshot(&entry, &supervisor, prev.as_ref());
                    poll::agents(
                        &entry,
                        &mut snapshot,
                        prev.as_ref(),
                        &index,
                        refresh_worktrees,
                    );
                    let knowledge = (with_knowledge && !snapshot.missing)
                        .then(|| fetch_knowledge(&client, &entry, snapshot.remote.as_deref()));
                    let setup = (with_knowledge && !snapshot.missing).then(|| {
                        let space = knowledge
                            .as_ref()
                            .and_then(|k| k.as_ref().ok())
                            .and_then(|k| k.space.as_ref())
                            .map(|s| s.id.clone());
                        read_setup(&client, &entry, &snapshot, space.as_deref())
                    });
                    PollResult {
                        path: entry.path,
                        snapshot,
                        knowledge,
                        setup,
                    }
                })
                .collect::<Vec<_>>()
        });
        let global = with_knowledge.then(|| {
            let client = self.client.clone();
            let configured = self.config.fubbik_bin.clone();
            let repos: Vec<PathBuf> = self.repos.iter().map(|r| r.entry.path.clone()).collect();
            cx.background_executor().spawn(async move {
                GlobalStatus {
                    api: client
                        .health()
                        .and_then(|health| Ok((health, client.capabilities()?))),
                    fubbik_bin: setup::resolve_fubbik_bin(configured.as_deref(), &repos),
                }
            })
        });
        cx.spawn(async move |this, cx| {
            let results = work.await;
            let global = match global {
                Some(task) => Some(task.await),
                None => None,
            };
            this.update(cx, |this, cx| {
                if global.is_some() {
                    this.global = global;
                    this.note_api_status();
                }
                this.apply(results, cx);
                this.polling = false;
                if std::mem::take(&mut this.poll_again) {
                    this.poll_now(false, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn apply(&mut self, results: Vec<PollResult>, cx: &mut Context<Self>) {
        let mut alerts = Vec::new();
        let knowledge_arrived = results.iter().any(|r| r.knowledge.is_some());
        for mut result in results {
            let Some(index) = self.repos.iter().position(|r| r.entry.path == result.path) else {
                continue; // removed while polling
            };
            detect_quiet(&mut self.quiet, &mut result.snapshot.sessions);
            let repo = &mut self.repos[index];
            if let Some(prev) = &repo.snapshot {
                alerts.extend(notify_changes(
                    index,
                    &repo.entry,
                    prev,
                    &result.snapshot,
                    &mut self.user_stopped,
                ));
            }
            repo.snapshot = Some(result.snapshot);
            if let Some(knowledge) = result.knowledge {
                repo.knowledge = Some(knowledge);
            }
            if let Some(setup) = result.setup {
                repo.setup = Some(setup);
            }
        }
        if let Some(alert) = alerts.pop() {
            self.show_alert(alert);
        }
        if knowledge_arrived {
            self.close_merged_tasks(cx);
        }
        self.drain_launch_queue(cx);
        self.report_test_exits(cx);
        self.publish_for_mcp();
        self.refresh_prs(false, cx);
    }
}

/// Polls of an unchanged screen after which a working agent counts as waiting.
const QUIET_POLLS: u32 = 3;

/// Agents without a status registry (Codex): a screen that changed and then
/// stayed still for `QUIET_POLLS` polls means the agent is waiting for input.
fn detect_quiet(quiet: &mut HashMap<String, Quiet>, sessions: &mut [crate::sessions::AiSession]) {
    for session in sessions.iter_mut() {
        if session.agent != crate::sessions::Agent::Codex || !session.state.is_live() {
            continue;
        }
        let Some(preview) = &session.preview else {
            continue;
        };
        let entry = quiet.entry(session.id.clone()).or_default();
        if entry.preview == *preview {
            entry.unchanged_polls += 1;
        } else {
            entry.was_working |= !entry.preview.is_empty();
            entry.unchanged_polls = 0;
            entry.preview = preview.clone();
        }
        if entry.was_working && entry.unchanged_polls >= QUIET_POLLS {
            session.state = SessionState::Idle;
        }
    }
}

/// MCP and hook state, and each worktree's CLAUDE.md against the generated one.
pub(super) fn read_setup(
    client: &FubbikClient,
    entry: &RepoEntry,
    snapshot: &RepoSnapshot,
    space_id: Option<&str>,
) -> RepoSetup {
    let mut roots = snapshot.worktree_paths();
    if roots.is_empty() {
        roots.push(entry.path.clone());
    }
    let claude_md = claude_md::read_config(&entry.path).map(|config| {
        let generated = client.claude_md(&config.tag, config.max_tokens, space_id);
        let worktrees = match &generated {
            Ok(text) => roots
                .iter()
                .map(|root| (root.clone(), claude_md::state(root, &config, text)))
                .collect(),
            Err(_) => Vec::new(),
        };
        ClaudeMdSync {
            config,
            generated,
            worktrees,
        }
    });
    RepoSetup {
        mcp: setup::mcp_status(&entry.path, &setup::claude_json()),
        hooks: roots
            .iter()
            .map(|root| (root.clone(), setup::hook_status(root)))
            .collect(),
        claude_md,
    }
}

pub(super) fn fetch_knowledge(
    client: &FubbikClient,
    entry: &RepoEntry,
    remote: Option<&str>,
) -> Result<Knowledge, String> {
    match client.detect_space(remote, &entry.path)? {
        Some(space) => client.knowledge(space),
        None => Ok(Knowledge::default()),
    }
}

/// A notification when a process goes down on its own, or starts failing
/// its health check.
pub(super) fn notify_changes(
    index: usize,
    entry: &RepoEntry,
    prev: &RepoSnapshot,
    next: &RepoSnapshot,
    user_stopped: &mut HashSet<String>,
) -> Vec<Alert> {
    let mut alerts = Vec::new();
    let mut alert = |title: &str, body: String, tab: Tab| {
        launch::notify(title, &body);
        alerts.push(Alert {
            message: format!("{title}: {body} — click to open"),
            target: Command::Focus { repo: index, tab },
        });
    };
    for process in &next.processes {
        let Some(before) = prev.processes.iter().find(|p| p.key == process.key) else {
            continue;
        };
        let repo = entry.name();
        let name = &process.spec.name;
        match (before.status, process.status) {
            (Status::Running | Status::External | Status::Degraded, Status::Stopped) => {
                if !user_stopped.remove(&process.key) {
                    alert(&repo, format!("{name} stopped"), Tab::Processes);
                }
            }
            (Status::Running | Status::External, Status::Degraded) => {
                alert(
                    &repo,
                    format!("{name} is failing its health check"),
                    Tab::Processes,
                );
            }
            _ => {}
        }
    }
    for session in &next.sessions {
        let was_working = prev.sessions.iter().any(|s| {
            s.id == session.id && matches!(s.state, SessionState::Busy | SessionState::Active)
        });
        if was_working && session.state == SessionState::Idle {
            alert(
                &format!("{} — {}", session.agent.label(), entry.name()),
                format!("Waiting for you: {}", session.title),
                Tab::Sessions,
            );
        }
    }
    alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{Agent, AiSession};

    fn codex(preview: &str) -> AiSession {
        AiSession {
            agent: Agent::Codex,
            id: "c1".into(),
            title: "t".into(),
            cwd: "/code".into(),
            branch: None,
            state: SessionState::Active,
            pid: None,
            last_activity: std::time::SystemTime::now(),
            pane: None,
            preview: Some(preview.into()),
            tokens: None,
        }
    }

    #[test]
    fn a_codex_screen_that_stops_changing_is_waiting() {
        // Given
        let mut quiet = HashMap::new();
        let mut states = Vec::new();

        // When: working (the screen changes), then still for three polls
        for preview in [
            "thinking.",
            "thinking..",
            "done?",
            "done?",
            "done?",
            "done?",
        ] {
            let mut sessions = [codex(preview)];
            detect_quiet(&mut quiet, &mut sessions);
            states.push(sessions[0].state);
        }

        // Then
        use SessionState::{Active, Idle};
        assert_eq!(states, [Active, Active, Active, Active, Active, Idle]);
    }

    #[test]
    fn a_screen_that_never_changed_is_not_called_waiting() {
        // Given: first seen already still (e.g. the app started meanwhile)
        let mut quiet = HashMap::new();
        let mut last = SessionState::Active;

        // When
        for _ in 0..6 {
            let mut sessions = [codex("idle prompt")];
            detect_quiet(&mut quiet, &mut sessions);
            last = sessions[0].state;
        }

        // Then
        assert_eq!(last, SessionState::Active);
    }
}
