//! The Worktrees and AI Sessions tabs.

use std::time::SystemTime;

use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::palette::Command;
use super::widgets::{button, label, loading, muted, muted_line, pill, row, section, section_or};
use super::{BranchContext, Field, Loadable, Tab, Workspace, display_path};
use crate::model::{RepoSnapshot, Status};
use crate::sessions::{AiSession, SessionState, ago};
use crate::theme;
use crate::worktree::Worktree;

/// Waiting for you stands out; working is the accent; the rest is quiet.
pub fn session_color(state: SessionState) -> Hsla {
    match state {
        SessionState::Busy => theme::accent(),
        SessionState::Idle => theme::status(Status::Degraded),
        SessionState::Active => theme::text_muted(),
        SessionState::Ended => theme::status(Status::Stopped),
    }
}

/// Waiting first, then working, then recently active.
fn state_rank(state: SessionState) -> u8 {
    match state {
        SessionState::Idle => 0,
        SessionState::Busy => 1,
        SessionState::Active => 2,
        SessionState::Ended => 3,
    }
}

/// Ended sessions shown before "Show all".
const RECENT_SHOWN: usize = 10;
/// Row ids on All agents start here (the Sessions page uses 0…).
const AGENTS_ROW_BASE: usize = 1000;

fn sessions_in<'a>(snapshot: &'a RepoSnapshot, worktree: &Worktree) -> Vec<&'a AiSession> {
    // A session belongs to the most specific worktree containing its cwd.
    let roots = snapshot.worktree_paths();
    snapshot
        .sessions
        .iter()
        .filter(|s| {
            let owner = roots
                .iter()
                .filter(|root| s.cwd.starts_with(root))
                .max_by_key(|root| root.components().count());
            owner == Some(&worktree.path)
        })
        .collect()
}

impl Workspace {
    pub(super) fn render_worktrees(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .child(label("NEW WORKTREE"))
            .child(div().w(px(260.)).child(self.field(
                Field::NewWorktree,
                "branch name — enter creates",
                window,
                cx,
            )))
            .child(button("worktree-create", "Create").on_click(
                cx.listener(move |this, _, _, cx| this.run(Command::CreateWorktree(index), cx)),
            ))
            .child(button("worktree-prune", "Prune stale").on_click(
                cx.listener(move |this, _, _, cx| this.run(Command::PruneWorktrees(index), cx)),
            ))
            .child(muted("New worktrees go next to the repo: <repo>-<branch>"));

        let body: AnyElement = match &snapshot.worktrees {
            None => muted("Reading worktrees…").into_any_element(),
            Some(Err(error)) => muted(format!("Not a git repo: {error}")).into_any_element(),
            Some(Ok(worktrees)) => {
                let rows = worktrees
                    .iter()
                    .enumerate()
                    .map(|(w, worktree)| self.worktree_row(index, w, worktree, snapshot, cx))
                    .collect();
                section(format!("Worktrees ({})", worktrees.len()), rows).into_any_element()
            }
        };
        div().flex().flex_col().gap_4().child(toolbar).child(body)
    }

    fn worktree_row(
        &self,
        repo: usize,
        w: usize,
        worktree: &Worktree,
        snapshot: &RepoSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = |name: &str| SharedString::from(format!("wt-{name}-{repo}-{w}"));
        let sessions = sessions_in(snapshot, worktree);
        let live = sessions.iter().filter(|s| s.state.is_live()).count();

        let mut badges = div().flex().flex_wrap().gap_1();
        if worktree.main {
            badges = badges.child(pill("main", theme::accent()));
        }
        if worktree.missing {
            badges = badges.child(pill("missing", theme::danger()));
        }
        if worktree.merged() {
            badges = badges.child(pill("merged", theme::status(Status::Running)));
        }
        if worktree.locked.is_some() {
            badges = badges.child(pill("locked", theme::status(Status::Degraded)));
        }
        if worktree.prunable.is_some() {
            badges = badges.child(pill("prunable", theme::status(Status::Degraded)));
        }
        if live > 0 {
            badges = badges.child(pill(format!("{live} AI live"), theme::accent()));
        }
        let name = worktree.name();
        for overlap in snapshot
            .overlaps
            .iter()
            .filter(|o| o.a == name || o.b == name)
        {
            let other = if overlap.a == name {
                &overlap.b
            } else {
                &overlap.a
            };
            badges = badges.child(pill(
                format!("overlaps {other}: {} file(s)", overlap.files.len()),
                theme::status(Status::Degraded),
            ));
        }

        let detail = if worktree.missing {
            "folder is gone — Prune removes git's record of it".to_string()
        } else {
            let mut parts = vec![format!("{} changed", worktree.changed_files)];
            if !worktree.main {
                parts.push(format!(
                    "{} ahead · {} behind main",
                    worktree.ahead, worktree.behind
                ));
            }
            parts.push(format!("{} AI sessions", sessions.len()));
            parts.join("  ·  ")
        };

        let path = worktree.path.clone();
        let mut actions = div()
            .flex()
            .flex_wrap()
            .justify_end()
            .gap_1()
            .max_w(px(460.));
        if worktree.missing {
            actions = actions.child(button(id("prune"), "Prune").on_click(
                cx.listener(move |this, _, _, cx| this.run(Command::PruneWorktrees(repo), cx)),
            ));
        }
        if !worktree.missing {
            let (editor, terminal, claude, context) =
                (path.clone(), path.clone(), path.clone(), path.clone());
            let expanded = self.expanded_worktree.as_ref() == Some(&path);
            actions = actions
                .child(
                    button(
                        id("context"),
                        if expanded { "Hide context" } else { "Context" },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.load_branch_context(repo, context.clone(), cx);
                        cx.notify();
                    })),
                )
                .child(button(id("editor"), "Editor").on_click(cx.listener(
                    move |this, _, _, cx| this.run(Command::OpenEditorAt(editor.clone()), cx),
                )))
                .child(button(id("terminal"), "Terminal.app").on_click(cx.listener(
                    move |this, _, _, cx| this.run(Command::OpenTerminalAt(terminal.clone()), cx),
                )))
                .child(button(id("shell"), "Shell tab").on_click(cx.listener({
                    let path = path.clone();
                    move |this, _, _, cx| this.run(Command::ShellAt(path.clone()), cx)
                })))
                .child(button(id("claude"), "New Claude").on_click(cx.listener(
                    move |this, _, _, cx| this.run(Command::NewSession(claude.clone()), cx),
                )));
        }
        if !worktree.main && !worktree.missing {
            let review = path.clone();
            actions = actions.child(button(id("review"), "Review").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.run(
                        Command::ReviewWorktree {
                            repo,
                            path: review.clone(),
                        },
                        cx,
                    );
                    cx.notify();
                },
            )));
        }
        if !worktree.main && !worktree.missing {
            actions = actions.child(
                button(id("remove"), "Remove")
                    .text_color(theme::danger())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run(Command::RemoveWorktree(repo, path.clone()), cx)
                    })),
            );
        }

        let tasks: Vec<String> = sessions
            .iter()
            .filter_map(|s| self.task_of_session(repo, &s.id))
            .collect();
        let detail = if tasks.is_empty() {
            detail
        } else {
            format!("{detail}  ·  task: {}", tasks.join(", "))
        };
        let context = (self.expanded_worktree.as_ref() == Some(&worktree.path))
            .then(|| self.repos[repo].branch_context.get(&worktree.path))
            .flatten()
            .map(|loaded| self.render_branch_context(loaded, cx));
        let md_stale = self.repos[repo]
            .setup
            .as_ref()
            .and_then(|s| s.claude_md.as_ref())
            .and_then(|sync| {
                sync.worktrees
                    .iter()
                    .find(|(root, _)| *root == worktree.path)
            })
            .is_some_and(|(_, state)| *state != crate::claude_md::MdState::InSync);
        let badges = badges.when(md_stale, |b| {
            b.child(pill("CLAUDE.md stale", theme::status(Status::Degraded)))
        });

        let line = row()
            .when(
                self.tab == Tab::Worktrees && self.list_cursor == Some(w),
                |r| r.bg(theme::surface_hover()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_0p5()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(worktree.name()),
                            )
                            .child(muted(worktree.head.clone()))
                            .child(badges),
                    )
                    .child(muted(display_path(&worktree.path)))
                    .child(muted(detail)),
            )
            .child(actions);
        div()
            .flex()
            .flex_col()
            .child(line)
            .children(context)
            .into_any_element()
    }

    /// The plan task a Claude session holds on a coordination board.
    pub(super) fn task_of_session(&self, repo: usize, session_id: &str) -> Option<String> {
        let knowledge = self.repos[repo].knowledge.as_ref()?.as_ref().ok()?;
        let (board, run) = knowledge.run_for_session(session_id)?;
        let titles: Vec<&str> = board
            .tasks_of(&run.id)
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        (!titles.is_empty()).then(|| format!("“{}” ({})", titles.join("”, “"), board.plan_title))
    }

    fn render_branch_context(
        &self,
        loaded: &Loadable<BranchContext>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = match loaded {
            Loadable::Loading => muted("Looking up what this branch touches…").into_any_element(),
            Loadable::Done(Err(error)) => {
                muted(format!("fubbik lookup failed: {error}")).into_any_element()
            }
            Loadable::Done(Ok(context)) => {
                let mut column = div().flex().flex_col().gap_3().child(muted(format!(
                    "{} files changed against main{}",
                    context.files.len(),
                    if context.files.len() > 40 {
                        " (first 40 checked per file)"
                    } else {
                        ""
                    }
                )));
                let chunk_link = |key: String,
                                  title: String,
                                  extra: String,
                                  cx: &mut Context<Self>,
                                  chunk_id: String| {
                    div()
                        .id(SharedString::from(key))
                        .flex()
                        .gap_2()
                        .cursor_pointer()
                        .hover(|d| d.bg(theme::surface_hover()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run(Command::OpenWeb(format!("/chunks/{chunk_id}")), cx)
                        }))
                        .child(div().text_sm().child(title))
                        .child(muted(extra))
                };
                if !context.linked.is_empty() {
                    let mut list = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(label("CONVENTIONS LINKED TO CHANGED FILES (check-files)"));
                    for (i, (chunk, files)) in context.linked.iter().enumerate() {
                        let reason = chunk.match_reason.clone().unwrap_or_default();
                        list = list.child(chunk_link(
                            format!("linked-{i}"),
                            chunk.title.clone(),
                            format!("{reason} · {}", files.join(", ")),
                            cx,
                            chunk.id.clone(),
                        ));
                    }
                    column = column.child(list);
                }
                for (s, (title, chunks)) in context.sections.iter().enumerate() {
                    let mut list = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(label(title.to_uppercase()));
                    for (i, chunk) in chunks.iter().enumerate() {
                        let extra = if chunk.stale {
                            "⚠ stale".to_string()
                        } else {
                            chunk.kind.clone()
                        };
                        list = list.child(chunk_link(
                            format!("ctx-{s}-{i}"),
                            chunk.title.clone(),
                            extra,
                            cx,
                            chunk.id.clone(),
                        ));
                    }
                    column = column.child(list);
                }
                if context.linked.is_empty() && context.sections.is_empty() {
                    column = column.child(muted("No chunks relate to these changes."));
                }
                column.into_any_element()
            }
        };
        div()
            .mx_4()
            .mb_3()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::input())
            .child(body)
            .into_any_element()
    }

    pub(super) fn render_sessions(
        &self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let now = SystemTime::now();
        let repo_path = self.repos[index].entry.path.clone();
        let order = self.session_order(index);
        let live_count = order.iter().filter(|s| s.state.is_live()).count();
        let ended_total = snapshot
            .sessions
            .iter()
            .filter(|s| !s.state.is_live())
            .count();
        let mut live_rows = Vec::new();
        let mut ended_rows = Vec::new();
        for (i, session) in order.iter().enumerate() {
            let row = self.session_row(index, i, session, snapshot, now, window, cx);
            if session.state.is_live() {
                live_rows.push(row)
            } else {
                ended_rows.push(row)
            }
        }
        let waiting = order
            .iter()
            .any(|s| s.state == SessionState::Idle && s.pane.is_some());
        let opens_in = if self.config.external_terminal() {
            "Terminal.app"
        } else {
            "the Terminal tab"
        };

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(button("session-new", "New Claude session").on_click(cx.listener(move |this, _, _, cx| {
                        this.run(Command::NewSession(repo_path.clone()), cx)
                    })))
                    .when(waiting, |d| {
                        d.child(
                            button(
                                "session-reply-longest",
                                format!(
                                    "Reply to the longest-waiting ({})",
                                    super::keymap::keys_for("reply_latest").map(|k| super::keymap::pretty(&k)).unwrap_or_default()
                                ),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reply_latest(window, cx);
                                cx.notify();
                            })),
                        )
                    })
                    .child(muted(format!("Claude Code and Codex sessions in this repo and its worktrees; Resume opens {opens_in}."))),
            )
            .child(section_or(format!("Live ({live_count})"), live_rows, "No live agents"))
            .child(section_or(format!("Recent ({ended_total})"), ended_rows, "No past sessions"))
            .when(ended_total > RECENT_SHOWN, |d| {
                d.child(
                    div()
                        .id("sessions-all")
                        .text_xs()
                        .text_color(theme::accent())
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sessions_all = !this.sessions_all;
                            cx.notify();
                        }))
                        .child(if self.sessions_all { "Show fewer".to_string() } else { format!("Show all {ended_total}") }),
                )
            })
    }

    /// AI Sessions in display order: waiting first (longest-waiting on
    /// top), then working, then recently active; then the ended ones,
    /// newest first (the last `RECENT_SHOWN` unless "Show all").
    pub(super) fn session_order(&self, repo: usize) -> Vec<AiSession> {
        let sessions = self
            .repos
            .get(repo)
            .and_then(|r| r.snapshot.as_ref())
            .map(|s| s.sessions.clone())
            .unwrap_or_default();
        let (mut live, ended): (Vec<_>, Vec<_>) =
            sessions.into_iter().partition(|s| s.state.is_live());
        live.sort_by_key(|s| (state_rank(s.state), s.last_activity));
        let shown = if self.sessions_all {
            ended.len()
        } else {
            ended.len().min(RECENT_SHOWN)
        };
        live.into_iter()
            .chain(ended.into_iter().take(shown))
            .collect()
    }

    /// All agents in display order: repos with waiting agents first, then
    /// the busiest; per repo its live sessions (waiting first) and its last
    /// few ended ones. Repos with nothing live come last.
    pub(super) fn all_agents_order(&self) -> Vec<(usize, AiSession)> {
        let mut repos: Vec<usize> = (0..self.repos.len())
            .filter(|&r| self.repos[r].snapshot.is_some())
            .collect();
        let counts = |r: usize| {
            let s = self.repos[r]
                .snapshot
                .as_ref()
                .map(|s| s.sessions.as_slice())
                .unwrap_or_default();
            (
                s.iter().filter(|s| s.state == SessionState::Idle).count(),
                s.iter().filter(|s| s.state.is_live()).count(),
            )
        };
        repos.sort_by_key(|&r| {
            let (waiting, live) = counts(r);
            (std::cmp::Reverse(waiting), std::cmp::Reverse(live))
        });
        let mut rows = Vec::new();
        for r in repos {
            let sessions = self.repos[r]
                .snapshot
                .as_ref()
                .map(|s| s.sessions.clone())
                .unwrap_or_default();
            let (mut live, ended): (Vec<_>, Vec<_>) =
                sessions.into_iter().partition(|s| s.state.is_live());
            live.sort_by_key(|s| (state_rank(s.state), s.last_activity));
            for s in live.into_iter().chain(ended.into_iter().take(3)) {
                rows.push((r, s));
            }
        }
        rows
    }

    /// Every repo's agents, waiting ones first; j/k and ⏎ work here too.
    pub(super) fn render_all_agents(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let now = SystemTime::now();
        let checking = self.repos.iter().filter(|r| r.snapshot.is_none()).count();
        let order = self.all_agents_order();
        let mut column = div().flex().flex_col().gap_6().child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("All agents"),
                )
                .child(muted(format!(
                    "{} live across {} repos",
                    self.live_agents(),
                    self.repos.len()
                )))
                .when(checking > 0, |d| {
                    d.child(loading(format!("{checking} repo(s) still checking")))
                }),
        );
        let mut current: Option<usize> = None;
        let mut rows: Vec<AnyElement> = Vec::new();
        let flush = |column: gpui::Div,
                     repo: Option<usize>,
                     rows: Vec<AnyElement>,
                     cx: &mut Context<Self>|
         -> gpui::Div {
            let Some(repo) = repo else { return column };
            let view = &self.repos[repo];
            let live = view.snapshot.as_ref().map_or(0, |s| s.live_sessions());
            let path = view.entry.path.clone();
            let header = div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id(("agents-repo", repo))
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|d| d.underline())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run(
                                Command::Focus {
                                    repo,
                                    tab: Tab::Sessions,
                                },
                                cx,
                            )
                        }))
                        .child(view.entry.name()),
                )
                .child(muted(format!("{live} live")))
                .child(div().flex_1())
                .child(
                    button(("agents-new", repo), "+ Claude").on_click(cx.listener(
                        move |this, _, _, cx| this.run(Command::NewSession(path.clone()), cx),
                    )),
                );
            column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(header)
                    .child(section_or("", rows, "No sessions")),
            )
        };
        for (flat, (repo, session)) in order.iter().enumerate() {
            if current != Some(*repo) {
                column = flush(column, current, std::mem::take(&mut rows), cx);
                current = Some(*repo);
            }
            let snapshot = self.repos[*repo]
                .snapshot
                .as_ref()
                .expect("ordered repos have snapshots");
            rows.push(self.session_row(
                *repo,
                AGENTS_ROW_BASE + flat,
                session,
                snapshot,
                now,
                window,
                cx,
            ));
        }
        column = flush(column, current, rows, cx);
        if order.is_empty() && checking == 0 {
            column = column.child(muted("No AI sessions in any registered repo yet."));
        }
        column.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn session_row(
        &self,
        repo: usize,
        i: usize,
        session: &AiSession,
        snapshot: &RepoSnapshot,
        now: SystemTime,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = |name: &str| SharedString::from(format!("session-{name}-{repo}-{i}"));
        let worktree = snapshot
            .worktrees
            .as_ref()
            .and_then(|w| w.as_ref().ok())
            .and_then(|worktrees| {
                worktrees
                    .iter()
                    .filter(|w| session.cwd.starts_with(&w.path))
                    .max_by_key(|w| w.path.components().count())
            });
        let place = match worktree {
            Some(w) if w.main => "main worktree".to_string(),
            Some(w) => format!("worktree {}", w.name()),
            None => display_path(&session.cwd),
        };
        let mut detail = vec![place];
        if let Some(branch) = &session.branch {
            detail.push(format!("on {branch}"));
        }
        detail.push(ago(session.last_activity, now));
        if let Some(task) = self.task_of_session(repo, &session.id) {
            detail.push(format!("task {task}"));
        }
        if session.pane.is_some() {
            detail.push("in tmux".to_string());
        }

        let (agent, session_id, cwd) = (session.agent, session.id.clone(), session.cwd.clone());
        let copy_id = session.id.clone();
        let mut actions = div().flex().gap_1();
        // Resume is for ended sessions; a live one would get a second client.
        if !session.state.is_live() {
            actions = actions.child(button(id("resume"), "Resume").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.run(
                        Command::ResumeSession(agent, session_id.clone(), cwd.clone()),
                        cx,
                    )
                },
            )));
        }
        if !session.state.is_live() {
            let saved = session.clone();
            actions = actions.child(button(id("note"), "Save as note").on_click(
                cx.listener(move |this, _, _, cx| this.save_session_note(repo, saved.clone(), cx)),
            ));
        }
        if let Some(pane) = session.pane.clone() {
            let (reply_pane, open_pane, open_cwd) =
                (pane.clone(), pane.clone(), session.cwd.clone());
            let replying = self
                .reply_target
                .as_ref()
                .is_some_and(|t| t.id == pane.id && t.server == pane.server);
            actions = actions
                .child(
                    button(id("reply"), if replying { "Cancel" } else { "Reply" }).on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.toggle_reply(reply_pane.clone(), window);
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    button(id("open"), "Open").on_click(cx.listener(move |this, _, _, cx| {
                        this.open_pane(open_pane.clone(), open_cwd.clone(), cx);
                        cx.notify();
                    })),
                );
        }
        actions = actions.child(
            button(id("copy"), "Copy ID")
                .on_click(cx.listener(move |this, _, _, cx| this.copy(copy_id.clone(), cx))),
        );

        let highlighted = if i >= AGENTS_ROW_BASE {
            self.tab == Tab::Agents && self.list_cursor == Some(i - AGENTS_ROW_BASE)
        } else {
            self.tab == Tab::Sessions && repo == self.selected && self.list_cursor == Some(i)
        };
        let row = row()
            .when(highlighted, |r| r.bg(theme::surface_hover()))
            .child(
                div()
                    .size_2()
                    .flex_none()
                    .rounded_full()
                    .bg(session_color(session.state)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(pill(session.agent.label(), theme::agent(session.agent)))
                            .child(
                                div()
                                    .min_w(px(0.))
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(session.title.clone()),
                            ),
                    )
                    .child(muted_line(detail.join("  ·  "))),
            )
            .child(pill(session.state.label(), session_color(session.state)))
            .child(actions);
        let preview = session
            .preview
            .clone()
            .filter(|p| !p.is_empty())
            .map(|text| {
                div()
                    .mx_4()
                    .mb_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme::input())
                    .font_family("Menlo")
                    .text_xs()
                    .text_color(theme::text_muted())
                    .overflow_hidden()
                    .children(
                        text.lines()
                            .map(|line| div().truncate().child(line.to_string())),
                    )
            });
        let reply = session
            .pane
            .as_ref()
            .filter(|p| {
                self.reply_target
                    .as_ref()
                    .is_some_and(|t| t.id == p.id && t.server == p.server)
            })
            .map(|_| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mx_4()
                    .mb_3()
                    .child(div().flex_1().child(self.field(
                        Field::Reply,
                        "Type to the agent — enter sends",
                        window,
                        cx,
                    )))
                    .child(
                        button(id("send"), "Send")
                            .on_click(cx.listener(|this, _, _, cx| this.send_reply(cx))),
                    )
            });
        div()
            .flex()
            .flex_col()
            .child(row)
            .children(preview)
            .children(reply)
            .into_any_element()
    }
}
