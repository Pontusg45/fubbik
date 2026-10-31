//! Polling: the 1 s tick, background probes, and applying their results.

use super::*;

impl Workspace {
    // ── polling ────────────────────────────────────────────────────────

    pub(super) fn on_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_multiple_of(PROBE_EVERY) {
            self.poll_now(self.tick.is_multiple_of(KNOWLEDGE_EVERY), cx);
        }
        self.tick += 1;
        if self
            .toast
            .as_ref()
            .is_some_and(|t| t.at.elapsed() > TOAST_FOR)
        {
            self.toast = None;
        }
        if self.tab == Tab::Logs && self.log_paused.is_none() {
            self.log_scroll.scroll_to_bottom();
        }
        cx.notify();
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
