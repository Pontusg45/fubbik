//! The selected repo: header, tabs, and one view per tab.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::palette::Command;
use super::widgets::{
    button, disabled_button, label, muted, muted_line, pill, row, section, sparkline, stat_card,
    status_dot, status_pill, top_highlight,
};
use super::{Field, RepoView, Tab, Workspace, display_path};
use crate::model::{ProcessState, RepoSnapshot, Status};
use crate::services::FILE_NAME;
use crate::services::ServicesFile;
use crate::theme;

const LOG_RENDER_LINES: usize = 500;

/// A repo's initial on its own color: the same in the sidebar and header.
pub(super) fn repo_badge(name: &str, size: f32) -> impl IntoElement {
    let color = theme::name_color(name);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_md()
        .bg(color.opacity(0.22))
        .border_1()
        .border_color(color.opacity(0.6))
        .text_color(color)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .child(
            name.chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
        )
}

/// Log chips for the built-in server, shown in every repo's Logs tab.
const BUILTIN_LOGS: [(&str, crate::backend::LogFile); 2] = [
    ("builtin::server", crate::backend::LogFile::Server),
    ("builtin::postgres", crate::backend::LogFile::Postgres),
];

impl Workspace {
    pub(super) fn render_empty(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("empty")
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .w_full()
                    .max_w(px(760.))
                    .pt_6()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Welcome to fubbik"))
                    .child(muted("Watch your repos' processes, databases, worktrees and AI agents, with fubbik's knowledge alongside."))
                    .child(self.render_checklist(cx))
                    .child(self.render_preferences(window, cx)),
            )
    }

    pub(super) fn render_repo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let index = self.selected;
        // The terminal fills the content area, without padding or scrolling.
        let terminal = (self.tab == Tab::Terminal)
            .then(|| self.terminal_for_selected(window, cx))
            .flatten();
        let mut insights = (self.tab == Tab::Insights).then(|| self.render_insights(index, cx));
        let hint = self.list_hint();
        let window: &Window = window;
        let repo = &self.repos[index];
        let body: AnyElement = match &repo.snapshot {
            None => muted("Checking…").into_any_element(),
            Some(snapshot) => match self.tab {
                Tab::Overview => self
                    .render_overview(index, snapshot, window, cx)
                    .into_any_element(),
                Tab::Processes => div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(self.render_processes(index, snapshot, cx))
                    .child(self.render_scripts(index, snapshot, window, cx))
                    .into_any_element(),
                Tab::Databases => self
                    .render_databases(index, snapshot, cx)
                    .into_any_element(),
                Tab::Logs => self.render_logs(repo, window, cx).into_any_element(),
                Tab::Knowledge => self.render_knowledge(index, window, cx).into_any_element(),
                Tab::Plans => self.render_plans(index, cx).into_any_element(),
                Tab::Setup => self.render_setup(index, window, cx).into_any_element(),
                Tab::Terminal => {
                    let dir = self.repos[index].entry.path.clone();
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap_2()
                        .child(div().text_sm().child("No terminals"))
                        .child(muted("Shells and AI sessions you open here keep running when you switch away."))
                        .child(button("terminal-empty-new", "+ Shell").on_click(cx.listener(move |this, _, _, cx| {
                            this.run(Command::ShellAt(dir.clone()), cx)
                        })))
                        .into_any_element()
                }
                Tab::Worktrees => self
                    .render_worktrees(index, snapshot, window, cx)
                    .into_any_element(),
                Tab::Sessions => self
                    .render_sessions(index, snapshot, window, cx)
                    .into_any_element(),
                Tab::Agents => self.render_all_agents(window, cx),
                Tab::Today => self.render_today(cx),
                Tab::Insights => insights
                    .take()
                    .unwrap_or_else(|| muted("…").into_any_element()),
            },
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .child(self.render_header(index, cx))
            .child(self.render_tabs(cx))
            .child(match terminal {
                Some(view) => {
                    let title = view.read(cx).title();
                    let second = self
                        .split_with
                        .filter(|id| Some(*id) != self.active_terminal)
                        .and_then(|id| self.terminals.iter().find(|t| t.id == id))
                        .map(|t| t.view.clone());
                    let repo_path = self.repos[index].entry.path.clone();
                    let active = self.active_terminal;
                    let chips: Vec<AnyElement> = self
                        .terminals_for_selected()
                        .iter()
                        .map(|tab| {
                            let id = tab.id;
                            let place = if tab.dir == repo_path {
                                String::new()
                            } else {
                                format!(
                                    " · {}",
                                    tab.dir
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_default()
                                )
                            };
                            let is_active = active == Some(id);
                            div()
                                .id(SharedString::from(format!("term-tab-{id}")))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(if is_active {
                                    theme::accent()
                                } else {
                                    theme::border()
                                })
                                .when(is_active, |d| d.bg(theme::surface_hover()))
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.active_terminal = Some(id);
                                    this.focus_terminal = true;
                                    cx.notify();
                                }))
                                .child(div().size(px(6.)).rounded_full().bg(
                                    if tab.label.starts_with("Claude")
                                        || tab.label.starts_with("Task")
                                    {
                                        theme::agent(crate::sessions::Agent::ClaudeCode)
                                    } else if tab.label.starts_with("Codex") {
                                        theme::agent(crate::sessions::Agent::Codex)
                                    } else if tab.label.starts_with("tmux") {
                                        theme::hue(theme::Hue::Violet)
                                    } else if tab.label.starts_with("PR")
                                        || tab.label.starts_with("Merge")
                                    {
                                        theme::hue(theme::Hue::Pink)
                                    } else {
                                        theme::text_muted()
                                    },
                                ))
                                .child(format!("{}{place}", tab.label))
                                .child(
                                    div()
                                        .id(SharedString::from(format!("term-close-{id}")))
                                        .text_color(theme::text_muted())
                                        .hover(|d| d.text_color(theme::danger()))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.run(Command::CloseTerminal(id), cx);
                                        }))
                                        .child("✕"),
                                )
                                .into_any_element()
                        })
                        .collect();
                    let copy = view.clone();
                    let new_shell_dir = repo_path.clone();
                    let header = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .bg(theme::surface())
                        .border_b_1()
                        .border_color(theme::border())
                        .children(chips)
                        .child(button("terminal-new", "+ Shell").on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.open_terminal(new_shell_dir.clone(), "Shell".into(), None, cx);
                                this.focus_terminal = true;
                                cx.notify();
                            },
                        )))
                        .child(div().flex_1())
                        .child(muted(title))
                        .child(
                            button(
                                "terminal-split",
                                if self.split_with.is_some() {
                                    "Unsplit"
                                } else {
                                    "Split"
                                },
                            )
                            .on_click(
                                cx.listener(|this, _, _, cx| this.run(Command::ToggleSplit, cx)),
                            ),
                        )
                        .child(button("terminal-copy", "Copy").on_click(cx.listener(
                            move |this, _, _, cx| {
                                copy.update(cx, |v, cx| v.copy_screen(cx));
                                this.show_info("Copied");
                            },
                        )));
                    // The frame: a raised, rounded panel; the tab strip is its
                    // header and the shell sits in a recessed well below it.
                    let frame = div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .relative()
                        .overflow_hidden()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::terminal_well())
                        .shadow(theme::frame_shadow())
                        .child(header)
                        .child(match second {
                            // Side by side, split by a border.
                            Some(second) => div()
                                .flex()
                                .flex_1()
                                .overflow_hidden()
                                .child(div().flex_1().overflow_hidden().px_3().py_2().child(view))
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .px_3()
                                        .py_2()
                                        .border_l_1()
                                        .border_color(theme::border())
                                        .child(second),
                                ),
                            None => div().flex_1().overflow_hidden().px_3().py_2().child(view),
                        })
                        .child(top_highlight());
                    div()
                        .id("content")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        .p_4()
                        .child(frame)
                        .into_any_element()
                }
                None => div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .id("content")
                            .flex_1()
                            .overflow_y_scroll()
                            .p_6()
                            .child(body),
                    )
                    .children(hint.map(|hint| {
                        div()
                            .flex_none()
                            .px_6()
                            .py_1p5()
                            .border_t_1()
                            .border_color(theme::border())
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(hint)
                    }))
                    .into_any_element(),
            })
    }

    fn render_header(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let repo = &self.repos[index];
        let status = repo
            .snapshot
            .as_ref()
            .map_or(Status::Stopped, |s| s.status());
        let git = match repo.snapshot.as_ref().and_then(|s| s.git.as_ref()) {
            Some(Ok(git)) => {
                let mut parts = vec![format!("{} @ {}", git.branch, git.head)];
                if git.ahead > 0 || git.behind > 0 {
                    parts.push(format!("↑{} ↓{}", git.ahead, git.behind));
                }
                parts.push(format!("{} changed", git.changed_files));
                parts.join("  ·  ")
            }
            Some(Err(_)) => "not a git repo".into(),
            None => String::new(),
        };
        let subtitle = [display_path(&repo.entry.path), git]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("  ·  ");
        let processes = repo
            .snapshot
            .as_ref()
            .map_or(&[][..], |s| s.processes.as_slice());
        let (all_up, none_ours) = (
            !processes.is_empty() && processes.iter().all(|p| p.status.is_up()),
            processes.iter().all(|p| !p.owned),
        );
        let (working, waiting) = repo.snapshot.as_ref().map_or((0, 0), |s| {
            use crate::sessions::SessionState;
            (
                s.sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Busy)
                    .count(),
                s.sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Idle)
                    .count(),
            )
        });
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(repo_badge(&repo.entry.name(), 24.))
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(repo.entry.name()),
                                    ),
                            )
                            .child(match &repo.snapshot {
                                Some(_) if processes.is_empty() => div().into_any_element(),
                                Some(_) => status_pill(status).into_any_element(),
                                None => pill("Checking…", theme::text_muted()).into_any_element(),
                            })
                            // Agents are their own state: busy agents are not "stopped".
                            .when(working + waiting > 0, |d| {
                                d.child(pill(
                                    match (working, waiting) {
                                        (w, 0) => format!("{w} agent(s) working"),
                                        (0, q) => format!("{q} waiting for you"),
                                        (w, q) => format!("{w} working · {q} waiting"),
                                    },
                                    if waiting > 0 {
                                        theme::status(Status::Degraded)
                                    } else {
                                        theme::agent(crate::sessions::Agent::ClaudeCode)
                                    },
                                ))
                            }),
                    )
                    .child(muted_line(subtitle)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_none()
                    // Only when there is something to start or stop.
                    .when(!processes.is_empty(), |d| {
                        d.child(if all_up {
                            disabled_button("start-all", "▶ Start all").into_any_element()
                        } else {
                            button("start-all", "▶ Start all")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(Command::StartAll(index), cx)
                                }))
                                .into_any_element()
                        })
                        .child(if none_ours {
                            disabled_button("stop-all", "■ Stop all").into_any_element()
                        } else {
                            button("stop-all", "■ Stop all")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(Command::StopAll(index), cx)
                                }))
                                .into_any_element()
                        })
                    })
                    .child(button("open-editor", "Editor").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::OpenEditor(index), cx)),
                    ))
                    .child(
                        button("open-terminal", "Terminal.app").on_click(cx.listener(
                            move |this, _, _, cx| this.run(Command::OpenTerminal(index), cx),
                        )),
                    )
                    .child(button("open-github", "GitHub").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::OpenGitHub(index), cx)),
                    )),
            )
    }

    /// What a tab holds that needs you, for its badge.
    fn tab_count(&self, tab: Tab) -> usize {
        let Some(view) = self.repos.get(self.selected) else {
            return 0;
        };
        let snapshot = view.snapshot.as_ref();
        match tab {
            Tab::Sessions => snapshot.map_or(0, |s| {
                s.sessions
                    .iter()
                    .filter(|s| s.state == crate::sessions::SessionState::Idle)
                    .count()
            }),
            Tab::Processes => snapshot.map_or(0, |s| {
                s.processes
                    .iter()
                    .filter(|p| matches!(p.status, Status::Error | Status::Degraded))
                    .count()
            }),
            Tab::Worktrees => snapshot.map_or(0, |s| s.overlaps.len()),
            Tab::Knowledge => match &view.knowledge {
                Some(Ok(k)) => {
                    (k.requirements.failing + k.violated_cells() + k.stale).max(0) as usize
                }
                _ => 0,
            },
            _ => 0,
        }
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Today and All agents are global views (sidebar); the strip is this repo's.
        let tabs = Tab::ALL.into_iter().filter(|t| !t.is_global()).map(|tab| {
            let active = tab == self.tab;
            let count = self.tab_count(tab);
            let underline = if tab == Tab::Setup {
                theme::accent()
            } else {
                tab.color()
            };
            div()
                .id(tab.label())
                .flex()
                .items_center()
                .gap_1p5()
                .px_3()
                .py_2()
                .text_sm()
                .cursor_pointer()
                .border_b_2()
                .flex_none()
                .border_color(if active {
                    underline
                } else {
                    gpui::transparent_black()
                })
                .text_color(if active {
                    theme::text()
                } else {
                    theme::text_muted()
                })
                .hover(|this| this.text_color(theme::text()))
                .on_click(cx.listener(move |this, _, _, cx| this.run(Command::ShowTab(tab), cx)))
                .child(
                    div()
                        .size(px(6.))
                        .rounded_full()
                        .bg(tab.color().opacity(if active { 1.0 } else { 0.5 })),
                )
                .child(tab.label())
                .when(count > 0, |d| {
                    d.child(
                        div()
                            .px_1p5()
                            .rounded_full()
                            .text_xs()
                            .bg(theme::status(Status::Degraded).opacity(0.2))
                            .text_color(theme::status(Status::Degraded))
                            .child(count.to_string()),
                    )
                })
                // The key only on the active tab (⌘/ lists them all).
                .when(active, |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(tab.shortcut()),
                    )
                })
        });
        div()
            .id("tabs")
            .flex()
            .gap_1()
            .px_6()
            .overflow_x_scroll()
            .border_b_1()
            .border_color(theme::border())
            .children(tabs)
    }

    fn render_overview(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let repo = &self.repos[index];
        let git = match &snapshot.git {
            Some(Ok(git)) => format!("{} · {} changed", git.branch, git.changed_files),
            _ => "—".into(),
        };
        let knowledge = match &repo.knowledge {
            Some(Ok(k)) if k.space.is_some() => format!("{} chunks · {} stale", k.chunks, k.stale),
            Some(Ok(_)) => "no space".into(),
            Some(Err(_)) => "offline".into(),
            None => "…".into(),
        };
        let cards = div()
            .flex()
            .gap_4()
            .child(stat_card(
                "Processes",
                format!(
                    "{} / {} up",
                    snapshot.processes_up(),
                    snapshot.processes.len()
                ),
            ))
            .child(stat_card(
                "Databases",
                format!(
                    "{} / {} up",
                    snapshot.databases_up(),
                    snapshot.databases.len()
                ),
            ))
            .child(stat_card("Git", git))
            .child(stat_card("Knowledge", knowledge))
            .child(stat_card(
                "AI & worktrees",
                format!(
                    "{} live · {} trees",
                    snapshot.live_sessions(),
                    snapshot.worktree_paths().len()
                ),
            ));

        let last_commit = match &snapshot.git {
            Some(Ok(git)) if !git.last_commit.is_empty() => {
                Some(muted(format!("Last commit: {}", git.last_commit)))
            }
            _ => None,
        };

        div()
            .flex()
            .flex_col()
            .gap_6()
            .children(self.render_problem(index, snapshot, cx))
            .child(cards)
            .children(last_commit)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(label("GROUP"))
                    .child(div().w(px(220.)).child(self.field(
                        Field::Group,
                        "none — type and press enter",
                        window,
                        cx,
                    ))),
            )
            .when(
                snapshot.has_services_file && snapshot.services_error.is_none(),
                |view| {
                    view.child(self.render_processes(index, snapshot, cx))
                        .child(self.render_databases(index, snapshot, cx))
                },
            )
    }

    /// Missing folder, broken services file, or none yet.
    fn render_problem(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let callout = |title: String, detail: String| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(theme::surface())
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(muted(detail))
        };
        if snapshot.missing {
            return Some(
                callout(
                    "Folder not found".into(),
                    "The repo was moved or deleted. It stays registered until you remove it."
                        .into(),
                )
                .into_any_element(),
            );
        }
        if let Some(error) = &snapshot.services_error {
            return Some(
                callout(format!("{FILE_NAME} has an error"), error.clone()).into_any_element(),
            );
        }
        if !snapshot.has_services_file {
            return Some(
                callout(
                    format!("No {FILE_NAME} yet"),
                    "It lists the processes and databases to watch. Generate a starter from package.json and the compose file, then edit it.".into(),
                )
                .child(
                    button("generate-services", "Generate from repo")
                        .on_click(cx.listener(move |this, _, _, cx| this.run(Command::GenerateServices(index), cx))),
                )
                .into_any_element(),
            );
        }
        None
    }

    fn render_processes(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = snapshot
            .processes
            .iter()
            .enumerate()
            .map(|(p, process)| self.process_row(index, p, process, cx))
            .collect();
        if snapshot.processes.is_empty() {
            let empty = row()
                .child(div().flex_1().child(muted(format!(
                    "No processes: list them in {} (the app can draft one from this repo)",
                    crate::services::FILE_NAME
                ))))
                .child(
                    button(("processes-generate", index), "Generate services file").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.run(Command::GenerateServices(index), cx)
                        }),
                    ),
                )
                .into_any_element();
            return section("Processes", vec![empty]);
        }
        let up = snapshot
            .processes
            .iter()
            .filter(|p| p.status.is_up())
            .count();
        section(
            format!("Processes · {up} of {} up", snapshot.processes.len()),
            rows,
        )
    }

    /// Run profiles and the repo's scripts, each a click away.
    fn render_scripts(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let profiles: Vec<String> = ServicesFile::load(&self.repos[index].entry.path)
            .ok()
            .flatten()
            .map(|s| s.profile.into_keys().collect())
            .unwrap_or_default();
        let profile_row = (!profiles.is_empty()).then(|| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(label("PROFILES"))
                .children(profiles.into_iter().enumerate().map(|(i, name)| {
                    let profile = name.clone();
                    button(("profile", i), format!("▶ {name}")).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.run(
                                Command::StartProfile {
                                    repo: index,
                                    profile: profile.clone(),
                                },
                                cx,
                            )
                        },
                    ))
                }))
                .child(muted("starts these, stops the repo's others"))
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(profile_row)
            .child(self.render_scripts_panel(index, snapshot, window, cx))
    }

    fn process_row(
        &self,
        repo: usize,
        p: usize,
        process: &ProcessState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spec = &process.spec;
        let id = |name: &str| SharedString::from(format!("{name}-{repo}-{p}"));
        let up = process.status.is_up();
        // The app's own built-in server holds this port: starting the
        // process would only collide with it.
        let builtin = spec
            .port
            .is_some_and(|port| port == crate::backend::api_port())
            && matches!(
                crate::backend::status(),
                crate::backend::Status::Running { .. }
            );
        let session = self.supervisor.tmux_session(&process.key);
        let state = match (builtin, process.owned, up) {
            (true, _, _) => "served by the app's built-in server".to_string(),
            (_, true, _) if session.is_some() => format!(
                "{} · tmux session {}",
                process.status.label(),
                session.as_deref().unwrap_or_default()
            ),
            (_, true, _) => format!("{} · started here", process.status.label()),
            (_, false, true) => format!("{} · started outside the app", process.status.label()),
            (_, false, false) => process.status.label().to_string(),
        };
        let status_color = if builtin {
            theme::status(Status::Running)
        } else {
            theme::status(process.status)
        };

        let mut actions = div().flex().justify_end().gap_1().w(px(260.)).flex_none();
        if builtin {
            // Nothing to start or stop here; Setup controls the built-in server.
        } else if process.owned {
            actions = actions
                .child(button(id("stop"), "Stop").on_click(
                    cx.listener(move |this, _, _, cx| this.run(Command::StopProcess(repo, p), cx)),
                ))
                .child(button(id("restart"), "Restart").on_click(cx.listener(
                    move |this, _, _, cx| this.run(Command::RestartProcess(repo, p), cx),
                )));
        } else if let (true, Some(pid)) = (up, process.pid) {
            let name = spec.name.clone();
            actions = actions.child(
                button(id("kill"), "Kill")
                    .text_color(theme::danger())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run(
                            Command::KillPid {
                                pid,
                                what: format!("{name} (pid {pid})"),
                            },
                            cx,
                        );
                        cx.notify();
                    })),
            );
        } else {
            actions = actions.child(button(id("start"), "Start").on_click(
                cx.listener(move |this, _, _, cx| this.run(Command::StartProcess(repo, p), cx)),
            ));
        }
        if let Some(session) = session.clone() {
            actions = actions.child(button(id("attach"), "Terminal").on_click(cx.listener(
                move |this, _, _, cx| this.run(Command::AttachRun(session.clone()), cx),
            )));
        }
        let key = process.key.clone();
        actions = actions.child(button(id("logs"), "Logs").on_click(cx.listener(
            move |this, _, _, cx| {
                this.log_key = Some(key.clone());
                this.log_paused = None;
                this.run(Command::ShowTab(Tab::Logs), cx);
            },
        )));

        // pid · memory · CPU, only while it runs.
        let live = (up && !builtin).then(|| {
            let mut facts = Vec::new();
            if let Some(pid) = process.pid {
                facts.push(format!("pid {pid}"));
            }
            if let Some(kb) = process.rss_kb {
                facts.push(format!("{:.0} MB", kb as f64 / 1024.0));
            }
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(muted(facts.join(" · ")))
                .child(sparkline(&process.cpu_history))
        });
        let port = spec.port.map(|port| {
            div()
                .id(id("port"))
                .text_xs()
                .when(up || builtin, |d| {
                    d.text_color(theme::accent())
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.open_url(&format!("http://localhost:{port}"))
                        }))
                })
                .when(!(up || builtin), |d| d.text_color(theme::text_muted()))
                .child(format!(":{port}"))
        });

        row()
            .when(
                self.tab == Tab::Processes && self.list_cursor == Some(p),
                |r| r.bg(theme::surface_hover()),
            )
            .child(div().size_2().flex_none().rounded_full().bg(status_color))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(spec.name.clone()),
                            )
                            .children(port),
                    )
                    .child(muted(format!("{} · {state}", spec.command))),
            )
            .child(
                div()
                    .w(px(220.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .children(live),
            )
            .child(actions)
            .into_any_element()
    }

    fn render_databases(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = snapshot
            .databases
            .iter()
            .enumerate()
            .map(|(d, db)| {
                let spec = &db.spec;
                let id = |name: &str| SharedString::from(format!("db-{name}-{index}-{d}"));
                let url = spec.url.clone();
                let mut actions = div().flex().gap_1().child(
                    button(id("copy"), "Copy URL")
                        .on_click(cx.listener(move |this, _, _, cx| this.copy(url.clone(), cx))),
                );
                let available = [
                    (
                        "start",
                        "Start",
                        spec.start_command().is_some() && !db.status.is_up(),
                    ),
                    (
                        "stop",
                        "Stop",
                        spec.stop_command().is_some() && db.status.is_up(),
                    ),
                    ("migrate", "Migrate", spec.migrate.is_some()),
                    ("seed", "Seed", spec.seed.is_some()),
                    ("studio", "Studio", spec.studio.is_some()),
                ];
                for (action, text, show) in available {
                    if show {
                        actions = actions.child(button(id(action), text).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.run(
                                    Command::DatabaseTask {
                                        repo: index,
                                        index: d,
                                        action,
                                    },
                                    cx,
                                );
                                cx.notify();
                            },
                        )));
                    }
                }
                row()
                    .child(status_dot(db.status))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(spec.name.clone()),
                            )
                            .child(muted(spec.engine.clone().unwrap_or_default())),
                    )
                    .child(muted(spec.url.clone()))
                    .child(
                        div()
                            .w(px(84.))
                            .flex()
                            .justify_end()
                            .child(status_pill(db.status)),
                    )
                    .child(actions)
                    .into_any_element()
            })
            .collect();
        section("Databases", rows)
    }

    /// A process's captured output, or a built-in server log file's tail.
    fn log_lines(&self, key: &str) -> Vec<String> {
        match BUILTIN_LOGS.iter().find(|(k, _)| *k == key) {
            Some((_, file)) => crate::backend::tail(&file.path(), 2000),
            None => self.supervisor.logs(key),
        }
    }

    fn render_logs(
        &self,
        repo: &RepoView,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let prefix = format!("{}::", repo.entry.path.display());
        let mut keys: Vec<String> = repo
            .snapshot
            .iter()
            .flat_map(|s| s.processes.iter().map(|p| p.key.clone()))
            .collect();
        for key in self.supervisor.log_keys(&prefix) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        // The built-in server's logs, the same in every repo.
        for (key, file) in BUILTIN_LOGS {
            if file.path().exists() {
                keys.push(key.to_string());
            }
        }
        let current = self
            .log_key
            .clone()
            .filter(|k| keys.contains(k))
            .or_else(|| keys.first().cloned());

        let chips = keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let name = match BUILTIN_LOGS.iter().find(|(k, _)| k == key) {
                    Some((_, crate::backend::LogFile::Server)) => "built-in server".to_string(),
                    Some((_, crate::backend::LogFile::Postgres)) => "built-in postgres".to_string(),
                    None => key.strip_prefix(&prefix).unwrap_or(key).to_string(),
                };
                let active = current.as_ref() == Some(key);
                let key = key.clone();
                button(("log-chip", i), name)
                    .when(active, |b| {
                        b.border_color(theme::accent()).bg(theme::surface_hover())
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.log_key = Some(key.clone());
                        this.log_paused = None;
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let lines = match (&self.log_paused, &current) {
            (Some(frozen), _) => frozen.clone(),
            (None, Some(key)) => self.log_lines(key),
            (None, None) => Vec::new(),
        };
        let filter = self.log_filter.text.to_lowercase();
        let shown: Vec<String> = lines
            .into_iter()
            .filter(|line| filter.is_empty() || line.to_lowercase().contains(&filter))
            .collect();
        let start = shown.len().saturating_sub(LOG_RENDER_LINES);
        let total = shown.len();

        let paused = self.log_paused.is_some();
        let pause_key = current.clone();
        let clear_key = current.clone();
        let toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(240.))
                    .child(self.field(Field::LogFilter, "Filter…", window, cx)),
            )
            .child(
                button("log-pause", if paused { "Resume" } else { "Pause" }).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.log_paused = match (&this.log_paused, &pause_key) {
                            (Some(_), _) | (None, None) => None,
                            (None, Some(key)) => Some(this.log_lines(key)),
                        };
                        cx.notify();
                    },
                )),
            )
            .child(
                button("log-clear", "Clear").on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(key) = &clear_key {
                        if this.supervisor.tmux_session(key).is_some() {
                            this.run(Command::ClearRunLog(key.clone()), cx);
                        } else {
                            this.supervisor.clear_logs(key);
                        }
                    }
                    this.log_paused = None;
                    cx.notify();
                })),
            )
            .child(muted(if total > LOG_RENDER_LINES {
                format!("last {LOG_RENDER_LINES} of {total} lines")
            } else {
                format!("{total} lines")
            }));

        let body = if current.is_none() {
            muted("No processes. Logs appear for processes started from this app.")
                .into_any_element()
        } else if total == 0 {
            muted("No output yet. Logs are captured for processes started from this app.")
                .into_any_element()
        } else {
            div()
                .id("log-lines")
                .track_scroll(&self.log_scroll)
                .overflow_y_scroll()
                // Scrolling up stops following new output; back at the
                // bottom, it follows again.
                .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                    let up = event.delta.pixel_delta(px(16.)).y > px(0.);
                    if up {
                        this.log_follow = false;
                    } else {
                        let max = this.log_scroll.max_offset().height;
                        let at = -this.log_scroll.offset().y;
                        this.log_follow = at >= max - px(24.);
                    }
                    cx.notify();
                }))
                .h(px(460.))
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(theme::input())
                .font_family("Menlo")
                .text_xs()
                .children(shown[start..].iter().map(|line| {
                    let row = div().child(line.clone());
                    match theme::log_line(line) {
                        Some(color) => row.text_color(color),
                        None => row,
                    }
                }))
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().flex_wrap().gap_1().children(chips))
            .child(toolbar)
            .child(body)
    }
}
