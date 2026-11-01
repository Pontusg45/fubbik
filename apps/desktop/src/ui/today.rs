//! Today: one screen across every repo for what needs you, what is in
//! flight, and what to start next. Rows are data first (`today_rows`), so
//! the list keys and ⇧⌘J / ⇧⌘S act on exactly what is shown.

use gpui::{AnyElement, FontWeight, Hsla, SharedString};

use super::plans::open_dependencies;
use super::widgets::{button, muted, pill, row, section};
use super::*;
use crate::activity;
use crate::model::Status;
use crate::sessions::SessionState;

/// Pending tasks shown per plan under "Next up".
const NEXT_PER_PLAN: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Needs,
    Flight,
    Next,
}

#[derive(Debug, Clone)]
pub struct TodayRow {
    pub section: Section,
    pub repo: String,
    pub tag: Option<(String, Hsla)>,
    pub title: String,
    pub detail: Option<String>,
    /// What ⏎ (and the first button) does.
    pub primary: Option<Command>,
    pub primary_label: &'static str,
    pub secondary: Vec<(String, Command)>,
}

impl Workspace {
    pub(super) fn today_rows(&self) -> Vec<TodayRow> {
        let mut rows = Vec::new();
        let now = activity::now();
        let need = |repo: &str,
                    tag: String,
                    color: Hsla,
                    title: String,
                    label: &'static str,
                    command: Command| TodayRow {
            section: Section::Needs,
            repo: repo.to_string(),
            tag: Some((tag, color)),
            title,
            detail: None,
            primary: Some(command),
            primary_label: label,
            secondary: Vec::new(),
        };
        for (repo, view) in self.repos.iter().enumerate() {
            let name = view.entry.name();
            let focus = |tab: Tab| Command::Focus { repo, tab };
            if let Some(snapshot) = &view.snapshot {
                if snapshot.missing || snapshot.services_error.is_some() {
                    let what = if snapshot.missing {
                        "folder is missing"
                    } else {
                        "services file has an error"
                    };
                    rows.push(need(
                        &name,
                        "repo".into(),
                        theme::danger(),
                        what.into(),
                        "Open",
                        focus(Tab::Overview),
                    ));
                }
                for session in snapshot
                    .sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Idle)
                {
                    let command = match &session.pane {
                        Some(pane) => Command::OpenPane {
                            pane: pane.clone(),
                            cwd: session.cwd.clone(),
                        },
                        None => focus(Tab::Sessions),
                    };
                    let tag = format!("{} waits", session.agent.label());
                    rows.push(need(
                        &name,
                        tag,
                        theme::agent(session.agent),
                        session.title.clone(),
                        "Open",
                        command,
                    ));
                }
                for process in snapshot
                    .processes
                    .iter()
                    .filter(|p| matches!(p.status, Status::Degraded | Status::Error))
                {
                    let status = process.status;
                    rows.push(need(
                        &name,
                        status.label().into(),
                        theme::status(status),
                        process.spec.name.clone(),
                        "Processes",
                        focus(Tab::Processes),
                    ));
                }
                for overlap in &snapshot.overlaps {
                    let title = format!(
                        "{} and {} both change {} file(s)",
                        overlap.a,
                        overlap.b,
                        overlap.files.len()
                    );
                    rows.push(need(
                        &name,
                        "overlap".into(),
                        theme::status(Status::Degraded),
                        title,
                        "Worktrees",
                        focus(Tab::Worktrees),
                    ));
                }
            }
            let Some(Ok(knowledge)) = &view.knowledge else {
                continue;
            };
            let failing = knowledge.requirements.failing + knowledge.violated_cells();
            if failing > 0 {
                let title = format!("{failing} failing requirement(s) or behaviour cell(s)");
                rows.push(need(
                    &name,
                    "failing".into(),
                    theme::danger(),
                    title,
                    "Knowledge",
                    focus(Tab::Knowledge),
                ));
            }
            if knowledge.stale > 0 {
                let title = format!("{} stale knowledge flag(s)", knowledge.stale);
                rows.push(need(
                    &name,
                    "stale".into(),
                    theme::status(Status::Degraded),
                    title,
                    "Knowledge",
                    focus(Tab::Knowledge),
                ));
            }
            for plan in &knowledge.plan_details {
                for task in plan.tasks.iter().filter(|t| t.status == "in_progress") {
                    let effort = self.task_effort(repo, &plan.id, task);
                    let mut detail = vec![plan.title.clone()];
                    if let Some((run, _)) = self.task_holder(repo, &plan.id, &task.id) {
                        detail.push(run.handle);
                    }
                    if let Some(since) = effort.since {
                        detail.push(format!("{} in progress", activity::short_age(now - since)));
                    }
                    if effort.tokens > 0 {
                        detail.push(format!("{}k tokens", effort.tokens / 1000));
                    }
                    rows.push(TodayRow {
                        section: Section::Flight,
                        repo: name.clone(),
                        tag: None,
                        title: task.title.clone(),
                        detail: Some(detail.join("  ·  ")),
                        primary: Some(Command::TaskTimeline {
                            repo,
                            plan: plan.id.clone(),
                            task: task.id.clone(),
                        }),
                        primary_label: "Timeline",
                        secondary: vec![("Board".into(), focus(Tab::Plans))],
                    });
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
                for task in startable {
                    let start = |background| Command::StartTask {
                        repo,
                        plan: plan.id.clone(),
                        task: task.id.clone(),
                        new_worktree: true,
                        background,
                    };
                    rows.push(TodayRow {
                        section: Section::Next,
                        repo: name.clone(),
                        tag: None,
                        title: task.title.clone(),
                        detail: Some(plan.title.clone()),
                        primary: Some(start(false)),
                        primary_label: "Start",
                        secondary: vec![("In background".into(), start(true))],
                    });
                }
            }
        }
        rows
    }

    pub(super) fn render_today(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.today_rows();
        let mut sections: [Vec<AnyElement>; 3] = Default::default();
        for (index, r) in rows.iter().enumerate() {
            let key = |name: &str| SharedString::from(format!("today-{name}-{index}"));
            let highlighted = self.tab == Tab::Today && self.list_cursor == Some(index);
            let mut line = row()
                .when(highlighted, |d| d.bg(theme::surface_hover()))
                .child(super::views::repo_badge(&r.repo, 18.));
            if let Some((tag, color)) = &r.tag {
                line = line.child(
                    div()
                        .w(px(124.))
                        .flex_none()
                        .child(pill(tag.clone(), *color)),
                );
            }
            line = line.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(div().text_sm().child(r.title.clone()))
                    .child(muted(match &r.detail {
                        Some(detail) => format!("{} · {detail}", r.repo),
                        None => r.repo.clone(),
                    })),
            );
            if let Some(command) = r.primary.clone() {
                line = line
                    .child(button(key("primary"), r.primary_label).on_click(
                        cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)),
                    ));
            }
            for (i, (label, command)) in r.secondary.iter().enumerate() {
                let command = command.clone();
                line = line
                    .child(button(key(&format!("extra-{i}")), label.clone()).on_click(
                        cx.listener(move |this, _, _, cx| this.run(command.clone(), cx)),
                    ));
            }
            let slot = match r.section {
                Section::Needs => 0,
                Section::Flight => 1,
                Section::Next => 2,
            };
            sections[slot].push(line.into_any_element());
        }
        let queued = self.launch_queue.len();
        let [needs, flight, next] = sections;
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
