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

impl Workspace {
    /// What a `fubbik://` link or the MCP `show` tool asks to see.
    fn link_command(&self, link: &crate::links::Link) -> Result<Command, String> {
        let tab = match &link.tab {
            None => None,
            Some(name) => Some(
                Tab::ALL
                    .into_iter()
                    .find(|t| {
                        t.label().eq_ignore_ascii_case(name)
                            || t.action_id()
                                .trim_start_matches("show_")
                                .eq_ignore_ascii_case(name)
                    })
                    .ok_or_else(|| format!("no tab named {name:?}"))?,
            ),
        };
        match &link.repo {
            Some(name) => {
                let repo = self
                    .repos
                    .iter()
                    .position(|r| {
                        r.entry.name() == *name || r.entry.path.to_string_lossy() == *name
                    })
                    .ok_or_else(|| format!("no repo named {name:?}"))?;
                Ok(Command::Focus {
                    repo,
                    tab: tab.filter(|t| !t.is_global()).unwrap_or(Tab::Overview),
                })
            }
            None => Ok(Command::ShowTab(tab.unwrap_or(Tab::Today))),
        }
    }

    pub(super) fn watch_links(&mut self, cx: &mut Context<Self>) {
        let links = crate::links::receiver();
        cx.spawn(async move |this, cx| {
            while let Ok(link) = links.recv().await {
                let alive = this.update(cx, |this, cx| {
                    cx.activate(true);
                    match this.link_command(&link) {
                        Ok(command) => this.run(command, cx),
                        Err(error) => this.show_error(format!("Link: {error}")),
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}
