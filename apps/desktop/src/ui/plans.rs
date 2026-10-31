//! The Plans tab: open plans, their tasks, who holds each task on the
//! coordination board, and starting a Claude session on a task.

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, SharedString, div, prelude::*, px};

use super::Workspace;
use super::palette::Command;
use super::widgets::{button, muted, pill, progress, row, section};
use crate::fubbik::{Knowledge, PlanDetail, Task};
use crate::model::Status;
use crate::sessions::{AiSession, SessionState};
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

impl Workspace {
    pub(super) fn render_plans(&self, repo: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let knowledge = match self.knowledge_or_notice(repo) {
            Ok(k) => k,
            Err(notice) => return notice,
        };
        let coordination = self
            .global
            .as_ref()
            .and_then(|g| g.api.as_ref().ok())
            .is_none_or(|(_, features)| features.iter().any(|f| f == "coordination"));
        let plans: Vec<&PlanDetail> = knowledge.plan_details.iter().collect();
        if plans.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_sm().child("No open plans in this space"))
                .child(muted("Create one with `fubbik plan create`, the MCP `create_plan` tool, or the web app."))
                .child(div().flex().child(
                    button("plans-web", "Open plans in fubbik").on_click(cx.listener(|this, _, _, cx| {
                        this.run(Command::OpenWeb("/plans".into()), cx)
                    })),
                ))
                .into_any_element();
        }
        let sessions: Vec<AiSession> = self.repos[repo]
            .snapshot
            .iter()
            .flat_map(|s| s.sessions.iter().cloned())
            .collect();

        let mut column = div().flex().flex_col().gap_6();
        if !coordination {
            column = column.child(muted(
                "The coordination board is switched off (fubbik.toml), so sessions start without claiming their task.",
            ));
        }
        for (p, plan) in plans.into_iter().enumerate() {
            let done = plan.tasks.iter().filter(|t| t.status == "done").count() as i64;
            let total = plan.tasks.len() as i64;
            let plan_id = plan.id.clone();
            let header = div()
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
                .child(progress(done, total))
                .child(muted(format!("{done}/{total} done")))
                .child(
                    button(SharedString::from(format!("plan-web-{p}")), "Open").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.run(Command::OpenWeb(format!("/plans/{plan_id}")), cx)
                        }),
                    ),
                );
            let rows: Vec<AnyElement> = plan
                .tasks
                .iter()
                .enumerate()
                .map(|(t, task)| self.task_row(repo, p, t, plan, task, knowledge, &sessions, cx))
                .collect();
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(header)
                    .child(section("Tasks", rows)),
            );
        }
        column.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn task_row(
        &self,
        repo: usize,
        p: usize,
        t: usize,
        plan: &PlanDetail,
        task: &Task,
        knowledge: &Knowledge,
        sessions: &[AiSession],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = |name: &str| SharedString::from(format!("task-{name}-{p}-{t}"));
        // Who holds it: a live claim on this plan's board.
        let holder = knowledge
            .boards
            .iter()
            .filter(|b| b.plan_id == plan.id)
            .find_map(|board| {
                let claim = board
                    .claims
                    .iter()
                    .find(|c| c.task_id == task.id && !c.expired)?;
                board.runs.iter().find(|r| r.id == claim.run_id)
            });
        let holder_session = holder
            .and_then(|run| run.session_id())
            .and_then(|sid| sessions.iter().find(|s| s.id == sid));
        let holder_text = match (holder, holder_session) {
            (Some(run), Some(session)) => format!(
                "held by {} · {}",
                run.handle,
                session.state.label().to_lowercase()
            ),
            (Some(run), None) => format!("held by {}", run.handle),
            (None, _) => String::new(),
        };
        let criteria = if task.acceptance.is_empty() {
            String::new()
        } else {
            let met = task.acceptance.iter().filter(|(_, done)| *done).count();
            format!("{met}/{} criteria", task.acceptance.len())
        };

        let mut actions = div().flex().gap_1();
        let startable = matches!(task.status.as_str(), "pending" | "blocked") && holder.is_none();
        if startable {
            for (name, label, new_worktree, background) in [
                ("start-wt", "Start in new worktree", true, false),
                ("start-bg", "Start in background", true, true),
                ("start-here", "Start here", false, false),
            ] {
                let (plan_id, task_id) = (plan.id.clone(), task.id.clone());
                actions = actions.child(button(id(name), label).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.run(
                            Command::StartTask {
                                repo,
                                plan: plan_id.clone(),
                                task: task_id.clone(),
                                new_worktree,
                                background,
                            },
                            cx,
                        )
                    },
                )));
            }
        }
        if let Some(session) = holder_session.filter(|s| s.state == SessionState::Ended) {
            let (agent, sid, cwd) = (session.agent, session.id.clone(), session.cwd.clone());
            actions = actions.child(button(id("resume"), "Resume session").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.run(Command::ResumeSession(agent, sid.clone(), cwd.clone()), cx)
                },
            )));
        }

        let detail: Vec<String> = [criteria, holder_text]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        row()
            .when(task.status == "done", |r| r.opacity(0.55))
            .child(div().w(px(92.)).child(pill(
                task.status.replace('_', " "),
                task_color(&task.status),
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(div().text_sm().child(task.title.clone()))
                    .child(muted(detail.join("  ·  "))),
            )
            .child(actions)
            .into_any_element()
    }
}
