//! Terminal tabs: opening (plain or tmux-backed), restoring, launching
//! commands in them, and picking the one to show.

use super::*;

impl Workspace {
    /// The strip under the transparent system titlebar: traffic lights on
    /// Opens a terminal tab: a login shell in `dir`, typing `command` when
    /// given. Returns its id.
    pub(super) fn open_terminal(
        &mut self,
        dir: PathBuf,
        label: String,
        command: Option<String>,
        cx: &mut Context<Self>,
    ) -> usize {
        self.open_terminal_with(dir, label, command, self.tmux_terminals, true, cx)
    }

    /// Opens a terminal tab. With `persist`, it is a tmux session on the
    /// app's server (it survives quitting the app); otherwise a plain shell.
    /// `activate` makes it the tab shown.
    pub(super) fn open_terminal_with(
        &mut self,
        dir: PathBuf,
        label: String,
        command: Option<String>,
        persist: bool,
        activate: bool,
        cx: &mut Context<Self>,
    ) -> usize {
        let id = self.next_terminal_id;
        self.next_terminal_id += 1;
        let server = tmux::Server::fubbik();
        let session = persist
            .then(|| {
                let mut n = id;
                while tmux::exists(&server, &tmux::session_name(&dir, n)) {
                    n += 1;
                }
                let name = tmux::session_name(&dir, n);
                match tmux::create(&server, &name, &dir, &label, command.as_deref()) {
                    Ok(()) => Some(name),
                    Err(error) => {
                        self.show_error(format!("tmux failed, using a plain shell: {error}"));
                        None
                    }
                }
            })
            .flatten();
        let view = match &session {
            Some(name) => {
                let (dir, name) = (dir.clone(), name.clone());
                cx.new(|cx| terminal_view::TerminalView::tmux(dir, name, cx))
            }
            None => cx.new(|cx| terminal_view::TerminalView::new(dir.clone(), command, cx)),
        };
        self.terminals.push(TermTab {
            id,
            dir,
            label,
            view,
            tmux: session,
        });
        if activate {
            self.active_terminal = Some(id);
        }
        id
    }

    /// A process or script's tmux session in a terminal tab (the existing
    /// tab when one is open).
    pub(super) fn attach_run(&mut self, session: String, cx: &mut Context<Self>) {
        if let Some(tab) = self
            .terminals
            .iter()
            .find(|t| t.tmux.as_deref() == Some(session.as_str()))
        {
            self.active_terminal = Some(tab.id);
        } else {
            let id = self.next_terminal_id;
            self.next_terminal_id += 1;
            let dir = self
                .repos
                .get(self.selected)
                .map(|r| r.entry.path.clone())
                .unwrap_or_default();
            let label = session
                .trim_start_matches(crate::tmux_runs::PREFIX)
                .to_string();
            let (view_dir, name) = (dir.clone(), session.clone());
            let view = cx.new(|cx| terminal_view::TerminalView::tmux(view_dir, name, cx));
            self.terminals.push(TermTab {
                id,
                dir,
                label,
                view,
                tmux: Some(session),
            });
            self.active_terminal = Some(id);
        }
        self.tab = Tab::Terminal;
        self.focus_terminal = true;
    }

    /// Reattaches the app's tmux sessions left from the last run as tabs.
    pub(super) fn restore_terminals(&mut self, cx: &mut Context<Self>) {
        for session in tmux::sessions(&tmux::Server::fubbik())
            .into_iter()
            // Process and script runs are not tabs; they open from Processes.
            .filter(|s| !s.name.starts_with(crate::tmux_runs::PREFIX))
        {
            let id = self.next_terminal_id;
            self.next_terminal_id += 1;
            let (dir, name) = (session.dir.clone(), session.name.clone());
            let view = cx.new(|cx| terminal_view::TerminalView::tmux(dir, name, cx));
            self.terminals.push(TermTab {
                id,
                dir: session.dir,
                label: session.label,
                view,
                tmux: Some(session.name),
            });
        }
    }

    /// Runs a command for the user, in the Terminal tab or, with
    /// `terminal = "external"`, in Terminal.app.
    pub(super) fn launch_in_terminal(
        &mut self,
        dir: PathBuf,
        label: String,
        command: String,
        cx: &mut Context<Self>,
    ) {
        self.launch(dir, label, command, false, cx);
    }

    /// `background`: open the tab without switching to it (with tmux, the
    /// session runs detached until you look).
    pub(super) fn launch(
        &mut self,
        dir: PathBuf,
        label: String,
        command: String,
        background: bool,
        cx: &mut Context<Self>,
    ) {
        if self.config.external_terminal() {
            self.run_background(cx, "", move || launch::run_in_terminal(&dir, &command));
            return;
        }
        // Show it in the repo it belongs to (unless it runs in the background).
        if !background
            && let Some(repo) = self.repos.iter().position(|r| {
                dir.starts_with(&r.entry.path)
                    || r.snapshot
                        .as_ref()
                        .is_some_and(|s| s.worktree_paths().contains(&dir))
            })
        {
            self.select(repo);
        }
        if background {
            self.open_terminal_with(dir, label, Some(command), self.tmux_terminals, false, cx);
            return;
        }
        self.open_terminal(dir, label, Some(command), cx);
        self.tab = Tab::Terminal;
        self.focus_terminal = true;
    }

    /// An AI agent launch: waits in the queue while `max_agents` are running.
    pub(super) fn launch_agent(
        &mut self,
        dir: PathBuf,
        label: String,
        command: String,
        background: bool,
        cx: &mut Context<Self>,
    ) {
        if self.at_agent_limit() {
            self.launch_queue.push_back(QueuedLaunch {
                dir,
                label,
                command,
            });
            let max = self.config.max_agents.unwrap_or_default();
            self.show_info(format!(
                "{max} agents already running — queued ({} waiting)",
                self.launch_queue.len()
            ));
            return;
        }
        self.recent_launches.push(Instant::now());
        self.launch(dir, label, command, background, cx);
    }

    fn at_agent_limit(&mut self) -> bool {
        self.recent_launches.retain(|t| t.elapsed() < RECENT_LAUNCH);
        self.config
            .max_agents
            .is_some_and(|max| self.live_agents() + self.recent_launches.len() >= max)
    }

    /// Starts queued agents while there is room (after each poll).
    pub(super) fn drain_launch_queue(&mut self, cx: &mut Context<Self>) {
        while !self.launch_queue.is_empty() && !self.at_agent_limit() {
            let Some(next) = self.launch_queue.pop_front() else {
                break;
            };
            self.recent_launches.push(Instant::now());
            self.show_info(format!("Started queued: {}", next.label));
            self.launch(next.dir, next.label, next.command, true, cx);
        }
    }

    /// Terminal tabs whose directory is the selected repo or one of its worktrees.
    pub(super) fn terminals_for_selected(&self) -> Vec<&TermTab> {
        let Some(repo) = self.repos.get(self.selected) else {
            return Vec::new();
        };
        let worktrees = repo
            .snapshot
            .as_ref()
            .map(|s| s.worktree_paths())
            .unwrap_or_default();
        self.terminals
            .iter()
            .filter(|t| t.dir.starts_with(&repo.entry.path) || worktrees.contains(&t.dir))
            .collect()
    }

    /// The terminal the Terminal tab shows, opening a shell in the repo when
    /// it has none.
    pub(super) fn terminal_for_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<terminal_view::TerminalView>> {
        let repo_path = self.repos.get(self.selected)?.entry.path.clone();
        let ids: Vec<usize> = self.terminals_for_selected().iter().map(|t| t.id).collect();
        let id = match self
            .active_terminal
            .filter(|id| ids.contains(id))
            .or(ids.last().copied())
        {
            Some(id) => id,
            // A shell opens by itself only when you just switched here (⌘0, a
            // tab click); after closing the last one, the tab stays empty.
            None if self.focus_terminal => self.open_terminal(repo_path, "Shell".into(), None, cx),
            None => return None,
        };
        self.active_terminal = Some(id);
        let view = self.terminals.iter().find(|t| t.id == id)?.view.clone();
        if std::mem::take(&mut self.focus_terminal) {
            view.read(cx).focus.focus(window);
        }
        Some(view)
    }
}
