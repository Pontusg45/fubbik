//! The selected repo: header, tabs, and one view per tab.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::palette::Command;
use super::widgets::{
    button, label, muted, row, section, sparkline, stat_card, status_dot, status_pill,
    top_highlight,
};
use super::{Field, RepoView, Tab, Workspace, display_path};
use crate::model::{ProcessState, RepoSnapshot, Status};
use crate::services::FILE_NAME;
use crate::theme;

const LOG_RENDER_LINES: usize = 500;

impl Workspace {
    pub(super) fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No repos yet"),
            )
            .child(muted(
                "Register a repo to watch its processes, databases and git state.",
            ))
            .child(
                button("empty-register", "Register repo…")
                    .text_sm()
                    .on_click(cx.listener(|this, _, _, cx| this.run(Command::RegisterRepo, cx))),
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
        let window: &Window = window;
        let repo = &self.repos[index];
        let body: AnyElement = match &repo.snapshot {
            None => muted("Checking…").into_any_element(),
            Some(snapshot) => match self.tab {
                Tab::Overview => self
                    .render_overview(index, snapshot, window, cx)
                    .into_any_element(),
                Tab::Processes => self
                    .render_processes(index, snapshot, cx)
                    .into_any_element(),
                Tab::Databases => self
                    .render_databases(index, snapshot, cx)
                    .into_any_element(),
                Tab::Logs => self.render_logs(repo, window, cx).into_any_element(),
                Tab::Knowledge => self.render_knowledge(index, window, cx).into_any_element(),
                Tab::Plans => self.render_plans(index, cx).into_any_element(),
                Tab::Setup => self.render_setup(index, cx).into_any_element(),
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
                        .child(div().flex_1().overflow_hidden().px_3().py_2().child(view))
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
                    .id("content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_6()
                    .child(body)
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
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(repo.entry.name()),
                            )
                            .child(status_pill(status)),
                    )
                    .child(muted(format!(
                        "{}  ·  {git}",
                        display_path(&repo.entry.path)
                    ))),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("start-all", "▶ Start all").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::StartAll(index), cx)),
                    ))
                    .child(button("stop-all", "■ Stop all").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::StopAll(index), cx)),
                    ))
                    .child(button("open-editor", "Editor").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::OpenEditor(index), cx)),
                    ))
                    .child(button("open-terminal", "Terminal").on_click(cx.listener(
                        move |this, _, _, cx| this.run(Command::OpenTerminal(index), cx),
                    )))
                    .child(button("open-github", "GitHub").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::OpenGitHub(index), cx)),
                    )),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = Tab::ALL.into_iter().enumerate().map(|(i, tab)| {
            let active = tab == self.tab;
            div()
                .id(tab.label())
                .px_3()
                .py_2()
                .text_sm()
                .cursor_pointer()
                .border_b_2()
                .border_color(if active {
                    theme::accent()
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
                .child(format!("{}  ⌘{}", tab.label(), (i + 1) % 10))
        });
        div()
            .flex()
            .gap_2()
            .px_6()
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
        section("Processes", rows)
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
        let ownership = match (process.owned, process.status.is_up()) {
            (true, _) => "started here",
            (false, true) => "external",
            (false, false) => "",
        };
        let pid = process
            .pid
            .map(|pid| format!("pid {pid}"))
            .unwrap_or_default();
        let memory = process
            .rss_kb
            .map(|kb| format!("{:.0} MB", kb as f64 / 1024.0))
            .unwrap_or_default();

        let mut actions = div().flex().gap_1();
        if process.owned {
            actions = actions
                .child(button(id("stop"), "Stop").on_click(
                    cx.listener(move |this, _, _, cx| this.run(Command::StopProcess(repo, p), cx)),
                ))
                .child(button(id("restart"), "Restart").on_click(cx.listener(
                    move |this, _, _, cx| this.run(Command::RestartProcess(repo, p), cx),
                )));
        } else if let (true, Some(pid)) = (process.status.is_up(), process.pid) {
            actions = actions.child(
                button(id("kill"), format!("Kill {pid}"))
                    .text_color(theme::danger())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.kill_external(pid, cx);
                        cx.notify();
                    })),
            );
        } else {
            actions = actions.child(button(id("start"), "Start").on_click(
                cx.listener(move |this, _, _, cx| this.run(Command::StartProcess(repo, p), cx)),
            ));
        }
        let key = process.key.clone();
        actions = actions.child(button(id("logs"), "Logs").on_click(cx.listener(
            move |this, _, _, cx| {
                this.log_key = Some(key.clone());
                this.log_paused = None;
                this.run(Command::ShowTab(Tab::Logs), cx);
            },
        )));

        row()
            .child(status_dot(process.status))
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
                    .child(muted(spec.command.clone())),
            )
            .when_some(spec.port, |row, port| {
                row.child(
                    div()
                        .id(id("port"))
                        .text_xs()
                        .text_color(theme::accent())
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.open_url(&format!("http://localhost:{port}"))
                        }))
                        .child(format!(":{port}")),
                )
            })
            .child(div().w(px(80.)).child(muted(pid)))
            .child(div().w(px(80.)).child(muted(ownership)))
            .child(sparkline(&process.cpu_history))
            .child(div().w(px(56.)).child(muted(memory)))
            .child(
                div()
                    .w(px(84.))
                    .flex()
                    .justify_end()
                    .child(status_pill(process.status)),
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
                                this.database_task(index, d, action, cx);
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
        let current = self
            .log_key
            .clone()
            .filter(|k| keys.contains(k))
            .or_else(|| keys.first().cloned());

        let chips = keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let name = key.strip_prefix(&prefix).unwrap_or(key).to_string();
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
            (None, Some(key)) => self.supervisor.logs(key),
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
                            (None, Some(key)) => Some(this.supervisor.logs(key)),
                        };
                        cx.notify();
                    },
                )),
            )
            .child(
                button("log-clear", "Clear").on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(key) = &clear_key {
                        this.supervisor.clear_logs(key);
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
                .h(px(460.))
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(theme::input())
                .font_family("Menlo")
                .text_xs()
                .children(shown[start..].iter().map(|line| div().child(line.clone())))
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
