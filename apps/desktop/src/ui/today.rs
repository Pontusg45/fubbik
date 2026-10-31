//! Today: one screen across every repo for what needs you, what is in
//! flight, and what to start next. Each line acts in one click.

use gpui::{AnyElement, FontWeight, Hsla, SharedString};

use super::plans::open_dependencies;
use super::widgets::{button, muted, pill, row, section};
use super::*;
use crate::activity;
use crate::model::Status;
use crate::sessions::SessionState;

/// Pending tasks shown per plan under "Next up".
const NEXT_PER_PLAN: usize = 2;

impl Workspace {
    pub(super) fn render_today(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut needs: Vec<AnyElement> = Vec::new();
        let mut flight: Vec<AnyElement> = Vec::new();
        let mut next: Vec<AnyElement> = Vec::new();
        let now = activity::now();

        for (repo, view) in self.repos.iter().enumerate() {
            let name = view.entry.name();
            let id = |kind: &str, n: usize| SharedString::from(format!("today-{kind}-{repo}-{n}"));
            let open = |label: &'static str,
                        key: SharedString,
                        tab: Tab,
                        cx: &mut Context<Self>| {
                button(key, label).on_click(
                    cx.listener(move |this, _, _, cx| this.run(Command::Focus { repo, tab }, cx)),
                )
            };
            let line = |color: Hsla, tag: &str, text: String| {
                row()
                    .child(div().w(px(96.)).child(pill(tag.to_string(), color)))
                    .child(div().flex_1().overflow_hidden().text_sm().child(text))
            };

            if let Some(snapshot) = &view.snapshot {
                if snapshot.missing || snapshot.services_error.is_some() {
                    let what = if snapshot.missing {
                        "folder is missing"
                    } else {
                        "services file has an error"
                    };
                    needs.push(
                        line(theme::danger(), "repo", format!("{name}: {what}"))
                            .child(open("Open", id("repo", 0), Tab::Overview, cx))
                            .into_any_element(),
                    );
                }
                for (n, session) in snapshot
                    .sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Idle)
                    .enumerate()
                {
                    let tag = format!("{} waits", session.agent.label());
                    let mut r = line(theme::accent(), &tag, format!("{name}: {}", session.title));
                    if let Some(pane) = session.pane.clone() {
                        let cwd = session.cwd.clone();
                        r = r.child(button(id("agent", n), "Open").on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.open_pane(pane.clone(), cwd.clone(), cx);
                                cx.notify();
                            },
                        )));
                    } else {
                        r = r.child(open("Sessions", id("agent-s", n), Tab::Sessions, cx));
                    }
                    needs.push(r.into_any_element());
                }
                for (n, process) in snapshot
                    .processes
                    .iter()
                    .filter(|p| matches!(p.status, Status::Degraded | Status::Error))
                    .enumerate()
                {
                    needs.push(
                        line(
                            theme::status(process.status),
                            process.status.label(),
                            format!("{name}: {}", process.spec.name),
                        )
                        .child(open("Processes", id("proc", n), Tab::Processes, cx))
                        .into_any_element(),
                    );
                }
                for (n, overlap) in snapshot.overlaps.iter().enumerate() {
                    needs.push(
                        line(
                            theme::status(Status::Degraded),
                            "overlap",
                            format!(
                                "{name}: {} and {} both change {} file(s)",
                                overlap.a,
                                overlap.b,
                                overlap.files.len()
                            ),
                        )
                        .child(open("Worktrees", id("overlap", n), Tab::Worktrees, cx))
                        .into_any_element(),
                    );
                }
            }

            let Some(Ok(knowledge)) = &view.knowledge else {
                continue;
            };
            let failing = knowledge.requirements.failing + knowledge.violated_cells();
            if failing > 0 {
                needs.push(
                    line(
                        theme::danger(),
                        "failing",
                        format!("{name}: {failing} failing requirement(s) or behaviour cell(s)"),
                    )
                    .child(open("Knowledge", id("failing", 0), Tab::Knowledge, cx))
                    .into_any_element(),
                );
            }
            if knowledge.stale > 0 {
                needs.push(
                    line(
                        theme::status(Status::Degraded),
                        "stale",
                        format!("{name}: {} stale knowledge flag(s)", knowledge.stale),
                    )
                    .child(open("Knowledge", id("stale", 0), Tab::Knowledge, cx))
                    .into_any_element(),
                );
            }

            for plan in &knowledge.plan_details {
                for (n, task) in plan
                    .tasks
                    .iter()
                    .filter(|t| t.status == "in_progress")
                    .enumerate()
                {
                    let effort = self.task_effort(repo, &plan.id, task);
                    let holder = self
                        .task_holder(repo, &plan.id, &task.id)
                        .map(|(run, _)| run.handle);
                    let mut detail = vec![format!("{name} · {}", plan.title)];
                    if let Some(h) = holder {
                        detail.push(h);
                    }
                    if let Some(since) = effort.since {
                        detail.push(format!("{} in progress", activity::short_age(now - since)));
                    }
                    if effort.tokens > 0 {
                        detail.push(format!("{}k tokens", effort.tokens / 1000));
                    }
                    let (p, t) = (plan.id.clone(), task.id.clone());
                    flight.push(
                        row()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .overflow_hidden()
                                    .child(div().text_sm().child(task.title.clone()))
                                    .child(muted(detail.join("  ·  "))),
                            )
                            .child(
                                button(id(&format!("tl-{}", plan.id), n), "Timeline").on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.run(
                                            Command::TaskTimeline {
                                                repo,
                                                plan: p.clone(),
                                                task: t.clone(),
                                            },
                                            cx,
                                        )
                                    }),
                                ),
                            )
                            .child(open(
                                "Board",
                                id(&format!("board-{}", plan.id), n),
                                Tab::Plans,
                                cx,
                            ))
                            .into_any_element(),
                    );
                }
                let startable = plan
                    .tasks
                    .iter()
                    .filter(|t| {
                        t.status == "pending"
                            && open_dependencies(plan, t).is_empty()
                            && self.task_holder(repo, &plan.id, &t.id).is_none()
                    })
                    .take(NEXT_PER_PLAN);
                for (n, task) in startable.enumerate() {
                    let (p, t) = (plan.id.clone(), task.id.clone());
                    let (bp, bt) = (plan.id.clone(), task.id.clone());
                    next.push(
                        row()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .overflow_hidden()
                                    .child(div().text_sm().child(task.title.clone()))
                                    .child(muted(format!("{name} · {}", plan.title))),
                            )
                            .child(
                                button(id(&format!("start-{}", plan.id), n), "Start").on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.run(
                                            Command::StartTask {
                                                repo,
                                                plan: p.clone(),
                                                task: t.clone(),
                                                new_worktree: true,
                                                background: false,
                                            },
                                            cx,
                                        )
                                    }),
                                ),
                            )
                            .child(
                                button(id(&format!("bg-{}", plan.id), n), "In background")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.run(
                                            Command::StartTask {
                                                repo,
                                                plan: bp.clone(),
                                                task: bt.clone(),
                                                new_worktree: true,
                                                background: true,
                                            },
                                            cx,
                                        )
                                    })),
                            )
                            .into_any_element(),
                    );
                }
            }
        }

        let queued = self.launch_queue.len();
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Today"),
                    )
                    .child(muted(format!(
                        "{} repos · {} live agent(s){}",
                        self.repos.len(),
                        self.live_agents(),
                        if queued > 0 {
                            format!(" · {queued} queued")
                        } else {
                            String::new()
                        }
                    ))),
            )
            .child(section(format!("Needs you ({})", needs.len()), needs))
            .child(section(format!("In flight ({})", flight.len()), flight))
            .child(section("Next up", next))
            .into_any_element()
    }

    /// Live agent sessions across all repos.
    pub(super) fn live_agents(&self) -> usize {
        self.repos
            .iter()
            .filter_map(|r| r.snapshot.as_ref())
            .map(|s| s.sessions.iter().filter(|s| s.state.is_live()).count())
            .sum()
    }
}
