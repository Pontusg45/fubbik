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
        if self
            .toast
            .as_ref()
            .is_some_and(|t| t.at.elapsed() > TOAST_FOR)
        {
            self.toast = None;
            redraw = true;
        }
        if self.tab == Tab::Logs && self.log_paused.is_none() {
            self.log_scroll.scroll_to_bottom();
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
        for result in results {
            let Some(repo) = self.repos.iter_mut().find(|r| r.entry.path == result.path) else {
                continue; // removed while polling
            };
            if let Some(prev) = &repo.snapshot {
                notify_changes(&repo.entry, prev, &result.snapshot, &mut self.user_stopped);
            }
            repo.snapshot = Some(result.snapshot);
            if let Some(knowledge) = result.knowledge {
                repo.knowledge = Some(knowledge);
            }
            if let Some(setup) = result.setup {
                repo.setup = Some(setup);
            }
        }
        self.report_test_exits(cx);
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
    entry: &RepoEntry,
    prev: &RepoSnapshot,
    next: &RepoSnapshot,
    user_stopped: &mut HashSet<String>,
) {
    for process in &next.processes {
        let Some(before) = prev.processes.iter().find(|p| p.key == process.key) else {
            continue;
        };
        let repo = entry.name();
        let name = &process.spec.name;
        match (before.status, process.status) {
            (Status::Running | Status::External | Status::Degraded, Status::Stopped) => {
                if !user_stopped.remove(&process.key) {
                    launch::notify(&repo, &format!("{name} stopped"));
                }
            }
            (Status::Running | Status::External, Status::Degraded) => {
                launch::notify(&repo, &format!("{name} is failing its health check"));
            }
            _ => {}
        }
    }
    for session in &next.sessions {
        let was_busy = prev
            .sessions
            .iter()
            .any(|s| s.id == session.id && s.state == SessionState::Busy);
        if was_busy && session.state == SessionState::Idle {
            launch::notify(
                &format!("{} — {}", session.agent.label(), entry.name()),
                &format!("Waiting for you: {}", session.title),
            );
        }
    }
}
