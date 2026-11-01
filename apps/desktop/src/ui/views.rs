//! The selected repo: header, tabs, and one view per tab.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::palette::Command;
use super::widgets::{
    button, disabled_button, label, loading, muted, muted_line, pill, row, section, section_or,
    sparkline, stat_card, status_dot, status_pill, top_highlight,
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

/// `postgresql://user:secret@host/db` → `postgresql://user:•••@host/db`.
fn mask_password(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(scheme), Some(at)) if at > scheme => {
            let credentials = &url[scheme + 3..at];
            match credentials.split_once(':') {
                Some((user, _)) => format!("{}{user}:•••{}", &url[..scheme + 3], &url[at..]),
                None => url.to_string(),
            }
        }
        _ => url.to_string(),
    }
}

/// A log key's readable name: `script:just:test` → `test (just)`,
/// `postgres:migrate` → `postgres › migrate`.
fn log_label(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("script:") {
        return match rest.split_once(':') {
            Some((tool, script)) => format!("{script} ({tool})"),
            None => rest.to_string(),
        };
    }
    match name.split_once(':') {
        Some((what, task)) => format!("{what} › {task}"),
        None => name.to_string(),
    }
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
            None => loading("Reading git, processes, databases and sessions…").into_any_element(),
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
                    let claude_dir = dir.clone();
                    // Process and script runs in tmux are not tabs: offer them here.
                    let runs: Vec<AnyElement> =
                        crate::tmux::sessions(&crate::tmux::Server::fubbik())
                            .into_iter()
                            .filter(|s| s.name.starts_with(crate::tmux_runs::PREFIX))
                            .map(|s| {
                                let name = s.name.clone();
                                row()
                                    .child(div().flex_1().text_sm().child(s.label.clone()))
                                    .child(muted(s.name.clone()))
                                    .child(
                                        button(
                                            SharedString::from(format!("attach-{name}")),
                                            "Attach",
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.run(Command::AttachRun(name.clone()), cx)
                                            }),
                                        ),
                                    )
                                    .into_any_element()
                            })
                            .collect();
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap_2()
                        .child(div().text_sm().child("No terminals"))
                        .child(muted("Shells and AI sessions you open here keep running when you switch away."))
                                                .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(button("terminal-empty-new", "+ Shell").on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(Command::ShellAt(dir.clone()), cx)
                                })))
                                .child(button("terminal-empty-claude", "+ Claude").on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(Command::NewSession(claude_dir.clone()), cx)
                                }))),
                        )
                        .when(!runs.is_empty(), |d| d.child(section("Running in tmux", runs)))
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
                        // Many tabs scroll; the buttons stay put on the right.
                        .child(
                            div()
                                .id("terminal-chips")
                                .flex()
                                .flex_1()
                                .min_w(px(0.))
                                .gap_2()
                                .overflow_x_scroll()
                                .children(chips),
                        )
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
                        .child(button("terminal-copy", "Copy screen").on_click(cx.listener(
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
        // Today and All agents are global views (Home); the strip is this repo's.
        let tabs: Vec<Tab> = Tab::ALL.into_iter().filter(|t| !t.is_global()).collect();
        let active_index = tabs.iter().position(|t| *t == self.tab);
        let items = tabs
            .into_iter()
            .map(|tab| {
                let underline = if tab == Tab::Setup {
                    theme::accent()
                } else {
                    tab.color()
                };
                tab_item(
                    tab.label(),
                    tab.label(),
                    tab.color(),
                    underline,
                    tab == self.tab,
                    self.tab_count(tab),
                    tab.shortcut(),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.run(Command::ShowTab(tab), cx)))
            })
            .collect();
        // Keep the active tab in view when it changes (a key, the palette).
        // GPUI drops the request on a frame before the strip has its layout,
        // so ask again (and for another frame) until it has one.
        if active_index != self.tabs_scrolled_to.get() {
            if let Some(i) = active_index {
                self.tabs_scroll.scroll_to_item(i);
            }
            if self.tabs_scroll.bounds().size.width > px(0.) {
                self.tabs_scrolled_to.set(active_index);
            } else {
                let this = cx.entity();
                cx.defer(move |cx| this.update(cx, |_, cx| cx.notify()));
            }
        }
        tab_strip("tabs", &self.tabs_scroll, items, cx)
    }

    fn render_overview(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let repo = &self.repos[index];
        // The header has the branch and changes; this says where it stands.
        let git = match &snapshot.git {
            Some(Ok(git)) if git.ahead == 0 && git.behind == 0 => "in sync".to_string(),
            Some(Ok(git)) => format!("↑{} ↓{}", git.ahead, git.behind),
            _ => "—".into(),
        };
        let (knowledge, knowledge_tab) = match &repo.knowledge {
            Some(Ok(k)) if k.space.is_some() => (
                format!("{} chunks · {} stale", k.chunks, k.stale),
                Tab::Knowledge,
            ),
            Some(Ok(_)) => ("no space — set up".into(), Tab::Setup),
            Some(Err(_)) => ("offline — see Setup".into(), Tab::Setup),
            None => ("…".into(), Tab::Knowledge),
        };
        let (working, waiting) = {
            use crate::sessions::SessionState;
            (
                snapshot
                    .sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Busy)
                    .count(),
                snapshot
                    .sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Idle)
                    .count(),
            )
        };
        let worktrees = match &snapshot.worktrees {
            Some(Ok(list)) => list.iter().filter(|w| !w.main).count(),
            _ => 0,
        };
        // Every card opens the tab it summarises.
        let linked = |id: &'static str, card: gpui::Div, tab: Tab, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .flex_1()
                .min_w(px(160.))
                .cursor_pointer()
                .hover(|d| d.opacity(0.85))
                .on_click(cx.listener(move |this, _, _, cx| this.run(Command::ShowTab(tab), cx)))
                .child(card.w_full())
        };
        let cards = div()
            .flex()
            .flex_wrap()
            .gap_4()
            .child(linked(
                "card-processes",
                stat_card(
                    "Processes",
                    format!(
                        "{} / {} up",
                        snapshot.processes_up(),
                        snapshot.processes.len()
                    ),
                ),
                Tab::Processes,
                cx,
            ))
            .child(linked(
                "card-databases",
                stat_card(
                    "Databases",
                    format!(
                        "{} / {} up",
                        snapshot.databases_up(),
                        snapshot.databases.len()
                    ),
                ),
                Tab::Databases,
                cx,
            ))
            .child(linked("card-git", stat_card("Git", git), Tab::Insights, cx))
            .child(linked(
                "card-knowledge",
                stat_card("Knowledge", knowledge),
                knowledge_tab,
                cx,
            ))
            .child(linked(
                "card-agents",
                stat_card(
                    "Agents",
                    match (working, waiting) {
                        (0, 0) => "none live".to_string(),
                        (w, 0) => format!("{w} working"),
                        (0, q) => format!("{q} waiting"),
                        (w, q) => format!("{w} working · {q} waiting"),
                    },
                ),
                Tab::Sessions,
                cx,
            ))
            .child(linked(
                "card-worktrees",
                stat_card("Worktrees", worktrees.to_string()),
                Tab::Worktrees,
                cx,
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
            // Only processes that need a look; the full list is a click away.
            .when(
                snapshot
                    .processes
                    .iter()
                    .any(|p| matches!(p.status, Status::Error | Status::Degraded)),
                |view| {
                    let rows = snapshot
                        .processes
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| matches!(p.status, Status::Error | Status::Degraded))
                        .map(|(p, process)| self.process_row(index, p, process, cx))
                        .collect();
                    view.child(section("Needs a look", rows))
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
                    "The repo was moved or deleted. Point fubbik at its new place, or remove it."
                        .into(),
                )
                .border_color(theme::danger())
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(button("locate-repo", "Locate…").on_click(cx.listener(
                            move |this, _, _, cx| this.run(Command::LocateRepo(index), cx),
                        )))
                        .child(button("remove-missing", "Remove from fubbik").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.run(Command::RemoveRepo(index), cx)
                            }),
                        )),
                )
                .into_any_element(),
            );
        }
        if let Some(error) = &snapshot.services_error {
            return Some(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::danger())
                    .bg(theme::surface())
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{FILE_NAME} has an error")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("Menlo")
                            .text_color(theme::danger())
                            .child(error.clone()),
                    )
                    .child(button("services-edit", "Open in editor").on_click(
                        cx.listener(move |this, _, _, cx| this.run(Command::OpenEditor(index), cx)),
                    ))
                    .into_any_element(),
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
        let pending = self
            .pending
            .get(&process.key)
            .filter(|(at, what)| {
                at.elapsed() < std::time::Duration::from_secs(15) && (*what == "Starting…") != up
            })
            .map(|(_, what)| *what);
        let elsewhere = self
            .launched_in
            .get(&process.key)
            .copied()
            .filter(|_| up && !process.owned);
        let crashed = self
            .last_exit
            .get(&process.key)
            .filter(|(code, _)| *code != Some(0) && !up)
            .map(|(code, at)| match code {
                Some(code) => format!(
                    "exited {code} · {} ago",
                    crate::activity::short_age(at.elapsed().as_secs() as i64)
                ),
                None => format!(
                    "killed · {} ago",
                    crate::activity::short_age(at.elapsed().as_secs() as i64)
                ),
            });
        let state = match (builtin, process.owned, up) {
            _ if pending.is_some() => pending.unwrap_or_default().to_string(),
            (_, false, true) if elsewhere == Some(super::RunTarget::Tab) => {
                "Running in a terminal tab".to_string()
            }
            (_, false, true) if elsewhere == Some(super::RunTarget::TerminalApp) => {
                "Running in Terminal.app".to_string()
            }
            (_, _, false) if crashed.is_some() => crashed.clone().unwrap_or_default(),
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
        } else if crashed.is_some() {
            theme::danger()
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
        } else if elsewhere == Some(super::RunTarget::Tab) {
            actions = actions.child(button(id("show-tab"), "Show").on_click(
                cx.listener(|this, _, _, cx| this.run(Command::ShowTab(Tab::Terminal), cx)),
            ));
        } else if elsewhere.is_some() {
            // In Terminal.app: stop it there.
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
            actions = actions.child(button(id("attach"), "Attach").on_click(cx.listener(
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
                    // The state first: it is what changes and matters.
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(if crashed.is_some() {
                                        theme::danger()
                                    } else {
                                        theme::text_muted()
                                    })
                                    .child(state),
                            )
                            .child(muted_line(format!("· {}", spec.command))),
                    ),
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
                let repo_path = self.repos[index].entry.path.clone();
                let task_key = |action: &str| {
                    crate::model::process_key(&repo_path, &format!("{}:{action}", spec.name))
                };
                let mut actions = div().flex().flex_none().gap_1().child(
                    button(id("copy"), "Copy URL")
                        .on_click(cx.listener(move |this, _, _, cx| this.copy(url.clone(), cx))),
                );
                // The last task's result, and whether one runs now.
                let mut last: Option<(String, Option<i32>, std::time::Instant)> = None;
                for action in ["start", "stop", "migrate", "seed", "studio"] {
                    if let Some((code, at)) = self.last_exit.get(&task_key(action))
                        && last.as_ref().is_none_or(|(_, _, t)| at > t)
                    {
                        last = Some((action.to_string(), *code, *at));
                    }
                }
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
                    let running = self.supervisor.is_running(&task_key(action));
                    if running {
                        // Running now: say so; Studio can be stopped.
                        let key = task_key(action);
                        actions = actions.child(disabled_button(id(action), format!("{text}…")));
                        if action == "studio" {
                            actions =
                                actions.child(button(id("studio-stop"), "Stop Studio").on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.run(Command::StopKey(key.clone()), cx)
                                    }),
                                ));
                        }
                    } else if show {
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
                let logs_key = last.as_ref().map(|(action, _, _)| task_key(action));
                if let Some(key) = logs_key {
                    actions = actions.child(button(id("logs"), "Logs").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.log_key = Some(key.clone());
                            this.log_paused = None;
                            this.run(Command::ShowTab(Tab::Logs), cx);
                        },
                    )));
                }
                let mut meta = vec![db.status.label().to_string()];
                if let Some(engine) = &spec.engine {
                    meta.push(engine.clone());
                }
                meta.push(mask_password(&spec.url));
                row()
                    .child(status_dot(db.status))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(spec.name.clone()),
                            )
                            .child(muted_line(meta.join(" · ")))
                            .children(last.map(|(action, code, at)| {
                                let ok = code == Some(0);
                                div()
                                    .text_xs()
                                    .text_color(if ok {
                                        theme::text_muted()
                                    } else {
                                        theme::danger()
                                    })
                                    .child(format!(
                                        "{action} {} · {} ago",
                                        if ok {
                                            "✓".to_string()
                                        } else {
                                            format!(
                                                "✗ exit {}",
                                                code.map_or("signal".into(), |c| c.to_string())
                                            )
                                        },
                                        crate::activity::short_age(at.elapsed().as_secs() as i64)
                                    ))
                            })),
                    )
                    .child(actions)
                    .into_any_element()
            })
            .collect();
        section_or(
            "Databases",
            rows,
            &format!(
                "No databases: add a [[database]] to {} (url, and start/stop/migrate/seed/studio commands)",
                crate::services::FILE_NAME
            ),
        )
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
        let is_builtin = |key: &str| BUILTIN_LOGS.iter().any(|(k, _)| *k == key);
        // Never the built-in server's log by default: this repo's first.
        let current = self
            .log_key
            .clone()
            .filter(|k| keys.contains(k))
            .or_else(|| keys.iter().find(|k| !is_builtin(k)).cloned());

        let chip = |i: usize, key: &String, cx: &mut Context<Self>| {
            let builtin = BUILTIN_LOGS.iter().find(|(k, _)| k == key).map(|(_, f)| *f);
            let name = match builtin {
                Some(crate::backend::LogFile::Server) => "built-in server".to_string(),
                Some(crate::backend::LogFile::Postgres) => "built-in postgres".to_string(),
                None => log_label(key.strip_prefix(&prefix).unwrap_or(key)),
            };
            let running = builtin.is_none() && self.supervisor.is_running(key);
            let active = current.as_ref() == Some(key);
            let key = key.clone();
            button(("log-chip", i), name)
                .flex()
                .items_center()
                .gap_1p5()
                .when(running, |b| {
                    b.child(
                        div()
                            .size(px(6.))
                            .rounded_full()
                            .bg(theme::status(Status::Running)),
                    )
                })
                .when(active, |b| {
                    b.border_color(theme::accent()).bg(theme::surface_hover())
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.log_key = Some(key.clone());
                    this.log_paused = None;
                    this.log_follow = true;
                    cx.notify();
                }))
                .into_any_element()
        };
        let mut chips: Vec<AnyElement> = Vec::new();
        for (i, key) in keys.iter().enumerate().filter(|(_, k)| !is_builtin(k)) {
            chips.push(chip(i, key, cx));
        }
        if keys.iter().any(|k| is_builtin(k)) {
            chips.push(div().px_2().child(muted("·")).into_any_element());
            for (i, key) in keys.iter().enumerate().filter(|(_, k)| is_builtin(k)) {
                chips.push(chip(i, key, cx));
            }
        }

        let lines = match (&self.log_paused, &current) {
            (Some(frozen), _) => frozen.clone(),
            (None, Some(key)) => self.log_lines(key),
            (None, None) => Vec::new(),
        };
        let filter = self.log_filter.text.to_lowercase();
        let all = lines.len();
        let shown: Vec<String> = lines
            .into_iter()
            .filter(|line| filter.is_empty() || line.to_lowercase().contains(&filter))
            .collect();
        let start = shown.len().saturating_sub(LOG_RENDER_LINES);
        let total = shown.len();
        let current_builtin = current.as_deref().is_some_and(is_builtin);
        // What the shown log belongs to, for Restart / Stop / Attach.
        let process = current.as_ref().and_then(|key| {
            repo.snapshot
                .as_ref()?
                .processes
                .iter()
                .position(|p| &p.key == key)
        });
        let repo_index = self
            .repos
            .iter()
            .position(|r| r.entry.path == repo.entry.path)
            .unwrap_or(self.selected);
        let running = current
            .as_deref()
            .is_some_and(|k| !current_builtin && self.supervisor.is_running(k));
        let session = current
            .as_deref()
            .and_then(|k| self.supervisor.tmux_session(k));
        let file =
            current
                .as_deref()
                .and_then(|k| match BUILTIN_LOGS.iter().find(|(b, _)| *b == k) {
                    Some((_, f)) => Some(f.path()),
                    None => self.supervisor.log_file(k),
                });
        let copy_text = shown[start..].join("\n");

        let paused = self.log_paused.is_some();
        let pause_key = current.clone();
        let clear_key = current.clone();
        let toolbar =
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(240.))
                        .child(self.field(Field::LogFilter, "Filter…", window, cx)),
                )
                .child(
                    button("log-pause", if paused { "Resume" } else { "Pause" }).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.log_paused = match (&this.log_paused, &pause_key) {
                                (Some(_), _) | (None, None) => None,
                                (None, Some(key)) => Some(this.log_lines(key)),
                            };
                            cx.notify();
                        }),
                    ),
                )
                .when(!current_builtin, |d| {
                    d.child(button("log-clear", "Clear").on_click(cx.listener(
                        move |this, _, _, cx| {
                            if let Some(key) = &clear_key {
                                if this.supervisor.tmux_session(key).is_some() {
                                    this.run(Command::ClearRunLog(key.clone()), cx);
                                } else {
                                    this.supervisor.clear_logs(key);
                                }
                            }
                            this.log_paused = None;
                            cx.notify();
                        },
                    )))
                })
                .when(total > 0, |d| {
                    d.child(button("log-copy", "Copy").on_click(
                        cx.listener(move |this, _, _, cx| this.copy(copy_text.clone(), cx)),
                    ))
                })
                .children(file.map(|path| {
                    button("log-file", "Open log file").on_click(cx.listener(move |_, _, _, cx| {
                        cx.open_url(&format!(
                            "file://{}",
                            path.display().to_string().replace(' ', "%20")
                        ))
                    }))
                }))
                // The process this log belongs to, from here.
                .when_some(process.filter(|_| running), |d, p| {
                    d.child(button("log-restart", "Restart").on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.log_follow = true;
                            this.run(Command::RestartProcess(repo_index, p), cx)
                        },
                    )))
                })
                .when(running, |d| {
                    let key = current.clone().unwrap_or_default();
                    d.child(button("log-stop", "Stop").on_click(cx.listener(
                        move |this, _, _, cx| match process {
                            Some(p) => this.run(Command::StopProcess(repo_index, p), cx),
                            None => this.run(Command::StopKey(key.clone()), cx),
                        },
                    )))
                })
                .children(session.map(|session| {
                    button("log-attach", "Attach").on_click(cx.listener(move |this, _, _, cx| {
                        this.run(Command::AttachRun(session.clone()), cx)
                    }))
                }))
                .child(div().flex_1())
                .when(!self.log_follow && total > 0, |d| {
                    d.child(button("log-latest", "↓ Latest").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.log_follow = true;
                            this.log_scroll.scroll_to_bottom();
                            cx.notify();
                        },
                    )))
                })
                .child(muted(match (filter.is_empty(), total > LOG_RENDER_LINES) {
                    (false, _) => format!("{total} of {all} lines match"),
                    (true, true) => format!("last {LOG_RENDER_LINES} of {total} lines"),
                    (true, false) => format!("{total} lines"),
                }));

        let body = if current.is_none() {
            muted("Nothing logged in this repo yet: logs appear for processes, scripts and database tasks started from the app.")
                .into_any_element()
        } else if total == 0 && !filter.is_empty() {
            muted(format!("No line matches “{}”", self.log_filter.text.trim())).into_any_element()
        } else if total == 0 {
            muted("No output yet.").into_any_element()
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
                // Fills the window below the header and toolbar.
                .h((window.viewport_size().height - px(330.)).max(px(240.)))
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

/// One tab in a tab strip: a colored dot, the label, a count badge, and the
/// key on the active tab.
pub(super) fn tab_item(
    id: &'static str,
    label: &'static str,
    color: gpui::Hsla,
    underline: gpui::Hsla,
    active: bool,
    count: usize,
    shortcut: String,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
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
        .child(
            div()
                .size(px(6.))
                .rounded_full()
                .bg(color.opacity(if active { 1.0 } else { 0.5 })),
        )
        .child(label)
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
        .when(active && !shortcut.is_empty(), |d| {
            d.child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(shortcut),
            )
        })
}

/// A row of tabs that scrolls sideways when it does not fit: with the wheel
/// or trackpad in either direction, or the ‹ › buttons at its ends.
pub(super) fn tab_strip(
    id: &'static str,
    scroll: &gpui::ScrollHandle,
    items: Vec<gpui::Stateful<gpui::Div>>,
    cx: &mut Context<Workspace>,
) -> gpui::Div {
    let offset = scroll.offset().x;
    let max = scroll.max_offset().width;
    let can_left = offset < px(0.);
    let can_right = max > px(0.) && -offset < max - px(1.);
    let nudge = |id: &'static str,
                 label: &'static str,
                 by: f32,
                 enabled: bool,
                 cx: &mut Context<Workspace>| {
        let scroll = scroll.clone();
        div()
            .id(id)
            .flex_none()
            .px_2()
            .py_2()
            .text_sm()
            .text_color(if enabled {
                theme::text()
            } else {
                theme::text_muted().opacity(0.4)
            })
            .when(enabled, |d| {
                d.cursor_pointer().hover(|d| d.bg(theme::surface_hover()))
            })
            .on_click(cx.listener(move |_, _, _, cx| {
                let mut point = scroll.offset();
                let max = scroll.max_offset().width;
                point.x = (point.x + px(by)).min(px(0.)).max(-max);
                scroll.set_offset(point);
                cx.notify();
            }))
            .child(label)
    };
    let wheel = scroll.clone();
    div()
        .flex()
        .items_center()
        .w_full()
        .min_w_0()
        .border_b_1()
        .border_color(theme::border())
        .px_3()
        .when(max > px(0.), |d| {
            d.child(nudge("tabs-left", "‹", 240., can_left, cx))
        })
        .child(
            div()
                .id(id)
                .flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .overflow_x_scroll()
                .track_scroll(scroll)
                // A vertical swipe or a plain wheel scrolls the strip too:
                // GPUI only does that when the sideways delta is exactly zero.
                .on_scroll_wheel(cx.listener(
                    move |_, event: &gpui::ScrollWheelEvent, window, cx| {
                        let delta = event.delta.pixel_delta(window.line_height());
                        if delta.y.abs() > delta.x.abs() && delta.x != px(0.) {
                            let mut point = wheel.offset();
                            let max = wheel.max_offset().width;
                            point.x = (point.x + delta.y).min(px(0.)).max(-max);
                            wheel.set_offset(point);
                            cx.notify();
                        }
                    },
                ))
                .children(items),
        )
        .when(max > px(0.), |d| {
            d.child(nudge("tabs-right", "›", -240., can_right, cx))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_are_masked_and_log_keys_read_well() {
        // Given
        let urls = [
            "postgresql://me:secret@db:5432/app",
            "postgresql://localhost/app",
        ];

        // When
        let masked: Vec<String> = urls.iter().map(|u| mask_password(u)).collect();
        let labels = [
            log_label("script:just:test"),
            log_label("postgres:migrate"),
            log_label("api"),
        ];

        // Then
        assert_eq!(
            masked,
            [
                "postgresql://me:•••@db:5432/app",
                "postgresql://localhost/app"
            ]
        );
        assert_eq!(labels, ["test (just)", "postgres › migrate", "api"]);
    }
}
