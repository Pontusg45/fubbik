//! What the newer hotkeys do: jump to what needs you, reply to the waiting
//! agent, start the next task, pick a repo by number, and drive terminal
//! tabs from the keyboard.

use super::*;
use crate::sessions::SessionState;

impl Workspace {
    /// ⌃1–⌃9.
    pub(super) fn select_repo_n(&mut self, n: usize, cx: &mut Context<Self>) {
        if n < self.repos.len() {
            self.select(n);
            cx.notify();
        }
    }

    /// ⇧⌘J: the next "Needs you" line on Today, cycling.
    pub(super) fn next_needs_you(&mut self, cx: &mut Context<Self>) {
        let actions: Vec<Command> = self
            .today_rows()
            .into_iter()
            .filter(|r| r.section == today::Section::Needs)
            .filter_map(|r| r.primary)
            .collect();
        if actions.is_empty() {
            self.show_info("Nothing needs you right now");
            return;
        }
        let index = self.needs_cursor % actions.len();
        self.needs_cursor = index + 1;
        self.run(actions[index].clone(), cx);
    }

    /// ⇧⌘R: the reply box for the agent that has waited longest.
    pub(super) fn reply_latest(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let waiting = self
            .repos
            .iter()
            .enumerate()
            .flat_map(|(i, r)| {
                r.snapshot
                    .iter()
                    .flat_map(move |s| s.sessions.iter().map(move |s| (i, s)))
            })
            .filter(|(_, s)| s.state == SessionState::Idle && s.pane.is_some())
            .min_by_key(|(_, s)| s.last_activity)
            .map(|(i, s)| (i, s.pane.clone().unwrap()));
        let Some((repo, pane)) = waiting else {
            self.show_info("No agent in tmux is waiting for you");
            return;
        };
        self.select(repo);
        self.tab = Tab::Sessions;
        self.reply_target = None;
        self.toggle_reply(pane, window);
        cx.notify();
    }

    /// ⇧⌘S: the first task under "Next up", started in the background.
    pub(super) fn start_next_task(&mut self, cx: &mut Context<Self>) {
        let next = self
            .today_rows()
            .into_iter()
            .filter(|r| r.section == today::Section::Next)
            .find_map(|r| {
                r.secondary
                    .into_iter()
                    .find(|(label, _)| label == "In background")
                    .map(|(_, c)| c)
            });
        match next {
            Some(command) => self.run(command, cx),
            None => self.show_info("No task is ready to start"),
        }
    }

    fn active_terminal_tab(&self) -> Option<(usize, Vec<usize>)> {
        let ids: Vec<usize> = self.terminals_for_selected().iter().map(|t| t.id).collect();
        let position = self
            .active_terminal
            .and_then(|a| ids.iter().position(|id| *id == a))?;
        Some((position, ids))
    }

    /// ⌘1–⌘9 in a terminal; ⇧⌘[ / ⇧⌘] step (`delta` −1 / +1).
    pub(super) fn switch_terminal_tab(
        &mut self,
        to: Option<usize>,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<usize> = self.terminals_for_selected().iter().map(|t| t.id).collect();
        if ids.is_empty() {
            return;
        }
        let index = match to {
            Some(n) if n < ids.len() => n,
            Some(_) => return,
            None => {
                let current = self.active_terminal_tab().map_or(0, |(p, _)| p) as isize;
                (current + delta).rem_euclid(ids.len() as isize) as usize
            }
        };
        self.active_terminal = Some(ids[index]);
        self.focus_terminal = true;
        cx.notify();
    }

    /// ⌘T in a terminal: another shell where the active one runs.
    pub(super) fn new_shell(&mut self, cx: &mut Context<Self>) {
        let dir = self
            .active_terminal
            .and_then(|a| self.terminals.iter().find(|t| t.id == a))
            .map(|t| t.dir.clone())
            .or_else(|| self.repos.get(self.selected).map(|r| r.entry.path.clone()));
        if let Some(dir) = dir {
            self.open_terminal(dir, "Shell".into(), None, cx);
            self.tab = Tab::Terminal;
            self.focus_terminal = true;
            cx.notify();
        }
    }

    /// ⌘W in a terminal: closes the active tab (and its tmux session).
    pub(super) fn close_terminal_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.active_terminal {
            self.run(Command::CloseTerminal(id), cx);
            self.focus_terminal = true;
        }
    }

    /// ⇧⌘K in a terminal.
    pub(super) fn clear_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self
            .active_terminal
            .and_then(|a| self.terminals.iter().find(|t| t.id == a))
        {
            tab.view.update(cx, |view, cx| view.clear(cx));
        }
    }

    /// Setup → Keyboard: opens keymap.toml in the editor, writing a
    /// commented starter (every action, with its default) when missing.
    pub(super) fn open_keymap_file(&mut self, cx: &mut Context<Self>) {
        let path = keymap::path();
        if !path.exists() {
            let mut text = String::from(
                "# fubbik desktop key bindings. Uncomment a line to change it; \"\" unbinds.\n\
                 # Keys: cmd, ctrl, alt, shift joined with -, e.g. \"cmd-shift-h\". Restart to apply.\n\n[bindings]\n",
            );
            for binding in keymap::defaults() {
                text.push_str(&format!(
                    "# {} = \"{}\"    # {}\n",
                    binding.action, binding.keys, binding.description
                ));
            }
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(error) = std::fs::write(&path, text) {
                self.show_error(format!("Could not write {}: {error}", path.display()));
                return;
            }
        }
        self.run_background(cx, "", move || launch::open_in_editor(&path));
    }
}
