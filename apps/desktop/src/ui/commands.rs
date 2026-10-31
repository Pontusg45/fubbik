//! Commands: processes, repos, worktrees, databases, and `run`, which every
//! button and palette entry goes through.

use super::*;

impl Workspace {
    // ── commands ───────────────────────────────────────────────────────

    pub(super) fn start_process(&mut self, repo: usize, index: usize, cx: &mut Context<Self>) {
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
        self.log_key = Some(state.key.clone());
        let supervisor = self.supervisor.clone();
        let cwd = state.spec.cwd(&view.entry.path);
        self.run_background(cx, "", move || {
            supervisor
                .start(&state.key, &state.spec.command, &cwd)
                .map(|_| ())
        });
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
            self.start_process(repo, index, cx);
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

    pub(super) fn start_all(&mut self, repo: usize, cx: &mut Context<Self>) {
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
                self.start_process(repo, index, cx);
            }
        }
    }

    pub(super) fn stop_all(&mut self, repo: usize, cx: &mut Context<Self>) {
        let count = self
            .repos
            .get(repo)
            .and_then(|v| v.snapshot.as_ref())
            .map_or(0, |s| s.processes.len());
        for index in 0..count {
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
        let key = process_key(&view.entry.path, &format!("{}:{action}", spec.name));
        self.log_key = Some(key.clone());
        self.user_stopped.insert(key.clone());
        let supervisor = self.supervisor.clone();
        let cwd = view.entry.path.clone();
        self.run_background(cx, "", move || {
            supervisor.stop(&key);
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

    pub(super) fn remove_repo(&mut self, repo: usize, cx: &mut Context<Self>) {
        if repo >= self.repos.len() {
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
        self.worktree_field.text.clear();
        self.refresh_worktrees = true;
        let path = view.entry.path.clone();
        self.run_background(cx, "Worktree created", move || {
            worktree::create(&path, &branch).map(|_| ())
        });
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
        self.refresh_worktrees = true;
        let repo_path = view.entry.path.clone();
        self.run_background(cx, "Worktree removed (its branch is kept)", move || {
            worktree::remove(&repo_path, &path)
        });
    }

    pub(super) fn run(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            Command::SelectRepo(i) => self.select(i),
            Command::StartProcess(r, p) => self.start_process(r, p, cx),
            Command::StopProcess(r, p) => self.stop_process(r, p, cx),
            Command::RestartProcess(r, p) => self.restart_process(r, p, cx),
            Command::StartAll(r) => self.start_all(r, cx),
            Command::StopAll(r) => self.stop_all(r, cx),
            Command::ShowTab(tab) => {
                self.tab = tab;
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
            } => self.run_script(repo, name, command, cx),
            Command::StartProfile { repo, profile } => self.start_profile(repo, &profile, cx),
            Command::SetMaxAgents(max) => {
                self.config.max_agents = max;
                self.save_config();
                self.drain_launch_queue(cx);
            }
            Command::CaptureNote => self.capture_open = true,
            Command::Focus { repo, tab } => {
                self.select(repo);
                self.tab = tab;
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
            Command::CloseTerminal(id) => {
                if self.split_with == Some(id) {
                    self.split_with = None;
                }
                // Closing a tab ends its session; quitting the app does not.
                if let Some(name) = self
                    .terminals
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.tmux.clone())
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
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let key = process_key(&view.entry.path, &format!("script:{name}"));
        let cwd = view.entry.path.clone();
        self.log_key = Some(key.clone());
        self.user_stopped.insert(key.clone());
        let supervisor = self.supervisor.clone();
        self.run_background(cx, "Running — see Logs", move || {
            supervisor.stop(&key);
            supervisor.start(&key, &command, &cwd).map(|_| ())
        });
    }

    /// Starts a profile's processes and stops the repo's other processes
    /// that this app started.
    pub(super) fn start_profile(&mut self, repo: usize, profile: &str, cx: &mut Context<Self>) {
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
        for (index, name, owned, up) in processes {
            match (wanted.contains(&name), owned, up) {
                (true, _, false) => self.start_process(repo, index, cx),
                (false, true, _) => self.stop_process(repo, index, cx),
                _ => {}
            }
        }
        self.show_info(format!("Profile “{profile}”: {}", wanted.join(", ")));
    }
}
