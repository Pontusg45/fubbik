//! The ⌘K palette: its entries, matching, and rendering.

use super::*;

/// The shortcut that does the same as a palette entry, if there is one.
fn command_keys(command: &Command) -> Option<String> {
    let action = match command {
        Command::ShowTab(tab) => tab.action_id().to_string(),
        Command::Refresh => "refresh".into(),
        Command::RegisterRepo => "register_repo".into(),
        Command::ToggleTheme => "toggle_theme".into(),
        Command::CaptureNote => "capture_note".into(),
        Command::ToggleSplit => "split_terminal".into(),
        Command::SelectRepo(i) if *i < 9 => format!("select_repo_{}", i + 1),
        Command::ReplyWaiting => "reply_latest".into(),
        Command::StartNextTask => "start_next_task".into(),
        Command::NextNeedsYou => "next_needs_you".into(),
        Command::ShowHome => "show_home".into(),
        _ => return None,
    };
    keymap::keys_for(&action)
}

/// What kind of thing a palette entry does, as a color: navigation blue,
/// processes green, agents orange, tasks pink, worktrees violet, scripts
/// by tool, themes yellow, the web app cyan, removing red.
fn command_color(command: &Command) -> gpui::Hsla {
    use theme::Hue;
    match command {
        Command::SelectRepo(_)
        | Command::ShowTab(_)
        | Command::Focus { .. }
        | Command::ShowLogs { .. } => theme::hue(Hue::Blue),
        Command::StartProcess(..)
        | Command::StopProcess(..)
        | Command::RestartProcess(..)
        | Command::StartAll(_)
        | Command::StopAll(_)
        | Command::StartProfile { .. } => theme::hue(Hue::Green),
        Command::NewSession(_) | Command::ResumeSession(..) => theme::hue(Hue::Orange),
        Command::StartTask { .. }
        | Command::TaskTimeline { .. }
        | Command::SetTaskStatus { .. } => theme::hue(Hue::Pink),
        Command::OpenEditorAt(_)
        | Command::OpenTerminalAt(_)
        | Command::ShellAt(_)
        | Command::ReviewWorktree { .. }
        | Command::CreateWorktree(_) => theme::hue(Hue::Violet),
        Command::RunScript { command, .. } => {
            theme::tool(command.split_whitespace().next().unwrap_or_default())
        }
        Command::SetColors(_) | Command::ToggleTheme => theme::hue(Hue::Yellow),
        Command::OpenWeb(_) | Command::OpenGitHub(_) => theme::hue(Hue::Cyan),
        Command::RemoveRepo(_) | Command::RemoveWorktree(..) => theme::danger(),
        _ => theme::text_muted(),
    }
}

/// Palette rows shown (and choosable) at once.
pub(super) const PALETTE_ROWS: usize = 12;

impl Workspace {
    /// The first character picks what the palette lists: `?` chunks, `@`
    /// repos, `#` tasks, `!` what is running (processes and agents), `>`
    /// commands only. With no prefix, commands, repos and tasks together.
    pub(super) fn palette_matches(&self) -> Vec<Entry> {
        let text = self.palette_field.text.as_str();
        let (entries, query) = match text.chars().next() {
            Some('?') => {
                return self
                    .palette_search
                    .iter()
                    .map(|chunk| Entry {
                        label: format!("Open chunk: {} ({})", chunk.title, chunk.kind),
                        command: Command::OpenWeb(format!("/chunks/{}", chunk.id)),
                    })
                    .collect();
            }
            Some('@') => (self.palette_repos(), &text[1..]),
            Some('#') => (self.palette_tasks(), &text[1..]),
            Some('!') => (self.palette_running(), &text[1..]),
            Some('>') => (self.palette_entries(), &text[1..]),
            _ => {
                let mut all = self.palette_entries();
                all.extend(self.palette_tasks());
                (all, text)
            }
        };
        let query = query.trim_start();
        let mut entries = entries;
        // Dozens of Ghostty themes would bury everything else: only when asked.
        if !query.to_lowercase().contains("theme") {
            entries.retain(
                |e| !matches!(&e.command, Command::SetColors(Some(name)) if name != "follow"),
            );
        }
        // The selected repo's entries first.
        let mine = self
            .repos
            .get(self.selected)
            .map(|r| format!("— {}", r.entry.name()));
        let mut matches = palette::filter_with(&entries, query, |e| {
            if mine.as_ref().is_some_and(|m| e.label.ends_with(m.as_str())) {
                8
            } else {
                0
            }
        });
        if query.is_empty() {
            palette::recent_first(&mut matches, &self.palette_recent);
        }
        matches
    }

    /// Runs the chosen entry; the reply box needs the window.
    pub(super) fn choose_from_palette(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if command == Command::ReplyWaiting {
            self.reply_latest(window, cx);
        } else {
            self.run_from_palette(command, cx);
        }
    }

    /// What the palette says when it lists nothing.
    fn palette_empty_text(&self) -> String {
        let text = self.palette_field.text.as_str();
        match text.strip_prefix('?').map(str::trim) {
            Some(terms) if terms.chars().count() < 2 => {
                "Type 2 or more characters to search chunks".into()
            }
            Some(_) if self.palette_searching => "Searching…".into(),
            Some(_) => match &self.palette_search_error {
                Some(error) => format!("Search failed: {error}"),
                None => "No chunks match".into(),
            },
            None => "No matching commands".into(),
        }
    }

    /// Runs a palette entry and remembers it as recent.
    pub(super) fn run_from_palette(&mut self, command: Command, cx: &mut Context<Self>) {
        self.palette_recent.retain(|c| *c != command);
        self.palette_recent.insert(0, command.clone());
        self.palette_recent.truncate(palette::RECENT);
        self.run(command, cx);
    }

    /// `@`: every repo, and its tabs.
    fn palette_repos(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        for (repo, view) in self.repos.iter().enumerate() {
            let name = view.entry.name();
            entries.push(Entry {
                label: name.clone(),
                command: Command::SelectRepo(repo),
            });
            for tab in [
                Tab::Overview,
                Tab::Processes,
                Tab::Sessions,
                Tab::Plans,
                Tab::Knowledge,
            ] {
                entries.push(Entry {
                    label: format!("{name} › {}", tab.label()),
                    command: Command::Focus { repo, tab },
                });
            }
        }
        entries
    }

    /// `#`: every task on every plan; ⏎ opens its timeline.
    fn palette_tasks(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        for (repo, view) in self.repos.iter().enumerate() {
            let Some(Ok(knowledge)) = &view.knowledge else {
                continue;
            };
            for plan in &knowledge.plan_details {
                for task in plan.tasks.iter().filter(|t| t.status != "done") {
                    entries.push(Entry {
                        label: format!(
                            "#{} ({}) — {} · {}",
                            task.title,
                            task.status.replace('_', " "),
                            plan.title,
                            view.entry.name()
                        ),
                        command: Command::TaskTimeline {
                            repo,
                            plan: plan.id.clone(),
                            task: task.id.clone(),
                        },
                    });
                }
            }
        }
        entries
    }

    /// `!`: processes that are up (⏎ shows the log) and live agents (⏎
    /// opens the pane, or the Sessions tab).
    fn palette_running(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        for (repo, view) in self.repos.iter().enumerate() {
            let name = view.entry.name();
            let Some(snapshot) = &view.snapshot else {
                continue;
            };
            for process in snapshot
                .processes
                .iter()
                .filter(|p| p.status != crate::model::Status::Stopped)
            {
                entries.push(Entry {
                    label: format!(
                        "{} ({}) — {name}",
                        process.spec.name,
                        process.status.label()
                    ),
                    command: Command::ShowLogs {
                        repo,
                        key: process.key.clone(),
                    },
                });
            }
            for session in snapshot.sessions.iter().filter(|s| s.state.is_live()) {
                let command = match &session.pane {
                    Some(pane) => Command::OpenPane {
                        pane: pane.clone(),
                        cwd: session.cwd.clone(),
                    },
                    None => Command::Focus {
                        repo,
                        tab: Tab::Sessions,
                    },
                };
                entries.push(Entry {
                    label: format!("{} “{}” — {name}", session.agent.label(), session.title),
                    command,
                });
            }
        }
        entries
    }

    pub(super) fn palette_entries(&self) -> Vec<Entry> {
        let mut entries = vec![
            Entry {
                label: "Register repo…".into(),
                command: Command::RegisterRepo,
            },
            Entry {
                label: "Refresh".into(),
                command: Command::Refresh,
            },
            Entry {
                label: "Today".into(),
                command: Command::ShowTab(Tab::Today),
            },
            Entry {
                label: "Capture note…".into(),
                command: Command::CaptureNote,
            },
            Entry {
                label: if self.config.menu_bar != Some(false) {
                    "Hide the menu bar item".into()
                } else {
                    "Show the menu bar item".into()
                },
                command: Command::SetMenuBar(self.config.menu_bar == Some(false)),
            },
            Entry {
                label: "Home".into(),
                command: Command::ShowHome,
            },
            Entry {
                label: "Reply to the waiting agent".into(),
                command: Command::ReplyWaiting,
            },
            Entry {
                label: "Start the next ready task".into(),
                command: Command::StartNextTask,
            },
            Entry {
                label: "Next thing that needs you".into(),
                command: Command::NextNeedsYou,
            },
            Entry {
                label: "Set up the projects tracker (.mcp.json + CLAUDE.md)".into(),
                command: Command::SetupTracker,
            },
            Entry {
                label: "Theme: Built-in".into(),
                command: Command::SetColors(None),
            },
            Entry {
                label: "Theme: Follow Ghostty config".into(),
                command: Command::SetColors(Some("follow".into())),
            },
            Entry {
                label: if theme::is_translucent() {
                    "Theme: switch to opaque".into()
                } else {
                    "Theme: switch to translucent".into()
                },
                command: Command::ToggleTheme,
            },
            Entry {
                label: "Open fubbik dashboard".into(),
                command: Command::OpenWeb("/dashboard".into()),
            },
            Entry {
                label: "Open knowledge health".into(),
                command: Command::OpenWeb("/knowledge-health".into()),
            },
            Entry {
                label: "Open knowledge graph".into(),
                command: Command::OpenWeb("/graph".into()),
            },
        ];
        entries.extend(Tab::ALL.map(|tab| Entry {
            label: format!("Show {}", tab.label()),
            command: Command::ShowTab(tab),
        }));
        entries.extend(self.ghostty_themes.iter().map(|name| Entry {
            label: format!("Theme: {name} (Ghostty)"),
            command: Command::SetColors(Some(name.clone())),
        }));
        for (r, view) in self.repos.iter().enumerate() {
            let name = view.entry.name();
            entries.push(Entry {
                label: format!("Go to {name}"),
                command: Command::SelectRepo(r),
            });
            entries.push(Entry {
                label: format!("Start all — {name}"),
                command: Command::StartAll(r),
            });
            entries.push(Entry {
                label: format!("Stop all — {name}"),
                command: Command::StopAll(r),
            });
            entries.push(Entry {
                label: format!("Open in editor — {name}"),
                command: Command::OpenEditor(r),
            });
            entries.push(Entry {
                label: format!("Open in terminal — {name}"),
                command: Command::OpenTerminal(r),
            });
            entries.push(Entry {
                label: format!("Open on GitHub — {name}"),
                command: Command::OpenGitHub(r),
            });
            if let Some(Ok(git)) = view.snapshot.as_ref().and_then(|s| s.git.as_ref()) {
                entries.push(Entry {
                    label: format!("Fetch — {name}"),
                    command: Command::Fetch(r),
                });
                if git.behind > 0 {
                    entries.push(Entry {
                        label: format!("Pull {} commit(s), fast-forward — {name}", git.behind),
                        command: Command::Pull(r),
                    });
                }
                if git.ahead > 0 {
                    entries.push(Entry {
                        label: format!("Push {} commit(s)… — {name}", git.ahead),
                        command: Command::Push(r),
                    });
                }
            }
            entries.push(Entry {
                label: format!("Remove repo — {name}"),
                command: Command::RemoveRepo(r),
            });
            entries.push(Entry {
                label: format!("Shell (embedded terminal) — {name}"),
                command: Command::ShellAt(view.entry.path.clone()),
            });
            for script in view.snapshot.iter().flat_map(|s| s.scripts.iter()) {
                entries.push(Entry {
                    label: format!("Run {} — {name}", script.command),
                    command: Command::RunScript {
                        repo: r,
                        name: script.name.clone(),
                        command: script.command.clone(),
                    },
                });
            }
            entries.push(Entry {
                label: format!("New Claude session — {name}"),
                command: Command::NewSession(view.entry.path.clone()),
            });
            let snapshot = view.snapshot.as_ref();
            for worktree in snapshot
                .and_then(|s| s.worktrees.as_ref()?.as_ref().ok())
                .into_iter()
                .flatten()
            {
                if worktree.main || worktree.missing {
                    continue;
                }
                let branch = worktree.name();
                entries.push(Entry {
                    label: format!("Open worktree {branch} in editor — {name}"),
                    command: Command::OpenEditorAt(worktree.path.clone()),
                });
                entries.push(Entry {
                    label: format!("New Claude session in {branch} — {name}"),
                    command: Command::NewSession(worktree.path.clone()),
                });
            }
            if let Some(Ok(knowledge)) = &view.knowledge {
                if let Some(space) = &knowledge.space {
                    entries.push(Entry {
                        label: format!("Open space in fubbik — {name}"),
                        command: Command::OpenWeb(format!("/spaces/{}", space.id)),
                    });
                }
                for plan in &knowledge.plan_details {
                    for task in plan.tasks.iter().filter(|t| t.status == "pending") {
                        entries.push(Entry {
                            label: format!(
                                "Start task “{}” in a new worktree — {}",
                                task.title, plan.title
                            ),
                            command: Command::StartTask {
                                repo: r,
                                plan: plan.id.clone(),
                                task: task.id.clone(),
                                new_worktree: true,
                                background: false,
                            },
                        });
                    }
                }
            }
            // A live session needs no resuming.
            for session in snapshot
                .iter()
                .flat_map(|s| s.sessions.iter())
                .filter(|s| !s.state.is_live())
                .take(10)
            {
                entries.push(Entry {
                    label: format!(
                        "Resume “{}” ({}) — {name}",
                        session.title,
                        session.agent.label()
                    ),
                    command: Command::ResumeSession(
                        session.agent,
                        session.id.clone(),
                        session.cwd.clone(),
                    ),
                });
            }
            for (p, process) in view
                .snapshot
                .iter()
                .flat_map(|s| s.processes.iter())
                .enumerate()
            {
                // Only what applies: Start when stopped, else Stop and Restart.
                let stopped = process.status == crate::model::Status::Stopped;
                let process = &process.spec.name;
                if stopped {
                    entries.push(Entry {
                        label: format!("Start {process} — {name}"),
                        command: Command::StartProcess(r, p),
                    });
                } else {
                    entries.push(Entry {
                        label: format!("Stop {process} — {name}"),
                        command: Command::StopProcess(r, p),
                    });
                    entries.push(Entry {
                        label: format!("Restart {process} — {name}"),
                        command: Command::RestartProcess(r, p),
                    });
                }
            }
        }
        entries
    }

    pub(super) fn toggle_palette(&mut self, window: &mut Window) {
        self.palette_open = !self.palette_open;
        self.palette_field.text.clear();
        self.palette_selected = 0;
        if self.palette_open {
            self.palette_field.focus.focus(window);
        } else {
            self.focus.focus(window);
        }
    }

    pub(super) fn render_palette(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let matches = self.palette_matches();
        let empty = matches.is_empty().then(|| self.palette_empty_text());
        // Sized to the window, within bounds.
        let viewport = window.viewport_size();
        let width = (viewport.width * 0.8).min(px(640.)).max(px(320.));
        let height = (viewport.height * 0.7).min(px(460.)).max(px(220.));
        let selected = self
            .palette_selected
            .min(matches.len().min(PALETTE_ROWS).saturating_sub(1));
        let rows =
            matches
                .into_iter()
                .take(PALETTE_ROWS)
                .enumerate()
                .map(|(i, entry)| {
                    let command = entry.command.clone();
                    div()
                        .id(("palette", i))
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .text_sm()
                        .cursor_pointer()
                        .when(i == selected, |row| row.bg(theme::surface_hover()))
                        .hover(|row| row.bg(theme::surface_hover()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.toggle_palette(window);
                            this.choose_from_palette(command.clone(), window, cx);
                        }))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(7.))
                                .flex_none()
                                .rounded_full()
                                .bg(command_color(&entry.command)),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(entry.label))
                        .children(command_keys(&entry.command).map(|keys| {
                            div().text_xs().text_color(theme::text_muted()).child(keys)
                        }))
                })
                .collect::<Vec<_>>();
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .pt(px(90.))
            .bg(theme::overlay())
            .child(
                div()
                    .id("palette")
                    .w(width)
                    .h(height)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::elevated())
                    .shadow(theme::frame_shadow())
                    .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                        this.toggle_palette(window);
                        cx.notify();
                    }))
                    .child(self.field(
                        Field::Palette,
                        "Type a command…   @ repos  # tasks  ! running  > commands  ? chunks",
                        window,
                        cx,
                    ))
                    .child(div().flex().flex_col().children(rows))
                    .children(empty.map(|text| {
                        div()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .text_color(theme::text_muted())
                            .child(text)
                    })),
            )
    }
}
