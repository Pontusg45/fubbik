//! Registered repos, grouped, with group-wide start and stop.

use gpui::{Context, FontWeight, IntoElement, SharedString, div, prelude::*, px};

use super::palette::Command;
use super::widgets::{button, muted};
use super::{Tab, Workspace, display_path};
use crate::model::Status;
use crate::sessions::SessionState;
use crate::theme;

/// Failing requirements plus violated matrix cells.
fn failing(repo: &super::RepoView) -> i64 {
    match &repo.knowledge {
        Some(Ok(k)) => k.requirements.failing + k.violated_cells(),
        _ => 0,
    }
}

impl Workspace {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut groups: Vec<Option<String>> = vec![None];
        for repo in &self.repos {
            if let Some(group) = &repo.entry.group
                && !groups.contains(&Some(group.clone()))
            {
                groups.push(Some(group.clone()));
            }
        }

        let mut sections = Vec::new();
        for group in groups {
            let members = self.repos_in_group(group.as_deref());
            if members.is_empty() && group.is_some() {
                continue;
            }
            sections.push(self.render_group(group, members, cx).into_any_element());
        }

        div()
            .id("sidebar")
            .flex()
            .flex_col()
            .w(px(250.))
            .h_full()
            .flex_none()
            .bg(theme::sidebar())
            .border_r_1()
            .border_color(theme::border())
            // ⇧⌘E: j/k now move through the repos.
            .when(self.sidebar_nav, |d| {
                d.border_r_2().border_color(theme::accent())
            })
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .pb_2()
                    .child(self.home_row(cx))
                    .child(self.global_row("Today", Tab::Today, cx))
                    .children(sections),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(theme::border())
                    .children(self.backend_seen.short().map(|(label, health)| {
                        use crate::backend::Health;
                        let color = match health {
                            Health::Good => theme::status(crate::model::Status::Running),
                            Health::Pending => theme::status(crate::model::Status::Degraded),
                            Health::Bad => theme::danger(),
                        };
                        div()
                            .id("backend-status")
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run(Command::ShowTab(super::Tab::Settings), cx);
                                cx.notify();
                            }))
                            .child(div().size(px(7.)).rounded_full().bg(color))
                            .child(label)
                    }))
                    .child(
                        button("register-repo", "+ Register repo")
                            .text_sm()
                            .py_1p5()
                            .on_click(
                                cx.listener(|this, _, _, cx| this.run(Command::RegisterRepo, cx)),
                            ),
                    )
                    .child(muted(format!(
                        "{} commands · {} {} switch repo · {} shortcuts",
                        super::keymap::keys_for("toggle_palette")
                            .map(|k| super::keymap::pretty(&k))
                            .unwrap_or_default(),
                        super::keymap::keys_for("prev_repo")
                            .map(|k| super::keymap::pretty(&k))
                            .unwrap_or_default(),
                        super::keymap::keys_for("next_repo")
                            .map(|k| super::keymap::pretty(&k))
                            .unwrap_or_default(),
                        super::keymap::keys_for("show_cheat_sheet")
                            .map(|k| super::keymap::pretty(&k))
                            .unwrap_or_default(),
                    ))),
            )
    }

    fn render_group(
        &self,
        group: Option<String>,
        members: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let title = group
            .clone()
            .unwrap_or_else(|| "REPOSITORIES".into())
            .to_uppercase();
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .pt_4()
            .pb_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text_muted())
                    .child(title),
            )
            .when_some(group.clone(), |header, name| {
                let start = members.clone();
                let stop = members.clone();
                header.child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            button(SharedString::from(format!("group-start-{name}")), "▶ all")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.ask_where(super::RunWhat::Group(start.clone()), cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            button(SharedString::from(format!("group-stop-{name}")), "■ all")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    for &repo in &stop {
                                        this.stop_all(repo, cx);
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            });

        let rows = members.into_iter().map(|index| {
            let repo = &self.repos[index];
            let status = repo
                .snapshot
                .as_ref()
                .map_or(Status::Stopped, |s| s.status());
            let waiting = repo.snapshot.as_ref().map_or(0, |s| {
                s.sessions
                    .iter()
                    .filter(|s| s.state == SessionState::Idle)
                    .count()
            });
            // Most actionable first: broken, waiting for you, failing, busy.
            let (detail, urgent) = match &repo.snapshot {
                None => ("checking…".to_string(), false),
                Some(s) if s.missing => ("folder missing".to_string(), true),
                Some(s) if s.services_error.is_some() => ("services file error".to_string(), true),
                Some(_) if waiting > 0 => (format!("{waiting} agent(s) waiting for you"), true),
                _ if failing(repo) > 0 => (format!("✕ {} failing checks", failing(repo)), true),
                Some(s) if s.live_sessions() > 0 => {
                    let busy = s
                        .sessions
                        .iter()
                        .filter(|s| s.state == SessionState::Busy)
                        .count();
                    (format!("{busy} agent(s) working"), false)
                }
                _ => (display_path(&repo.entry.path), false),
            };
            // A repo with busy agents but no services is not "stopped".
            let dot_color = match &repo.snapshot {
                None => theme::text_muted().opacity(0.4),
                Some(s) if status == Status::Stopped && s.live_sessions() > 0 => {
                    theme::agent(crate::sessions::Agent::ClaudeCode)
                }
                _ => theme::status(status),
            };
            let selected = index == self.selected && !self.home && !self.tab.is_global();
            div()
                .id(("repo", index))
                .group("repo-row")
                .flex()
                .items_center()
                .gap_2()
                .mx_2()
                .px_3()
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .when(selected, |row| row.bg(theme::surface_hover()))
                .hover(|row| row.bg(theme::surface_hover()))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.run(Command::SelectRepo(index), cx)),
                )
                .border_l_2()
                .border_color(if selected {
                    theme::name_color(&repo.entry.name())
                } else {
                    gpui::transparent_black()
                })
                .child(super::views::repo_badge(&repo.entry.name(), 20.))
                .child(div().size_2().flex_none().rounded_full().bg(dot_color))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .child(div().text_sm().truncate().child(repo.entry.name()))
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(if status == Status::Error || urgent {
                                    if waiting > 0 && status != Status::Error {
                                        theme::status(Status::Degraded)
                                    } else {
                                        theme::danger()
                                    }
                                } else {
                                    theme::text_muted()
                                })
                                .child(detail),
                        ),
                )
                .child(
                    div()
                        .id(("remove", index))
                        .px_1()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .invisible()
                        .group_hover("repo-row", |s| s.visible())
                        .hover(|s| s.text_color(theme::danger()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.run(Command::RemoveRepo(index), cx);
                        }))
                        .child("✕"),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(header)
            .children(rows)
    }
}
