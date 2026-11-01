//! Reacts to the app's tmux server as things happen: a bell in a terminal
//! tab becomes a toast and a "Needs you" line on Today until the tab is
//! looked at; a pane that exits re-polls at once, so a finished run shows
//! its exit straight away.

use std::time::{Duration, Instant};

use super::*;
use crate::tmux_events::{self, EventKind};

/// How often the events file is checked for new lines.
const EVENTS_EVERY: Duration = Duration::from_millis(300);

impl Workspace {
    /// Installs the hooks (when the app's tmux server runs) and follows the
    /// events file from its current end.
    pub(super) fn watch_tmux_events(&mut self, cx: &mut Context<Self>) {
        if !crate::tmux::available() {
            return;
        }
        let path = tmux_events::events_path();
        // No server yet: the hooks go on with the first session.
        let _ = tmux_events::install(&crate::tmux::Server::fubbik(), &path);
        let mut offset = tmux_events::end_of(&path);
        self._events_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(EVENTS_EVERY).await;
                let read = {
                    let path = path.clone();
                    cx.background_executor()
                        .spawn(async move { tmux_events::read_new(&path, offset) })
                };
                let (events, next) = read.await;
                offset = next;
                if events.is_empty() {
                    continue;
                }
                if this
                    .update(cx, |this, cx| this.on_tmux_events(events, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    pub(super) fn on_tmux_events(
        &mut self,
        events: Vec<tmux_events::Event>,
        cx: &mut Context<Self>,
    ) {
        let mut poll = false;
        for event in events {
            match event.kind {
                EventKind::Died => poll = true,
                EventKind::Bell => {
                    let Some(tab) = self
                        .terminals
                        .iter()
                        .find(|t| t.tmux.as_deref() == Some(event.session.as_str()))
                    else {
                        continue; // a run or someone else's session
                    };
                    let id = tab.id;
                    let label = tab.label.clone();
                    // The tab in front already has your attention.
                    let shown = self.tab == Tab::Terminal && self.active_terminal == Some(id);
                    if shown || self.bells.iter().any(|(b, _)| *b == id) {
                        continue;
                    }
                    self.bells.push((id, Instant::now()));
                    self.show_alert(Alert {
                        message: format!("🔔 {label} wants your attention"),
                        target: Command::FocusTerminal(id),
                    });
                }
            }
        }
        if poll {
            self.poll_now(false, cx);
        }
        cx.notify();
    }

    /// Shows a terminal tab (selecting its repo) and clears its bell.
    pub(super) fn focus_terminal_tab(&mut self, id: usize) {
        let Some(dir) = self
            .terminals
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.dir.clone())
        else {
            return;
        };
        if let Some(repo) = self.repos.iter().position(|r| {
            dir.starts_with(&r.entry.path)
                || r.snapshot
                    .as_ref()
                    .is_some_and(|s| s.worktree_paths().contains(&dir))
        }) {
            self.select(repo);
        }
        self.tab = Tab::Terminal;
        self.active_terminal = Some(id);
        self.focus_terminal = true;
        self.bells.retain(|(b, _)| *b != id);
    }

    /// Clears the bell of the terminal tab on screen.
    pub(super) fn clear_shown_bell(&mut self) {
        if self.tab == Tab::Terminal
            && let Some(id) = self.active_terminal
        {
            self.bells.retain(|(b, _)| *b != id);
        }
    }
}
