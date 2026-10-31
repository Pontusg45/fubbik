//! The ⌘K palette: its entries, matching, and rendering.

use super::*;

/// What kind of thing a palette entry does, as a color: navigation blue,
/// processes green, agents orange, tasks pink, worktrees violet, scripts
/// by tool, themes yellow, the web app cyan, removing red.
fn command_color(command: &Command) -> gpui::Hsla {
    use theme::Hue;
    match command {
        Command::SelectRepo(_) | Command::ShowTab(_) | Command::Focus { .. } => {
            theme::hue(Hue::Blue)
        }
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

impl Workspace {
    /// `?query` searches chunks; anything else filters the commands.
    pub(super) fn palette_matches(&self) -> Vec<Entry> {
        if self.palette_field.text.starts_with('?') {
            return self
                .palette_search
                .iter()
                .map(|chunk| Entry {
                    label: format!("Open chunk: {} ({})", chunk.title, chunk.kind),
                    command: Command::OpenWeb(format!("/chunks/{}", chunk.id)),
                })
                .collect();
        }
        palette::filter(&self.palette_entries(), &self.palette_field.text)
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
            for session in snapshot.iter().flat_map(|s| s.sessions.iter()).take(10) {
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
                let process = &process.spec.name;
                entries.push(Entry {
                    label: format!("Start {process} — {name}"),
                    command: Command::StartProcess(r, p),
                });
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
        let selected = self.palette_selected.min(matches.len().saturating_sub(1));
        let rows = matches
            .into_iter()
            .take(12)
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
                        this.run(command.clone(), cx);
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
                    .child(entry.label)
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
                    .w(px(520.))
                    .h(px(420.))
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
                    .child(self.field(Field::Palette, "Type a command…", window, cx))
                    .child(div().flex().flex_col().children(rows)),
            )
    }
}
