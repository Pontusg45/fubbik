//! The Plans tab as a board: one plan at a time, its tasks in columns by
//! status. Cards show dependencies, acceptance criteria (a checklist you can
//! tick), who holds the task, and what it has cost; they move between
//! columns, start an agent, and open their timeline.

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, SharedString, div, prelude::*, px};

use super::Workspace;
use super::palette::Command;
use super::widgets::{button, muted, pill, progress};
use crate::activity;
use crate::fubbik::{PlanDetail, Task};
use crate::model::Status;
use crate::theme;

pub fn task_color(status: &str) -> Hsla {
    match status {
        "done" => theme::status(Status::Running),
        "in_progress" => theme::accent(),
        "blocked" => theme::status(Status::Degraded),
        "skipped" => theme::status(Status::Stopped),
        _ => theme::text_muted(),
    }
}

/// Columns, left to right, and the statuses each one shows.
const COLUMNS: [(&str, &[&str]); 4] = [
    ("Pending", &["pending"]),
    ("In progress", &["in_progress"]),
    ("Blocked", &["blocked"]),
    ("Done", &["done", "skipped"]),
];

/// Where ◀ / ▶ move a task from each status.
fn neighbours(status: &str) -> (Option<&'static str>, Option<&'static str>) {
    match status {
        "pending" => (None, Some("in_progress")),
        "in_progress" => (Some("pending"), Some("done")),
        "blocked" => (Some("pending"), Some("in_progress")),
        "done" | "skipped" => (Some("in_progress"), None),
        _ => (None, None),
    }
}

/// Dependencies of `task` that are not done yet.
pub fn open_dependencies<'a>(plan: &'a PlanDetail, task: &Task) -> Vec<&'a Task> {
    task.depends_on
        .iter()
        .filter_map(|id| plan.tasks.iter().find(|t| t.id == *id))
        .filter(|t| !matches!(t.status.as_str(), "done" | "skipped"))
        .collect()
}

impl Workspace {
    pub(super) fn render_plans(&self, repo: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let knowledge = match self.knowledge_or_notice(repo) {
            Ok(k) => k,
            Err(notice) => return notice,
        };
        if knowledge.plan_details.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_sm().child("No open plans in this space"))
                .child(muted("Create one with `fubbik plan create`, the MCP `create_plan` tool, or the web app."))
                .child(div().flex().child(button("plans-web", "Open plans in fubbik").on_click(cx.listener(
                    |this, _, _, cx| this.run(Command::OpenWeb("/plans".into()), cx),
                ))))
                .into_any_element();
        }
        // The chosen plan, else the first in progress, else the first.
        let plan = self
            .board_plan
            .as_ref()
            .and_then(|id| knowledge.plan_details.iter().find(|p| p.id == *id))
            .or_else(|| {
                knowledge
                    .plan_details
                    .iter()
                    .find(|p| p.status == "in_progress")
            })
            .unwrap_or(&knowledge.plan_details[0]);

        let chips: Vec<_> =
            knowledge
                .plan_details
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let id = p.id.clone();
                    let done = p.tasks.iter().filter(|t| t.status == "done").count();
                    button(
                        ("plan-chip", i),
                        format!("{} {done}/{}", p.title, p.tasks.len()),
                    )
                    .when(p.id == plan.id, |b| {
                        b.border_color(theme::accent()).bg(theme::surface_hover())
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run(Command::SelectPlan(id.clone()), cx)
                    }))
                })
                .collect();
        let plan_id = plan.id.clone();
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .children(chips)
            .child(div().flex_1())
            .child(
                button("plan-open-web", "Open in fubbik").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.run(Command::OpenWeb(format!("/plans/{plan_id}")), cx)
                    },
                )),
            );

        let mut columns = Vec::new();
        for (title, statuses) in COLUMNS {
            let tasks: Vec<&Task> = plan
                .tasks
                .iter()
                .filter(|t| statuses.contains(&t.status.as_str()))
                .collect();
            let cards: Vec<AnyElement> = tasks
                .iter()
                .map(|task| self.task_card(repo, plan, task, cx))
                .collect();
            columns.push(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(200.))
                    .gap_2()
                    .p_2()
                    .rounded_lg()
                    .bg(theme::input())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title.to_uppercase()),
                            )
                            .child(muted(tasks.len().to_string())),
                    )
                    .children(cards),
            );
        }
        let done = plan.tasks.iter().filter(|t| t.status == "done").count() as i64;
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(header)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(plan.title.clone()),
                    )
                    .child(pill(
                        plan.status.replace('_', " "),
                        task_color(&plan.status),
                    ))
                    .child(progress(done, plan.tasks.len() as i64)),
            )
            .child(div().flex().gap_3().items_start().children(columns))
            .into_any_element()
    }

    fn task_card(
        &self,
        repo: usize,
        plan: &PlanDetail,
        task: &Task,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = |name: &str| SharedString::from(format!("card-{name}-{}", task.id));
        let blockers = open_dependencies(plan, task);
        let unblocks = plan
            .tasks
            .iter()
            .filter(|t| t.depends_on.contains(&task.id))
            .count();
        let holder = self.task_holder(repo, &plan.id, &task.id);
        let effort = self.task_effort(repo, &plan.id, task);
        let met = task.acceptance.iter().filter(|(_, done)| *done).count();

        let mut lines: Vec<String> = Vec::new();
        if !blockers.is_empty() {
            let names: Vec<&str> = blockers.iter().map(|t| t.title.as_str()).collect();
            lines.push(format!("waits on {}", names.join(", ")));
        }
        if unblocks > 0 {
            lines.push(format!("unblocks {unblocks}"));
        }
        if let Some((run, _)) = &holder {
            lines.push(format!("held by {}", run.handle));
        }
        if effort.sessions > 0 || effort.since.is_some() {
            let mut parts = Vec::new();
            if let Some(since) = effort.since {
                parts.push(format!(
                    "{} in progress",
                    activity::short_age(activity::now() - since)
                ));
            }
            if effort.sessions > 0 {
                parts.push(format!("{} session(s)", effort.sessions));
            }
            if effort.tokens > 0 {
                parts.push(format!("{}k tokens", effort.tokens / 1000));
            }
            lines.push(parts.join(" · "));
        }

        let (back, forward) = neighbours(&task.status);
        let mut actions = div().flex().flex_wrap().gap_1();
        if let Some(to) = back {
            let (p, t) = (plan.id.clone(), task.id.clone());
            actions = actions.child(button(key("back"), "◀").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.run(
                        Command::SetTaskStatus {
                            plan: p.clone(),
                            task: t.clone(),
                            status: to,
                        },
                        cx,
                    )
                },
            )));
        }
        if let Some(to) = forward {
            let (p, t) = (plan.id.clone(), task.id.clone());
            actions = actions.child(button(key("forward"), "▶").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.run(
                        Command::SetTaskStatus {
                            plan: p.clone(),
                            task: t.clone(),
                            status: to,
                        },
                        cx,
                    )
                },
            )));
        }
        if matches!(task.status.as_str(), "pending" | "in_progress") {
            let (p, t) = (plan.id.clone(), task.id.clone());
            actions = actions.child(button(key("block"), "Block").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.run(
                        Command::SetTaskStatus {
                            plan: p.clone(),
                            task: t.clone(),
                            status: "blocked",
                        },
                        cx,
                    )
                },
            )));
        }
        if task.status == "pending" && holder.is_none() && blockers.is_empty() {
            let (p, t) = (plan.id.clone(), task.id.clone());
            actions = actions.child(button(key("start"), "Start").on_click(cx.listener(
                move |this, _, _, cx| {
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
                },
            )));
        }
        let (p, t) = (plan.id.clone(), task.id.clone());
        actions = actions.child(button(key("timeline"), "Timeline").on_click(cx.listener(
            move |this, _, _, cx| {
                this.run(
                    Command::TaskTimeline {
                        repo,
                        plan: p.clone(),
                        task: t.clone(),
                    },
                    cx,
                )
            },
        )));

        let open = self.open_checklist.as_deref() == Some(task.id.as_str());
        let checklist = (!task.acceptance.is_empty()).then(|| {
            let toggle_id = task.id.clone();
            let summary = div()
                .id(key("criteria"))
                .cursor_pointer()
                .text_xs()
                .text_color(if met == task.acceptance.len() {
                    theme::status(Status::Running)
                } else {
                    theme::text_muted()
                })
                .hover(|d| d.text_color(theme::text()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run(Command::ToggleChecklist(toggle_id.clone()), cx)
                }))
                .child(format!(
                    "{} {met}/{} criteria",
                    if open { "▾" } else { "▸" },
                    task.acceptance.len()
                ));
            let mut items = Vec::new();
            if open {
                for (i, (text, done)) in task.acceptance.iter().enumerate() {
                    let (p, t) = (plan.id.clone(), task.id.clone());
                    items.push(
                        div()
                            .id(SharedString::from(format!("criterion-{}-{i}", task.id)))
                            .flex()
                            .gap_1p5()
                            .cursor_pointer()
                            .text_xs()
                            .hover(|d| d.bg(theme::surface_hover()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run(
                                    Command::ToggleCriterion {
                                        repo,
                                        plan: p.clone(),
                                        task: t.clone(),
                                        index: i,
                                    },
                                    cx,
                                )
                            }))
                            .child(if *done { "☑" } else { "☐" })
                            .child(text.clone()),
                    );
                }
            }
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(summary)
                .children(items)
        });
        let all_met =
            !task.acceptance.is_empty() && met == task.acceptance.len() && task.status != "done";

        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(if blockers.is_empty() {
                theme::border()
            } else {
                theme::status(Status::Degraded).opacity(0.5)
            })
            .bg(theme::surface())
            .shadow(theme::card_shadow())
            .when(task.status == "done", |d| d.opacity(0.6))
            .child(div().text_sm().child(task.title.clone()))
            .children(lines.into_iter().map(muted))
            .children(checklist)
            .when(all_met, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme::status(Status::Running))
                        .child("All criteria met"),
                )
            })
            .child(actions)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, status: &str, depends_on: &[&str]) -> Task {
        Task {
            id: id.into(),
            title: id.to_uppercase(),
            status: status.into(),
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn open_dependencies_ignore_finished_ones() {
        // Given
        let plan = PlanDetail {
            tasks: vec![
                task("a", "done", &[]),
                task("b", "in_progress", &[]),
                task("c", "pending", &["a", "b"]),
            ],
            ..Default::default()
        };

        // When
        let open = open_dependencies(&plan, &plan.tasks[2]);

        // Then
        assert_eq!(
            open.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn neighbours_walk_the_board() {
        // Given
        let statuses = ["pending", "in_progress", "done"];

        // When
        let moves: Vec<_> = statuses.iter().map(|s| neighbours(s)).collect();

        // Then
        assert_eq!(moves[0], (None, Some("in_progress")));
        assert_eq!(moves[1], (Some("pending"), Some("done")));
        assert_eq!(moves[2], (Some("in_progress"), None));
    }
}
