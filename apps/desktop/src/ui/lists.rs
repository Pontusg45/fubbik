//! Keyboard lists: j/k (↓/↑) move a highlight in the current view, ⏎ does
//! the row's main thing, and single letters do the rest. Active only while
//! nothing that types (a field, a terminal) has focus. ⇧⌘E hands j/k to
//! the sidebar until ⏎ or esc.

use gpui::Keystroke;

use super::*;

/// j/k on a list of `len` rows; None for any other key.
fn step(cursor: Option<usize>, len: usize, key: &str) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match key {
        "j" | "down" => Some(cursor.map_or(0, |c| (c + 1).min(len - 1))),
        "k" | "up" => Some(cursor.map_or(0, |c| c.saturating_sub(1))),
        _ => None,
    }
}

impl Workspace {
    /// True when the key was used.
    pub(super) fn on_list_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mods = &keystroke.modifiers;
        if mods.platform || mods.control || mods.alt {
            return false;
        }
        let key = keystroke.key.as_str();
        if self.sidebar_nav {
            return self.on_sidebar_key(key);
        }
        if self.home {
            // Home: j/k over the project cards, ⏎ opens one.
            if let Some(cursor) = step(self.list_cursor, self.repos.len(), key) {
                self.list_cursor = Some(cursor);
                return true;
            }
            return match (key, self.list_cursor) {
                ("enter", Some(cursor)) => {
                    self.select(cursor);
                    self.tab = Tab::Overview;
                    true
                }
                _ => false,
            };
        }
        if self.tab == Tab::Plans {
            return self.on_board_key(key, cx);
        }
        let len = self.list_len();
        if let Some(cursor) = step(self.list_cursor, len, key) {
            self.list_cursor = Some(cursor);
            return true;
        }
        let Some(cursor) = self.list_cursor.filter(|c| *c < len) else {
            return false;
        };
        let repo = self.selected;
        match self.tab {
            Tab::Today => {
                let Some(command) = self
                    .today_rows()
                    .into_iter()
                    .nth(cursor)
                    .and_then(|r| r.primary)
                else {
                    return false;
                };
                match key {
                    "enter" => self.run(command, cx),
                    _ => return false,
                }
            }
            Tab::Sessions => {
                let Some(session) = self.session_order(repo).into_iter().nth(cursor) else {
                    return false;
                };
                match (key, session.pane.clone()) {
                    ("enter", Some(pane)) => self.open_pane(pane, session.cwd.clone(), cx),
                    ("enter", None) if !session.state.is_live() => self.run(
                        Command::ResumeSession(
                            session.agent,
                            session.id.clone(),
                            session.cwd.clone(),
                        ),
                        cx,
                    ),
                    ("r", Some(pane)) => self.toggle_reply(pane, window),
                    _ => return false,
                }
            }
            Tab::Processes => {
                let Some(process) = self.repos[repo]
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.processes.get(cursor))
                    .cloned()
                else {
                    return false;
                };
                match key {
                    "enter" | "s" if process.owned => {
                        self.run(Command::StopProcess(repo, cursor), cx)
                    }
                    "enter" | "s" => self.run(Command::StartProcess(repo, cursor), cx),
                    "r" => self.run(Command::RestartProcess(repo, cursor), cx),
                    "l" => {
                        self.log_key = Some(process.key.clone());
                        self.log_paused = None;
                        self.tab = Tab::Logs;
                    }
                    "o" => match process.spec.port {
                        Some(port) => cx.open_url(&format!("http://localhost:{port}")),
                        None => return false,
                    },
                    _ => return false,
                }
            }
            Tab::Worktrees => {
                let Some(worktree) = self.repos[repo]
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.worktrees.as_ref()?.as_ref().ok()?.get(cursor).cloned())
                else {
                    return false;
                };
                let path = worktree.path.clone();
                match key {
                    "enter" | "c" if !worktree.missing => self.load_branch_context(repo, path, cx),
                    "v" if !worktree.main && !worktree.missing => {
                        self.run(Command::ReviewWorktree { repo, path }, cx)
                    }
                    "t" if !worktree.missing => self.run(Command::ShellAt(path), cx),
                    "n" if !worktree.missing => self.run(Command::NewSession(path), cx),
                    "x" if !worktree.main => self.run(Command::RemoveWorktree(repo, path), cx),
                    _ => return false,
                }
            }
            Tab::Knowledge => {
                let Some(Ok(knowledge)) = &self.repos[repo].knowledge else {
                    return false;
                };
                let flags = knowledge.stale_flags.len();
                if cursor < flags {
                    let flag = knowledge.stale_flags[cursor].clone();
                    match key {
                        "d" => self.dismiss_stale(flag.id, cx),
                        "enter" | "o" => {
                            self.run(Command::OpenWeb(format!("/chunks/{}", flag.chunk_id)), cx)
                        }
                        _ => return false,
                    }
                } else {
                    let Some(proposal) = knowledge.proposals.get(cursor - flags).cloned() else {
                        return false;
                    };
                    match key {
                        "a" => self.review(proposal.id, true, cx),
                        "r" => self.review(proposal.id, false, cx),
                        "enter" | "o" => self.run(
                            Command::OpenWeb(format!("/chunks/{}", proposal.chunk_id)),
                            cx,
                        ),
                        _ => return false,
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Rows in the current view's keyboard list.
    fn list_len(&self) -> usize {
        let snapshot = self
            .repos
            .get(self.selected)
            .and_then(|r| r.snapshot.as_ref());
        match self.tab {
            Tab::Today => self.today_rows().len(),
            Tab::Sessions => self.session_order(self.selected).len(),
            Tab::Processes => snapshot.map_or(0, |s| s.processes.len()),
            Tab::Worktrees => snapshot
                .and_then(|s| s.worktrees.as_ref()?.as_ref().ok())
                .map_or(0, Vec::len),
            Tab::Knowledge => match self
                .repos
                .get(self.selected)
                .and_then(|r| r.knowledge.as_ref())
            {
                Some(Ok(k)) => k.stale_flags.len() + k.proposals.len(),
                _ => 0,
            },
            _ => 0,
        }
    }

    fn on_sidebar_key(&mut self, key: &str) -> bool {
        let len = self.repos.len();
        match key {
            "enter" | "escape" => self.sidebar_nav = false,
            _ => match step(Some(self.selected), len, key) {
                Some(next) => self.select(next),
                None => return false,
            },
        }
        true
    }

    /// The board: h/l pick a column, j/k a card, ←/→ move the card,
    /// space opens its checklist, s starts it, t its timeline, b blocks it.
    fn on_board_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let repo = self.selected;
        let Some(plan) = self.board_plan_detail(repo) else {
            return false;
        };
        let columns = plans::columns(&plan);
        let (mut col, mut row) = self.board_cursor.unwrap_or((0, 0));
        let non_empty: Vec<usize> = (0..columns.len())
            .filter(|c| !columns[*c].is_empty())
            .collect();
        if non_empty.is_empty() {
            return false;
        }
        if !non_empty.contains(&col) {
            col = non_empty[0];
            row = 0;
        }
        match key {
            "h" | "l" => {
                let at = non_empty.iter().position(|c| *c == col).unwrap_or(0);
                let next = if key == "h" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(non_empty.len() - 1)
                };
                col = non_empty[next];
                row = row.min(columns[col].len() - 1);
            }
            "j" | "down" | "k" | "up" => {
                row = step(Some(row), columns[col].len(), key).unwrap_or(row)
            }
            _ => {
                let Some(task) = columns[col].get(row).cloned() else {
                    return false;
                };
                let (p, t) = (plan.id.clone(), task.id.clone());
                match key {
                    "left" | "right" => {
                        let (back, forward) = plans::neighbours(&task.status);
                        let Some(to) = (if key == "left" { back } else { forward }) else {
                            return false;
                        };
                        self.run(
                            Command::SetTaskStatus {
                                plan: p,
                                task: t,
                                status: to,
                            },
                            cx,
                        );
                    }
                    "space" => self.run(Command::ToggleChecklist(t), cx),
                    "s" if task.status == "pending" => self.run(
                        Command::StartTask {
                            repo,
                            plan: p,
                            task: t,
                            new_worktree: true,
                            background: false,
                        },
                        cx,
                    ),
                    "b" => self.run(
                        Command::SetTaskStatus {
                            plan: p,
                            task: t,
                            status: "blocked",
                        },
                        cx,
                    ),
                    "enter" | "t" => self.run(
                        Command::TaskTimeline {
                            repo,
                            plan: p,
                            task: t,
                        },
                        cx,
                    ),
                    _ => return false,
                }
            }
        }
        self.board_cursor = Some((col, row));
        true
    }

    /// Keys for the current view, for the hint bar under it.
    pub(super) fn list_hint(&self) -> Option<String> {
        if self.sidebar_nav {
            return Some("Sidebar: j/k pick a repo · ⏎ or esc back".into());
        }
        if self.home {
            return Some("j/k move · ⏎ open the project · ⌘1–⌘9 a tab of the selected repo · ⌘/ all shortcuts".into());
        }
        let view = match self.tab {
            Tab::Today => "Today",
            Tab::Processes => "Processes",
            Tab::Worktrees => "Worktrees",
            Tab::Sessions => "AI Sessions",
            Tab::Plans => "Plans",
            Tab::Knowledge => "Knowledge",
            _ => return None,
        };
        let keys = keymap::LIST_KEYS
            .iter()
            .find(|(group, _, _)| *group == view)
            .map(|(_, keys, what)| format!("{keys}: {what}"));
        Some(match keys {
            Some(keys) => format!("j/k move · ⏎ open · {keys} · ⌘/ all shortcuts"),
            None => "j/k move · ⏎ open · ⌘/ all shortcuts".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j_and_k_stay_inside_the_list() {
        // Given
        let len = 3;

        // When
        let moves = [
            step(None, len, "j"),
            step(Some(2), len, "j"),
            step(Some(0), len, "k"),
            step(Some(1), len, "x"),
            step(None, 0, "j"),
        ];

        // Then
        assert_eq!(moves, [Some(0), Some(2), Some(0), None, None]);
    }
}
