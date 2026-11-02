//! Keeps the menu bar item (`crate::menu_bar`) in step with Today, and runs
//! what is picked from it.

use super::today::Section;
use super::*;
use crate::menu_bar::{self, Entry};

/// Needs-you rows listed before "+N more".
const MAX_ROWS: usize = 8;

impl Workspace {
    pub(super) fn sync_menu_bar(&mut self) {
        if self.config.menu_bar == Some(false) {
            menu_bar::remove();
            self.menu_commands.clear();
            return;
        }
        let needs: Vec<_> = self
            .today_rows()
            .into_iter()
            .filter(|r| r.section == Section::Needs)
            .collect();
        let mut entries = Vec::new();
        let mut commands = Vec::new();
        let mut push = |entry: Entry, command: Option<Command>| {
            entries.push(entry);
            commands.push(command);
        };
        if needs.is_empty() {
            push(Entry::Label("Nothing needs you".into()), None);
        } else {
            push(Entry::Label(format!("{} need you", needs.len())), None);
            for row in needs.iter().take(MAX_ROWS) {
                let text = if row.repo.is_empty() || row.title.contains(&row.repo) {
                    row.title.clone()
                } else {
                    format!("{} · {}", row.repo, row.title)
                };
                push(
                    Entry::Item(text.chars().take(80).collect()),
                    Some(row.primary.clone().unwrap_or(Command::ShowTab(Tab::Today))),
                );
            }
            if needs.len() > MAX_ROWS {
                push(
                    Entry::Item(format!("+{} more on Today", needs.len() - MAX_ROWS)),
                    Some(Command::ShowTab(Tab::Today)),
                );
            }
        }
        push(Entry::Separator, None);
        push(
            Entry::Item("Open Today".into()),
            Some(Command::ShowTab(Tab::Today)),
        );
        push(
            Entry::Item(format!("All agents ({} live)", self.live_agents())),
            Some(Command::ShowTab(Tab::Agents)),
        );
        push(
            Entry::Item("Capture note…".into()),
            Some(Command::CaptureNote),
        );
        let title = if needs.is_empty() {
            "fk".to_string()
        } else {
            format!("fk {}", needs.len())
        };
        menu_bar::set(&title, &entries);
        self.menu_commands = commands;
    }

    /// Refreshes the item every few seconds (also while the window is
    /// hidden) and runs picked entries with the app brought forward.
    pub(super) fn watch_menu_bar(&mut self, cx: &mut Context<Self>) {
        self.sync_menu_bar();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(3))
                    .await;
                if this.update(cx, |this, _| this.sync_menu_bar()).is_err() {
                    break;
                }
            }
        })
        .detach();
        let picks = menu_bar::receiver();
        cx.spawn(async move |this, cx| {
            while let Ok(index) = picks.recv().await {
                let alive = this.update(cx, |this, cx| {
                    if let Some(Some(command)) = this.menu_commands.get(index).cloned() {
                        cx.activate(true);
                        this.run(command, cx);
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}
