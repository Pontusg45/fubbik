//! Commands: processes, repos, worktrees, databases, and `run`, which every
//! button and palette entry goes through.

use super::*;

impl Workspace {
    // ── commands ───────────────────────────────────────────────────────

    pub(super) fn start_process(
        &mut self,
        repo: usize,
        index: usize,
        target: RunTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Some(state) = view
            .snapshot
            .as_ref()
            .and_then(|s| s.processes.get(index))
            .cloned()
        else {
            return;
        };
        if state.owned {
            return;
        }
        if state.status.is_up() {
            let holder = state
                .pid
                .map(|pid| format!(" by pid {pid}"))
                .unwrap_or_default();
            let port = state
                .spec
                .port
                .map(|p| format!("port {p}"))
                .unwrap_or("its port".into());
            self.show_error(format!(
                "{} is already up: {port} is held{holder}",
                state.spec.name
            ));
            return;
        }
        self.user_stopped.remove(&state.key);
        let cwd = state.spec.cwd(&view.entry.path);
        self.launch_run(
            state.key.clone(),
            state.spec.name.clone(),
            state.spec.command.clone(),
            cwd,
            target,
            cx,
        );
    }

    /// Starts a process or script where chosen.
    pub(super) fn launch_run(
        &mut self,
        key: String,
        label: String,
        command: String,
        cwd: PathBuf,
        target: RunTarget,
        cx: &mut Context<Self>,
    ) {
        match target {
            RunTarget::Background | RunTarget::Tmux => {
                self.launched_in.remove(&key);
                self.last_exit.remove(&key);
                self.pending
                    .insert(key.clone(), (Instant::now(), "Starting…"));
            }
            RunTarget::Tab | RunTarget::TerminalApp => {
                self.launched_in.insert(key.clone(), target);
            }
        }
        match target {
            RunTarget::Background => {
                if self.supervisor.is_running(&key) {
                    self.show_error(format!(
                        "{label} is still running — stop it first (or Restart)"
                    ));
                    return;
                }
                self.log_key = Some(key.clone());
                self.show_alert(Alert {
                    message: format!("Started {label} — output in Logs"),
                    target: Command::ShowTab(Tab::Logs),
                });
                let supervisor = self.supervisor.clone();
                self.run_background(cx, "", move || {
                    supervisor.start(&key, &command, &cwd).map(|_| ())
                });
            }
            RunTarget::Tmux => {
                if !tmux::available() {
                    self.show_error("tmux is not installed (brew install tmux)");
                    return;
                }
                self.log_key = Some(key.clone());
                let session = crate::tmux_runs::session_name(&key);
                self.show_info(format!(
                    "Running in tmux session {session} — attach from any terminal: {}",
                    crate::tmux_runs::attach_command(&session)
                ));
                let supervisor = self.supervisor.clone();
                self.run_background(cx, "", move || {
                    supervisor
                        .start_tmux(&key, &command, &cwd, &label)
                        .map(|_| ())
                });
            }
            RunTarget::Tab => {
                // Chosen explicitly: a tab, even when AI sessions go to Terminal.app.
                self.open_terminal(cwd, label.clone(), Some(command), cx);
                self.tab = Tab::Terminal;
                self.focus_terminal = true;
            }
            RunTarget::TerminalApp => {
                self.run_background(cx, "Opened in Terminal.app", move || {
                    launch::run_in_terminal(&cwd, &command)
                });
            }
        }
    }

    /// Runs `what` where the saved choice says, or asks.
    pub(super) fn ask_where(&mut self, what: RunWhat, cx: &mut Context<Self>) {
        match self.config.run_in.as_deref().and_then(RunTarget::from_key) {
            Some(target) => self.run_what(what, target, cx),
            None => {
                self.run_prompt = Some(what);
                self.run_prompt_remember = false;
            }
        }
    }

    pub(super) fn run_what(&mut self, what: RunWhat, target: RunTarget, cx: &mut Context<Self>) {
        match what {
            RunWhat::Process { repo, index } => self.start_process(repo, index, target, cx),
            RunWhat::All(repo) => self.start_all(repo, target, cx),
            RunWhat::Group(repos) => {
                for repo in repos {
                    self.start_all(repo, target, cx);
                }
            }
            RunWhat::Profile { repo, profile } => self.start_profile(repo, &profile, target, cx),
            RunWhat::Script {
                repo,
                name,
                command,
            } => self.run_script(repo, name, command, target, cx),
        }
    }

    /// What `what` is, for the prompt's title.
    pub(super) fn run_what_label(&self, what: &RunWhat) -> String {
        let repo_name = |r: usize| {
            self.repos
                .get(r)
                .map(|v| v.entry.name())
                .unwrap_or_default()
        };
        match what {
            RunWhat::Process { repo, index } => {
                let process = self
                    .repos
                    .get(*repo)
                    .and_then(|v| v.snapshot.as_ref())
                    .and_then(|s| s.processes.get(*index));
                match process {
                    Some(p) => format!(
                        "{} ({}) — {}",
                        p.spec.name,
                        p.spec.command,
                        repo_name(*repo)
                    ),
                    None => repo_name(*repo),
                }
            }
            RunWhat::All(repo) => format!("every stopped process — {}", repo_name(*repo)),
            RunWhat::Group(repos) => format!(
                "every stopped process in {}",
                repos
                    .iter()
                    .map(|r| repo_name(*r))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            RunWhat::Profile { repo, profile } => {
                format!("profile {profile} — {}", repo_name(*repo))
            }
            RunWhat::Script { repo, command, .. } => format!("{command} — {}", repo_name(*repo)),
        }
    }

    pub(super) fn stop_process(&mut self, repo: usize, index: usize, cx: &mut Context<Self>) {
        let Some(state) = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .and_then(|s| s.processes.get(index))
            .cloned()
        else {
            return;
        };
        if !state.owned {
            if state.pid.is_some() {
                self.show_error(format!(
                    "{} was not started here; use \"Kill\" to stop the external process",
                    state.spec.name
                ));
            }
            return;
        }
        self.user_stopped.insert(state.key.clone());
        self.pending
            .insert(state.key.clone(), (Instant::now(), "Stopping…"));
        let supervisor = self.supervisor.clone();
        self.run_background(cx, "", move || {
            supervisor.stop(&state.key);
            Ok(())
        });
    }

    pub(super) fn restart_process(&mut self, repo: usize, index: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Some(state) = view
            .snapshot
            .as_ref()
            .and_then(|s| s.processes.get(index))
            .cloned()
        else {
            return;
        };
        if !state.owned {
            self.ask_where(RunWhat::Process { repo, index }, cx);
            return;
        }
        let supervisor = self.supervisor.clone();
        let cwd = state.spec.cwd(&view.entry.path);
        self.user_stopped.insert(state.key.clone());
        self.run_background(cx, "", move || {
            supervisor
                .restart(&state.key, &state.spec.command, &cwd)
                .map(|_| ())
        });
    }

    pub(super) fn kill_external(&mut self, pid: u32, cx: &mut Context<Self>) {
        self.run_background(cx, "Sent SIGTERM", move || supervisor::kill_external(pid));
    }

    pub(super) fn start_all(&mut self, repo: usize, target: RunTarget, cx: &mut Context<Self>) {
        let count = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .map_or(0, |s| s.processes.len());
        for index in 0..count {
            let up = self.repos[repo]
                .snapshot
                .as_ref()
                .is_some_and(|s| s.processes[index].status.is_up());
            if !up {
                self.start_process(repo, index, target, cx);
            }
        }
    }

    pub(super) fn stop_all(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.repos.get(repo).and_then(|v| v.snapshot.as_ref()) else {
            return;
        };
        // Only what the app started; processes held by others stay up.
        let ours: Vec<usize> = (0..snapshot.processes.len())
            .filter(|&i| snapshot.processes[i].owned)
            .collect();
        let outside = snapshot
            .processes
            .iter()
            .filter(|p| !p.owned && p.status.is_up())
            .count();
        if ours.is_empty() {
            if outside > 0 {
                self.show_info(format!(
                    "Nothing to stop: {outside} running process(es) were started outside the app"
                ));
            }
            return;
        }
        let names: Vec<String> = ours
            .iter()
            .map(|&i| snapshot.processes[i].spec.name.clone())
            .collect();
        let title = format!(
            "Stop {} in {}?",
            names.join(", "),
            self.repos[repo].entry.name()
        );
        let detail = if outside > 0 {
            format!("{outside} process(es) started outside the app keep running.")
        } else {
            String::new()
        };
        if !self.confirmed_or_ask(title, detail, "Stop all", Command::StopAll(repo)) {
            return;
        }
        for index in ours {
            self.stop_process(repo, index, cx);
        }
    }

    pub(super) fn repos_in_group(&self, group: Option<&str>) -> Vec<usize> {
        (0..self.repos.len())
            .filter(|&i| self.repos[i].entry.group.as_deref() == group)
            .collect()
    }

    pub(super) fn database_task(
        &mut self,
        repo: usize,
        index: usize,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Some(db) = view
            .snapshot
            .as_ref()
            .and_then(|s| s.databases.get(index))
            .cloned()
        else {
            return;
        };
        let spec = &db.spec;
        let command = match action {
            "start" => spec.start_command(),
            "stop" => spec.stop_command(),
            "migrate" => spec.migrate.clone(),
            "seed" => spec.seed.clone(),
            "studio" => spec.studio.clone(),
            _ => None,
        };
        let Some(command) = command else {
            self.show_error(format!(
                "{} has no {action} command in {}",
                spec.name,
                crate::services::FILE_NAME
            ));
            return;
        };
        if matches!(action, "seed" | "stop") && !self.confirmed {
            let (title, what) = match action {
                "seed" => (
                    format!("Seed {}?", spec.name),
                    "Seeding often resets or overwrites its data.",
                ),
                _ => (
                    format!("Stop {}?", spec.name),
                    "Anything using it loses its connection.",
                ),
            };
            let detail = format!("{what} Runs: {command}");
            let label = if action == "seed" { "Seed" } else { "Stop" };
            self.ask(
                title,
                detail,
                label,
                Command::DatabaseTask {
                    repo,
                    index,
                    action,
                },
            );
            return;
        }
        let key = process_key(&view.entry.path, &format!("{}:{action}", spec.name));
        if self.supervisor.is_running(&key) {
            self.show_error(format!(
                "{} {action} is still running — see Logs",
                spec.name
            ));
            return;
        }
        let message = format!("{} {action} started — output in Logs", spec.name);
        let cwd = view.entry.path.clone();
        self.log_key = Some(key.clone());
        self.user_stopped.insert(key.clone());
        self.show_alert(Alert {
            message,
            target: Command::ShowTab(Tab::Logs),
        });
        let supervisor = self.supervisor.clone();
        self.run_background(cx, "", move || {
            supervisor.start(&key, &command, &cwd).map(|_| ())
        });
    }

    pub(super) fn copy(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_info("Copied");
    }

    pub(super) fn register_repo(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Register".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let mut added = 0;
                for path in paths {
                    let root = git::toplevel(&path).unwrap_or(path);
                    if this.config.add_repo(root.clone()) {
                        this.repos.push(RepoView::new(RepoEntry {
                            path: root,
                            group: None,
                        }));
                        added += 1;
                    }
                }
                if added == 0 {
                    this.show_info("Already registered");
                    return;
                }
                this.save_config();
                this.watch_files(cx);
                this.select(this.repos.len() - 1);
                this.poll_now(true, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Re-points a registered repo whose folder moved.
    pub(super) fn locate_repo(&mut self, repo: usize, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Use this folder".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                let root = git::toplevel(&path).unwrap_or(path);
                let Some(view) = this.repos.get_mut(repo) else {
                    return;
                };
                let old = view.entry.path.clone();
                view.entry.path = root;
                view.snapshot = None;
                view.knowledge = None;
                this.config.remove_repo(&old);
                this.save_config();
                this.watch_files(cx);
                this.poll_now(true, cx);
                this.show_info("Repo located");
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn remove_repo(&mut self, repo: usize, cx: &mut Context<Self>) {
        if repo >= self.repos.len() {
            return;
        }
        let name = self.repos[repo].entry.name();
        if !self.confirmed_or_ask(
            format!("Remove {name} from fubbik?"),
            "Processes the app started there are stopped. Its files, branches and worktrees are untouched; register it again any time.",
            "Remove",
            Command::RemoveRepo(repo),
        ) {
            return;
        }
        self.stop_all(repo, cx);
        let removed = self.repos.remove(repo);
        self.config.remove_repo(&removed.entry.path);
        self.save_config();
        self.watch_files(cx);
        self.selected = self.selected.min(self.repos.len().saturating_sub(1));
        self.sync_group_field();
        self.show_info(format!(
            "Removed {} (its files are untouched)",
            removed.entry.name()
        ));
    }

    pub(super) fn generate_services(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let path = view.entry.path.clone();
        if crate::services::path(&path).exists() {
            self.show_error(format!("{} already exists", crate::services::FILE_NAME));
            return;
        }
        self.run_background(
            cx,
            "Wrote fubbik.services.toml — edit it to add ports and health URLs",
            move || {
                let detected: ServicesFile = detect::detect(&path);
                detected.save(&path)
            },
        );
    }

    pub(super) fn open_github(&mut self, repo: usize, cx: &mut Context<Self>) {
        let url = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .and_then(|s| s.remote.as_deref())
            .and_then(git::web_url);
        match url {
            Some(url) => cx.open_url(&url),
            None => self.show_error("No origin remote to open"),
        }
    }

    pub(super) fn create_worktree(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let branch = self.worktree_field.text.trim().to_string();
        if branch.is_empty() {
            self.show_error("Type a branch name for the new worktree");
            return;
        }
        self.refresh_worktrees = true;
        let path = view.entry.path.clone();
        // Created: the toast offers the next step (a shell there).
        let task = cx
            .background_executor()
            .spawn(async move { worktree::create(&path, &branch) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(created) => {
                        this.worktree_field.text.clear();
                        this.show_alert(Alert {
                            message: format!(
                                "Worktree created at {} — open a shell there",
                                display_path(&created)
                            ),
                            target: Command::ShellAt(created),
                        });
                    }
                    Err(error) => this.show_error(error),
                }
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn remove_worktree(&mut self, repo: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let live = view
            .snapshot
            .iter()
            .flat_map(|s| s.sessions.iter())
            .any(|s: &AiSession| s.state.is_live() && s.cwd.starts_with(&path));
        if live {
            self.show_error("An AI session is still running in that worktree");
            return;
        }
        let info = view.snapshot.as_ref().and_then(|s| {
            s.worktrees
                .as_ref()?
                .as_ref()
                .ok()?
                .iter()
                .find(|w| w.path == path)
                .cloned()
        });
        let detail = match &info {
            Some(w) => {
                let mut parts = vec![format!("{} — the branch is kept.", display_path(&path))];
                if w.changed_files > 0 {
                    parts.push(format!(
                        "{} uncommitted change(s) would be lost (git refuses unless forced).",
                        w.changed_files
                    ));
                }
                if w.ahead > 0 {
                    parts.push(format!(
                        "{} commit(s) not on main stay on the branch.",
                        w.ahead
                    ));
                }
                parts.join(" ")
            }
            None => display_path(&path),
        };
        let repo_path = view.entry.path.clone();
        if !self.confirmed_or_ask(
            "Remove this worktree?",
            detail,
            "Remove",
            Command::RemoveWorktree(repo, path.clone()),
        ) {
            return;
        }
        self.refresh_worktrees = true;
        let task = cx.background_executor().spawn({
            let (repo_path, path) = (repo_path.clone(), path.clone());
            async move { worktree::remove(&repo_path, &path) }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.show_info("Worktree removed (its branch is kept)"),
                    // Dirty or locked: offer the forced remove, which asks again.
                    Err(error) => this.show_alert(Alert {
                        message: format!("git refused: {error} — remove anyway with --force?"),
                        target: Command::ForceRemoveWorktree(repo, path.clone()),
                    }),
                }
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn run(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            Command::SelectRepo(i) => self.select(i),
            Command::StartProcess(r, p) => {
                self.ask_where(RunWhat::Process { repo: r, index: p }, cx)
            }
            Command::StopProcess(r, p) => self.stop_process(r, p, cx),
            Command::RestartProcess(r, p) => self.restart_process(r, p, cx),
            Command::StartAll(r) => self.ask_where(RunWhat::All(r), cx),
            Command::StopAll(r) => self.stop_all(r, cx),
            Command::ShowTab(tab) => {
                self.home = false;
                if self.tab != tab {
                    self.list_cursor = None;
                    self.board_cursor = None;
                }
                                self.tab = tab;
                if !tab.is_global() {
                    self.repo_tab = tab;
                }
                self.focus_terminal = tab == Tab::Terminal;
            }
            Command::RegisterRepo => self.register_repo(cx),
            Command::RemoveRepo(r) => self.remove_repo(r, cx),
            Command::GenerateServices(r) => self.generate_services(r, cx),
            Command::OpenEditor(r) => {
                if let Some(view) = self.repos.get(r) {
                    let path = view.entry.path.clone();
                    self.run_background(cx, "", move || launch::open_in_editor(&path));
                }
            }
            Command::OpenTerminal(r) => {
                if let Some(view) = self.repos.get(r) {
                    let path = view.entry.path.clone();
                    self.run_background(cx, "", move || launch::open_in_terminal(&path));
                }
            }
            Command::OpenGitHub(r) => self.open_github(r, cx),
            Command::Refresh => {
                // Picks up edits to the Ghostty config when following it.
                if self.config.ghostty_theme.as_deref() == Some("follow") {
                    let _ = theme::apply(Some("follow"));
                }
                                self.poll_now(true, cx);
                self.refresh_prs(true, cx);
            }
            Command::SetColors(choice) => match theme::apply(choice.as_deref()) {
                Ok(label) => {
                    self.config.ghostty_theme = choice;
                    self.save_config();
                    self.show_info(format!("Colors: {label}"));
                }
                Err(error) => self.show_error(error),
            },
            Command::ToggleTheme => {
                let on = !theme::is_translucent();
                theme::set_translucent(on);
                self.config.theme = Some(if on { "translucent" } else { "opaque" }.into());
                self.save_config();
            }
            Command::OpenEditorAt(path) => {
                self.run_background(cx, "", move || launch::open_in_editor(&path))
            }
            Command::OpenTerminalAt(path) => {
                self.run_background(cx, "", move || launch::open_in_terminal(&path))
            }
            Command::NewSession(path) => {
                self.launch_agent(path, "Claude".into(), "claude".into(), false, cx)
            }
            Command::ResumeSession(agent, id, cwd) => {
                let label = format!("{} {}", agent.label(), &id[..id.len().min(8)]);
                self.launch_agent(cwd, label, agent.resume_command(&id), false, cx);
            }
            Command::CreateWorktree(r) => self.create_worktree(r, cx),
            Command::OpenWeb(path) => cx.open_url(&format!("{}{path}", self.config.web_url())),
            Command::SetPersistence(on) => {
                self.config.terminal_persistence = if on { None } else { Some("none".into()) };
                self.tmux_terminals = on && tmux::available();
                self.save_config();
                self.show_info("Applies to terminal tabs opened from now on");
            }
            Command::SetExternalTerminal(on) => {
                self.config.terminal = on.then(|| "external".to_string());
                self.save_config();
            }
            Command::ImportDatabase(source) if !self.confirmed => {
                let detail = format!(
                    "The built-in database is replaced by a copy of {source} (only read). Anything only in the built-in one is lost — Setup → Back up now first if unsure. The API pauses during the copy."
                );
                self.ask(
                    "Replace the built-in database?",
                    detail,
                    "Replace",
                    Command::ImportDatabase(source),
                );
            }
            Command::ImportDatabase(source) => {
                use crate::backend::{Control, data_dir, request_import, send};
                if send(Control::Import(source.clone())) {
                    self.show_info("Importing: the API pauses for the copy, then comes back");
                } else {
                    match request_import(&data_dir(), &source) {
                        Ok(()) => self.show_info(
                            "The built-in server is not running here: the import runs when it next starts",
                        ),
                        Err(error) => self.show_error(format!("Could not queue the import: {error}")),
                    }
                }
            }
            Command::BackupNow => {
                if !crate::backend::send(crate::backend::Control::BackupNow) {
                    self.show_error("The built-in server is not running in this app");
                }
            }
            Command::ShowBackups => {
                let dir = crate::backend::data_dir().join("backups");
                let _ = std::fs::create_dir_all(&dir);
                cx.open_url(&format!(
                    "file://{}",
                    dir.display().to_string().replace(' ', "%20")
                ));
            }
            Command::StopBackgroundServer => match crate::backend::stop_background() {
                Ok(()) => self.show_info("Stopped the background server"),
                Err(error) => self.show_error(error),
            },
            Command::SetEmbeddedBackend(on) => {
                self.config.backend = (!on).then(|| "off".to_string());
                self.save_config();
                self.show_info("Applies when the app next starts");
            }
            Command::SetTaskStatus { plan, task, status } => {
                self.set_task_status(plan, task, status, cx)
            }
            Command::ToggleCriterion {
                repo,
                plan,
                task,
                index,
            } => self.toggle_criterion(repo, plan, task, index, cx),
            Command::TaskTimeline { repo, plan, task } => self.task_timeline(repo, plan, task, cx),
            Command::SaveTaskNote { repo, plan, task } => self.save_task_note(repo, plan, task, cx),
            Command::SelectPlan(id) => self.board_plan = Some(id),
            Command::ToggleChecklist(id) => {
                self.open_checklist = if self.open_checklist.as_ref() == Some(&id) {
                    None
                } else {
                    Some(id)
                };
            }
            Command::RunScript {
                repo,
                name,
                command,
            } => self.ask_where(
                RunWhat::Script {
                    repo,
                    name,
                    command,
                },
                cx,
            ),
            Command::StartProfile { repo, profile } => {
                self.ask_where(RunWhat::Profile { repo, profile }, cx)
            }
            Command::RunAt {
                what,
                target,
                remember,
            } => {
                self.run_prompt = None;
                if remember {
                    self.config.run_in = Some(target.key().to_string());
                    self.save_config();
                }
                self.run_what(what, target, cx);
            }
            Command::SetRunIn(target) => {
                self.config.run_in = target.map(|t| t.key().to_string());
                self.save_config();
            }
            Command::AttachRun(session) => self.attach_run(session, cx),
                        Command::AskFirst(confirm) => self.confirm = Some(*confirm),
                        Command::LocateRepo(repo) => self.locate_repo(repo, cx),
            Command::DeleteBranch { repo, name } => {
                if self.confirmed_or_ask(
                    format!("Delete branch {name}?"),
                    "It is merged; git refuses if it is not. Its commits stay on the branch it was merged into.",
                    "Delete",
                    Command::DeleteBranch { repo, name: name.clone() },
                ) && let Some(view) = self.repos.get(repo)
                {
                    let path = view.entry.path.clone();
                    let days = self.insights_days;
                    self.run_background(cx, "Branch deleted", move || worktree::delete_merged_branch(&path, &name));
                    self.load_insights(repo, days, cx);
                }
            }
            Command::KillPid { pid, what } => {
                let detail = format!(
                    "pid {pid} was not started by this app. It gets SIGTERM; unsaved work in it is lost."
                );
                if self.confirmed_or_ask(
                    format!("Kill {what}?"),
                    detail,
                    "Kill",
                    Command::KillPid {
                        pid,
                        what: what.clone(),
                    },
                ) {
                    self.kill_external(pid, cx);
                }
            }
            Command::Review { proposal, approve } => {
                if approve
                    || self.confirmed_or_ask(
                        "Reject this proposal?",
                        "The proposed change is discarded; the chunk stays as it is.",
                        "Reject",
                        Command::Review {
                            proposal: proposal.clone(),
                            approve,
                        },
                    )
                {
                    self.review(proposal, approve, cx);
                }
            }
            Command::ForceRemoveWorktree(r, path) => {
                let detail = format!(
                    "{} has changes git would lose. --force deletes them along with the folder; the branch is kept.",
                    display_path(&path)
                );
                if self.confirmed_or_ask(
                    "Remove anyway, losing its changes?",
                    detail,
                    "Remove with --force",
                    Command::ForceRemoveWorktree(r, path.clone()),
                ) && let Some(view) = self.repos.get(r)
                {
                    let repo_path = view.entry.path.clone();
                    self.refresh_worktrees = true;
                    self.run_background(cx, "Worktree removed (its branch is kept)", move || {
                        worktree::remove_force(&repo_path, &path)
                    });
                }
            }
            Command::ClearRunLog(key) => {
                if self.confirmed_or_ask(
                    "Clear this log?",
                    "Its log file is emptied; the process keeps running.",
                    "Clear",
                    Command::ClearRunLog(key.clone()),
                ) {
                    self.supervisor.clear_logs(&key);
                }
            }
            Command::DatabaseTask {
                repo,
                index,
                action,
            } => self.database_task(repo, index, action, cx),
            Command::StopKey(key) => {
                self.user_stopped.insert(key.clone());
                let supervisor = self.supervisor.clone();
                self.run_background(cx, "Stopped", move || {
                    supervisor.stop(&key);
                    Ok(())
                });
            }
            Command::DismissToast(index) => {
                if index < self.toasts.len() {
                    self.toasts.remove(index);
                }
            }
            Command::SetMaxAgents(max) => {
                self.config.max_agents = max;
                self.save_config();
                self.drain_launch_queue(cx);
            }
                        Command::CaptureNote => self.capture_open = true,
            // The palette opens the reply box itself (it has the window).
            Command::ReplyWaiting => self.show_info("Press ⇧⌘R to reply to the waiting agent"),
            Command::StartNextTask => self.start_next_task(cx),
            Command::NextNeedsYou => self.next_needs_you(cx),
                        Command::ShowHome => self.go_home(),
                        Command::OpenUrl(url) => cx.open_url(&url),
            Command::FocusTerminal(id) => self.focus_terminal_tab(id),
            Command::OpenPane { pane, cwd } => self.open_pane(pane, cwd, cx),
            Command::Focus { repo, tab } => {
                self.select(repo);
                self.tab = tab;
            }
            Command::ShowLogs { repo, key } => {
                self.select(repo);
                self.log_key = Some(key);
                self.log_paused = None;
                self.tab = Tab::Logs;
            }
            Command::ReviewWorktree { repo, path } => self.review_worktree(repo, path, cx),
            Command::RunIn {
                dir,
                label,
                command,
            } => self.launch_in_terminal(dir, label, command, cx),
            Command::ShellAt(path) => {
                match self
                    .terminals
                    .iter()
                    .find(|t| t.dir == path && t.label == "Shell")
                {
                    Some(existing) => self.active_terminal = Some(existing.id),
                    None => {
                        self.open_terminal(path, "Shell".into(), None, cx);
                    }
                }
                self.tab = Tab::Terminal;
                self.focus_terminal = true;
            }
            Command::ToggleSplit => {
                if self.split_with.take().is_none()
                    && let Some(active) = self.active_terminal
                    && let Some(dir) = self
                        .terminals
                        .iter()
                        .find(|t| t.id == active)
                        .map(|t| t.dir.clone())
                {
                    let second = self.open_terminal_with(
                        dir,
                        "Shell".into(),
                        None,
                        self.tmux_terminals,
                        false,
                        cx,
                    );
                    self.split_with = Some(second);
                }
            }
            Command::CloseTerminal(id) if !self.confirmed => {
                // A shell tab still running something (a server, an agent) asks first.
                let busy = self
                    .terminals
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.tmux.clone())
                    .filter(|name| !name.starts_with(crate::tmux_runs::PREFIX))
                    .and_then(|name| tmux::foreground_command(&tmux::Server::fubbik(), &name))
                    .filter(|command| {
                        !["zsh", "bash", "fish", "sh", "-zsh", "login", "tmux"]
                            .contains(&command.as_str())
                    });
                match busy {
                    Some(command) => self.ask(
                        format!("Close this tab? {command} is still running"),
                        "Closing ends its tmux session and everything in it.",
                        "Close",
                        Command::CloseTerminal(id),
                    ),
                    None => {
                        self.confirmed = true;
                        self.run(Command::CloseTerminal(id), cx);
                        self.confirmed = false;
                    }
                }
            }
            Command::CloseTerminal(id) => {
                if self.split_with == Some(id) {
                    self.split_with = None;
                }
                // Closing a tab ends its shell session; quitting the app does
                // not. A process or script run (fk-run-*) only detaches.
                if let Some(name) = self
                    .terminals
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.tmux.clone())
                    .filter(|name| !name.starts_with(crate::tmux_runs::PREFIX))
                {
                    let _ = tmux::kill(&tmux::Server::fubbik(), &name);
                }
                self.terminals.retain(|t| t.id != id);
            }
            Command::StartTask {
                repo,
                plan,
                task,
                new_worktree,
                background,
            } => self.start_task(repo, &plan, &task, new_worktree, background, cx),
            Command::RemoveWorktree(r, path) => self.remove_worktree(r, path, cx),
            Command::PruneWorktrees(r) => {
                if let Some(view) = self.repos.get(r) {
                    let repo = view.entry.path.clone();
                    self.refresh_worktrees = true;
                    self.run_background(cx, "Pruned stale worktrees", move || {
                        worktree::prune(&repo)
                    });
                }
            }
        }
        cx.notify();
    }

    /// A repo script as a logged task (Logs tab).
    pub(super) fn run_script(
        &mut self,
        repo: usize,
        name: String,
        command: String,
        target: RunTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let key = process_key(&view.entry.path, &crate::scripts::run_name(&name, &command));
        let cwd = view.entry.path.clone();
        self.remember_script(repo, &command);
        self.user_stopped.insert(key.clone());
        if target == RunTarget::Tmux && self.supervisor.is_running(&key) {
            self.show_error(format!("{name} is still running in its tmux session"));
            return;
        }
        self.launch_run(key, name, command, cwd, target, cx);
    }

    /// Starts a profile's processes and stops the repo's other processes
    /// that this app started.
    pub(super) fn start_profile(
        &mut self,
        repo: usize,
        profile: &str,
        target: RunTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let Ok(Some(services)) = ServicesFile::load(&view.entry.path) else {
            return;
        };
        let Some(wanted) = services.profile.get(profile).map(|p| p.processes.clone()) else {
            self.show_error(format!("No profile “{profile}”"));
            return;
        };
        let processes: Vec<(usize, String, bool, bool)> = view
            .snapshot
            .iter()
            .flat_map(|s| s.processes.iter().enumerate())
            .map(|(i, p)| (i, p.spec.name.clone(), p.owned, p.status.is_up()))
            .collect();
        let stopping: Vec<String> = processes
            .iter()
            .filter(|(_, name, owned, _)| *owned && !wanted.contains(name))
            .map(|(_, name, _, _)| name.clone())
            .collect();
        if !stopping.is_empty()
            && !self.confirmed_or_ask(
                format!("Start profile “{profile}”?"),
                format!("It also stops {}.", stopping.join(", ")),
                "Start profile",
                Command::RunAt {
                    what: RunWhat::Profile {
                        repo,
                        profile: profile.to_string(),
                    },
                    target,
                    remember: false,
                },
            )
        {
            return;
        }
        for (index, name, owned, up) in processes {
            match (wanted.contains(&name), owned, up) {
                (true, _, false) => self.start_process(repo, index, target, cx),
                (false, true, _) => self.stop_process(repo, index, cx),
                _ => {}
            }
        }
        self.show_info(format!("Profile “{profile}”: {}", wanted.join(", ")));
    }
}
