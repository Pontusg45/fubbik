//! Today: one screen across every repo for what needs you, what is in
//! flight, and what to start next. Rows are data first (`today_rows`), so
//! the list keys and ⇧⌘J / ⇧⌘S act on exactly what is shown.

use gpui::{AnyElement, FontWeight, Hsla, SharedString};

use super::plans::open_dependencies;
use super::widgets::{button, muted, pill, row, section_or};
use super::*;
use crate::activity;
use crate::model::Status;
use crate::sessions::SessionState;

fn section_order(section: Section) -> u8 {
    match section {
        Section::Needs => 0,
        Section::Flight => 1,
        Section::Next => 2,
    }
}

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
        // The tracker: alerts from watches need you; open todos are next.
        let tracker = crate::tracker::snapshot();
        for alert in tracker.alerts.iter().filter(|a| !a.dismissed) {
            let mut row = need(
                alert.repo.as_deref().unwrap_or(""),
                "watch".into(),
                theme::hue(crate::theme::Hue::Violet),
                alert.text.clone(),
                "Dismiss",
                Command::DismissAlert(alert.id.clone()),
            );
            if let Some(repo) = self
                .repos
                .iter()
                .position(|r| alert.repo.as_deref() == Some(r.entry.name().as_str()))
            {
                row.secondary = vec![(
                    "Open repo".into(),
                    Command::Focus {
                        repo,
                        tab: Tab::Overview,
                    },
                )];
            }
            rows.push(row);
        }
        for note in tracker.notes.iter().filter(|n| n.kind == "todo" && !n.done) {
            rows.push(TodayRow {
                section: Section::Next,
                repo: note.repo.clone().unwrap_or_default(),
                tag: Some(("todo".into(), theme::hue(crate::theme::Hue::Violet))),
                title: note.text.clone(),
                detail: None,
                primary: Some(Command::CompleteNote(note.id.clone())),
                primary_label: "Done",
                secondary: Vec::new(),
            });
        }
        // Terminal tabs that rang the bell (a program wants you).
        for (id, _) in &self.bells {
            let Some(tab) = self.terminals.iter().find(|t| t.id == *id) else {
                continue;
            };
            let repo = self
                .repos
                .iter()
                .find(|r| tab.dir.starts_with(&r.entry.path))
                .map(|r| r.entry.name())
                .unwrap_or_default();
            rows.push(need(
                &repo,
                "bell".into(),
                theme::hue(crate::theme::Hue::Orange),
                format!("{} rang the bell", tab.label),
                "Open the tab",
                Command::FocusTerminal(*id),
            ));
        }
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
                    let tag = session.agent.label().to_string();
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
                        "Show processes",
                        focus(Tab::Processes),
                    ));
                }
                // Pull requests of this repo's checked-out branches that
                // need you (failing checks, changes requested, conflicts).
                for worktree in snapshot
                    .worktrees
                    .as_ref()
                    .and_then(|w| w.as_ref().ok())
                    .into_iter()
                    .flatten()
                {
                    let Some(branch) = worktree.branch.as_deref() else {
                        continue;
                    };
                    let Some(pr) = self.pr_for(repo, branch) else {
                        continue;
                    };
                    if let Some(why) = pr.needs_you() {
                        let mut row = need(
                            &name,
                            "PR".into(),
                            theme::danger(),
                            format!("PR #{} ({branch}): {why}", pr.number),
                            "Open PR",
                            Command::OpenUrl(pr.url.clone()),
                        );
                        row.secondary = vec![("Show worktrees".into(), focus(Tab::Worktrees))];
                        rows.push(row);
                    }
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
                        "Show worktrees",
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
                    "Show checks",
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
                    "Show checks",
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
                        primary_label: "Open timeline",
                        secondary: vec![("Open board".into(), focus(Tab::Plans))],
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
                        primary_label: "Start in new worktree",
                        secondary: vec![("In background".into(), start(true))],
                    });
                }
            }
        }
        // Needs you: most urgent first (a broken repo, a failing process,
        // failing checks, a waiting agent, overlaps, stale knowledge).
        let rank = |row: &TodayRow| -> u8 {
            match row.tag.as_ref().map(|(t, _)| t.as_str()) {
                _ if row.section != Section::Needs => 9,
                Some("repo") => 0,
                Some("Error") => 1,
                Some("Degraded") => 2,
                Some("failing") | Some("PR") | Some("watch") => 3,
                Some("overlap") => 5,
                Some("stale") => 6,
                _ => 4, // an agent waiting
            }
        };
        rows.sort_by_key(|r| (section_order(r.section), rank(r)));
        rows
    }

    /// Today's rows as shown: filtered to one repo when a chip is picked.
    pub(super) fn today_view_rows(&self) -> Vec<TodayRow> {
        let rows = self.today_rows();
        match &self.today_filter {
            Some(repo) => rows.into_iter().filter(|r| &r.repo == repo).collect(),
            None => rows,
        }
    }

    pub(super) fn render_today(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.today_view_rows();
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
                    .child(div().text_sm().truncate().child(r.title.clone()))
                    .children(r.detail.clone().map(super::widgets::muted_line)),
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
            .children(self.today_caveat().map(super::widgets::loading))
            .child(self.today_filters(cx))
            .child(section_or(
                format!("Needs you ({})", needs.len()),
                needs,
                "All clear: nothing waits for you",
            ))
            .child(section_or(
                format!("In flight ({})", flight.len()),
                flight,
                "Nothing in progress",
            ))
            .child(section_or(
                format!("Next up ({})", next.len()),
                next,
                "No task is ready to start",
            ))
            .into_any_element()
    }

    /// What Today cannot see yet (so an empty list is not a false all-clear).
    fn today_caveat(&self) -> Option<String> {
        let checking = self.repos.iter().filter(|r| r.snapshot.is_none()).count();
        let offline: Vec<String> = self
            .repos
            .iter()
            .filter(|r| matches!(r.knowledge, Some(Err(_))))
            .map(|r| r.entry.name())
            .collect();
        let mut parts = Vec::new();
        if checking > 0 {
            parts.push(format!("{checking} repo(s) still checking"));
        }
        if !offline.is_empty() {
            parts.push(format!("knowledge offline for {}", offline.join(", ")));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// Repo chips: All, or one repo.
    fn today_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let chip = |id: SharedString,
                    text: String,
                    active: bool,
                    filter: Option<String>,
                    cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_2()
                .py(px(3.))
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .border_1()
                .border_color(if active {
                    theme::accent()
                } else {
                    theme::border()
                })
                .text_color(if active {
                    theme::text()
                } else {
                    theme::text_muted()
                })
                .hover(|d| d.bg(theme::surface_hover()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.today_filter = filter.clone();
                    this.list_cursor = None;
                    cx.notify();
                }))
                .child(text)
        };
        let mut chips = vec![
            chip(
                "today-all".into(),
                "All repos".into(),
                self.today_filter.is_none(),
                None,
                cx,
            )
            .into_any_element(),
        ];
        for view in &self.repos {
            let name = view.entry.name();
            let active = self.today_filter.as_deref() == Some(name.as_str());
            chips.push(
                chip(
                    format!("today-repo-{name}").into(),
                    name.clone(),
                    active,
                    Some(name),
                    cx,
                )
                .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(chips)
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
