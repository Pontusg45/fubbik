//! Insights for one repo: a standup or weekly summary (copy it, or save it
//! as a chunk), a health score with its reasons, a four-week activity
//! heatmap, stale branches and idle worktrees. Computed on demand in the
//! background.

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{
    button, disabled_button, label, loading, muted, pill, row, section, section_or,
};
use super::*;
use crate::activity::{self, Branch, Health, HealthInputs};

/// Heatmap span, in days (four weeks).
const HEATMAP_DAYS: usize = 28;
/// A branch with no commit for this long is stale.
const STALE_BRANCH_DAYS: i64 = 30;
/// A non-main worktree with no commit for this long is idle.
const IDLE_WORKTREE_DAYS: i64 = 7;

#[derive(Debug, Clone)]
pub struct Insights {
    pub days: i64,
    pub summary: String,
    pub health: Health,
    pub commits_per_day: Vec<u32>,
    pub sessions_per_day: Vec<u32>,
    pub stale_branches: Vec<Branch>,
    pub idle_worktrees: Vec<(String, i64)>,
}

impl Workspace {
    pub(super) fn load_insights(&mut self, repo: usize, days: i64, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get_mut(repo) else {
            return;
        };
        // Keep what is shown while recomputing; only a first load blanks.
        if matches!(view.insights, Some(Loadable::Done(Ok(_)))) {
            view.insights_refreshing = true;
        } else {
            view.insights = Some(Loadable::Loading);
        }
        let path = view.entry.path.clone();
        let name = view.entry.name();
        let snapshot = view.snapshot.clone().unwrap_or_default();
        let knowledge = view.knowledge.clone().and_then(Result::ok);
        let audit = view.audit;
        let coverage = match &view.gaps {
            Some(Loadable::Done(Ok(report))) => Some(report.coverage()),
            _ => None,
        };
        let work = cx.background_executor().spawn(async move {
            let now = activity::now();
            let main = snapshot
                .worktrees
                .as_ref()
                .and_then(|w| w.as_ref().ok()?.first()?.branch.clone())
                .unwrap_or_else(|| "main".into());
            let commits = activity::commits_since(&path, HEATMAP_DAYS as i64);
            let in_period: Vec<activity::Commit> = commits
                .iter()
                .filter(|c| now - c.time <= days * 86_400)
                .cloned()
                .collect();
            let within = |at: &Option<String>| {
                at.as_deref()
                    .and_then(activity::parse_rfc3339)
                    .is_some_and(|t| now - t <= days * 86_400)
            };
            let (mut done, mut doing, mut blocked) = (Vec::new(), Vec::new(), Vec::new());
            for plan in knowledge.iter().flat_map(|k| k.plan_details.iter()) {
                for task in &plan.tasks {
                    let label = format!("{} ({})", task.title, plan.title);
                    match task.status.as_str() {
                        "done" if within(&task.updated_at) => done.push(label),
                        "in_progress" => doing.push(label),
                        "blocked" => blocked.push(label),
                        _ => {}
                    }
                }
            }
            let session_age = |s: &crate::sessions::AiSession| {
                s.last_activity
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            };
            let sessions: Vec<String> = snapshot
                .sessions
                .iter()
                .filter(|s| now - session_age(s) <= days * 86_400)
                .map(|s| format!("{}: {}", s.agent.label(), s.title))
                .collect();
            let requirements = knowledge.as_ref().map(|k| {
                (
                    k.requirements.passing,
                    k.requirements.failing,
                    k.requirements.untested,
                )
            });
            let summary = activity::summary(&activity::Period {
                repo: &name,
                days,
                commits: &in_period,
                tasks_done: &done,
                tasks_in_progress: &doing,
                tasks_blocked: &blocked,
                sessions: &sessions,
                requirements,
            });
            let commits_per_day = activity::heatmap(
                &commits.iter().map(|c| c.time).collect::<Vec<_>>(),
                HEATMAP_DAYS,
                now,
            );
            let sessions_per_day = activity::heatmap(
                &snapshot
                    .sessions
                    .iter()
                    .map(session_age)
                    .collect::<Vec<_>>(),
                HEATMAP_DAYS,
                now,
            );
            let stale_branches = activity::stale_branches(&path, &main, STALE_BRANCH_DAYS);
            let idle_worktrees: Vec<(String, i64)> = snapshot
                .worktrees
                .as_ref()
                .and_then(|w| w.as_ref().ok())
                .into_iter()
                .flatten()
                .filter(|w| !w.main && !w.missing)
                .filter_map(|w| {
                    let last = activity::commits_since(&w.path, 3650)
                        .first()
                        .map(|c| c.time)?;
                    let age = (now - last) / 86_400;
                    (age >= IDLE_WORKTREE_DAYS).then(|| (w.name(), age))
                })
                .collect();
            let health = activity::health(&HealthInputs {
                failing_processes: snapshot
                    .processes
                    .iter()
                    .filter(|p| {
                        matches!(
                            p.status,
                            crate::model::Status::Degraded | crate::model::Status::Error
                        )
                    })
                    .count(),
                failing_requirements: knowledge.as_ref().map_or(0, |k| k.requirements.failing),
                violated_cells: knowledge.as_ref().map_or(0, |k| k.violated_cells()),
                stale_branches: stale_branches.len(),
                idle_worktrees: idle_worktrees.len(),
                stale_knowledge: knowledge.as_ref().map_or(0, |k| k.stale),
                uncommitted_files: snapshot
                    .git
                    .as_ref()
                    .and_then(|g| g.as_ref().ok())
                    .map_or(0, |g| g.changed_files),
                vulnerabilities: audit,
                coverage,
            });
            Insights {
                days,
                summary,
                health,
                commits_per_day,
                sessions_per_day,
                stale_branches,
                idle_worktrees,
            }
        });
        cx.spawn(async move |this, cx| {
            let insights = work.await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.repos.get_mut(repo) {
                    view.insights = Some(Loadable::Done(Ok(insights)));
                    view.insights_refreshing = false;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `cargo audit` / `pnpm audit` when installed; feeds the health score.
    pub(super) fn run_audit(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        if view.auditing {
            return;
        }
        let path = view.entry.path.clone();
        if let Some(view) = self.repos.get_mut(repo) {
            view.auditing = true;
        }
        let work = cx
            .background_executor()
            .spawn(async move { activity::audit(&path) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.repos.get_mut(repo) {
                    view.auditing = false;
                }
                match result {
                    None => this.show_info("No audit tool for this repo (cargo-audit or pnpm)"),
                    Some(Err(error)) => this.show_error(error),
                    Some(Ok(n)) => {
                        if let Some(view) = this.repos.get_mut(repo) {
                            view.audit = Some(n);
                        }
                        this.show_info(format!("{n} known vulnerabilit(ies)"));
                        let days = this.insights_days;
                        this.load_insights(repo, days, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_insights(&mut self, repo: usize, cx: &mut Context<Self>) -> AnyElement {
        let days = self.insights_days;
        let loaded = self.repos[repo].insights.clone();
        let stale = !matches!(&loaded, Some(Loadable::Done(Ok(i))) if i.days == days);
        let refreshing = self.repos[repo].insights_refreshing;
        if loaded.is_none() || (stale && !refreshing && !matches!(loaded, Some(Loadable::Loading)))
        {
            self.load_insights(repo, days, cx);
        }
        let controls = div()
            .flex()
            .items_center()
            .gap_2()
            .children([(1, "Today"), (7, "This week")].map(|(n, text)| {
                button(SharedString::from(format!("insights-{n}")), text)
                    .when(days == n, |b| {
                        b.border_color(theme::accent()).bg(theme::surface_hover())
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.insights_days = n;
                        this.load_insights(repo, n, cx);
                        cx.notify();
                    }))
            }))
            .child(button("insights-refresh", "Refresh").on_click(cx.listener(
                move |this, _, _, cx| {
                    let n = this.insights_days;
                    this.load_insights(repo, n, cx);
                    cx.notify();
                },
            )))
            .child(if self.repos[repo].auditing {
                disabled_button("insights-audit", "Auditing…").into_any_element()
            } else {
                button("insights-audit", "Run audit")
                    .on_click(cx.listener(move |this, _, _, cx| this.run_audit(repo, cx)))
                    .into_any_element()
            })
            .children(self.repos[repo].audit.map(|n| {
                muted(if n == 0 {
                    "no known vulnerabilities".to_string()
                } else {
                    format!("{n} known vulnerabilit(ies)")
                })
            }))
            .when(refreshing, |d| d.child(loading("Refreshing…")));
        let Some(Loadable::Done(Ok(insights))) = loaded else {
            return div()
                .flex()
                .flex_col()
                .gap_4()
                .child(controls)
                .child(loading("Reading git history, sessions and knowledge…"))
                .into_any_element();
        };

        let summary_text = insights.summary.clone();
        let (copy_text, save_text) = (summary_text.clone(), summary_text.clone());
        let summary = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(label("SUMMARY"))
                    .child(button("summary-copy", "Copy Markdown").on_click(
                        cx.listener(move |this, _, _, cx| this.copy(copy_text.clone(), cx)),
                    ))
                    .when(self.repos[repo].space_id().is_none(), |d| {
                        d.child(disabled_button("summary-save", "Save as chunk"))
                            .child(muted("link a fubbik space (Setup) to save it"))
                    })
                    .when(self.repos[repo].space_id().is_some(), |d| {
                        d.child(
                            button("summary-save", "Save as chunk").on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let Some(space) =
                                        this.repos.get(repo).and_then(|r| r.space_id())
                                    else {
                                        this.show_error("This repo has no fubbik space");
                                        return;
                                    };
                                    let title = save_text
                                        .lines()
                                        .next()
                                        .unwrap_or("Summary")
                                        .trim_start_matches("## ")
                                        .to_string();
                                    let content = save_text.clone();
                                    this.api_action(
                                        "Saved the summary as a chunk",
                                        move |c| c.quick_note(&space, &title, &content),
                                        cx,
                                    );
                                },
                            )),
                        )
                    }),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(theme::input())
                    .font_family("Menlo")
                    .text_xs()
                    .children(summary_text.lines().map(|l| div().child(l.to_string()))),
            );

        let score_color = match insights.health.score {
            80.. => theme::status(crate::model::Status::Running),
            50..80 => theme::status(crate::model::Status::Degraded),
            _ => theme::danger(),
        };
        let mut health_rows: Vec<AnyElement> = insights
            .health
            .reasons
            .iter()
            .map(|(points, reason)| {
                row()
                    .child(div().w(px(56.)).child(muted(format!("−{points}"))))
                    .child(div().text_sm().child(reason.clone()))
                    .into_any_element()
            })
            .collect();
        if health_rows.is_empty() {
            health_rows.push(
                row()
                    .child(muted("Nothing is costing points."))
                    .into_any_element(),
            );
        }
        let health = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(label("HEALTH"))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(score_color)
                            .child(format!("{} / 100", insights.health.score)),
                    )
                    .when(self.repos[repo].audit.is_none(), |d| {
                        d.child(muted("dependency audit not run"))
                    }),
            )
            .child(section("What costs points", health_rows));

        let heat = |title: &str, counts: &[u32], color: gpui::Hsla| {
            let peak = counts.iter().copied().max().unwrap_or(0).max(1);
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(div().w(px(110.)).child(muted(title.to_string())))
                .child(div().flex().gap(px(3.)).children(counts.iter().map(|&n| {
                    let strength = if n == 0 {
                        0.0
                    } else {
                        0.25 + 0.75 * n as f32 / peak as f32
                    };
                    div().size(px(14.)).rounded_sm().bg(if n == 0 {
                        theme::border()
                    } else {
                        color.opacity(strength)
                    })
                })))
                .child(muted(format!("{} total", counts.iter().sum::<u32>())))
        };
        let dates = div()
            .flex()
            .items_center()
            .gap_3()
            .child(div().w(px(110.)))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .w(px(HEATMAP_DAYS as f32 * 17.0 - 3.0))
                    .child(muted(format!("{HEATMAP_DAYS} days ago")))
                    .child(muted("today")),
            );
        let heatmap = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(label(format!("ACTIVITY — LAST {HEATMAP_DAYS} DAYS")))
            .child(heat(
                "commits",
                &insights.commits_per_day,
                theme::hue(theme::Hue::Green),
            ))
            .child(heat(
                "agent sessions",
                &insights.sessions_per_day,
                theme::agent(crate::sessions::Agent::ClaudeCode),
            ))
            .child(dates);

        let branches: Vec<AnyElement> = insights
            .stale_branches
            .iter()
            .map(|b| {
                let name = b.name.clone();
                row()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_sm()
                            .truncate()
                            .child(b.name.clone()),
                    )
                    .child(muted(format!("{}d since last commit", b.age_days)))
                    .when(b.merged, |r| {
                        r.child(
                            button(
                                SharedString::from(format!("branch-delete-{name}")),
                                "Delete",
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.run(
                                        Command::DeleteBranch {
                                            repo,
                                            name: name.clone(),
                                        },
                                        cx,
                                    )
                                },
                            )),
                        )
                    })
                    .child(pill(
                        if b.merged { "merged" } else { "not merged" },
                        if b.merged {
                            theme::status(crate::model::Status::Running)
                        } else {
                            theme::text_muted()
                        },
                    ))
                    .into_any_element()
            })
            .collect();
        let idle: Vec<AnyElement> = insights
            .idle_worktrees
            .iter()
            .map(|(name, age)| {
                row()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_sm()
                            .truncate()
                            .child(name.clone()),
                    )
                    .child(muted(format!("{age}d since last commit")))
                    .child(
                        button(
                            SharedString::from(format!("idle-{name}")),
                            "Show in Worktrees",
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run(
                                Command::Focus {
                                    repo,
                                    tab: Tab::Worktrees,
                                },
                                cx,
                            )
                        })),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(controls)
            .child(summary)
            .child(health)
            .child(heatmap)
            .child(section_or(
                format!("Stale branches (no commit for {STALE_BRANCH_DAYS}+ days)"),
                branches,
                "No stale branches",
            ))
            .child(section_or(
                format!("Idle worktrees (no commit for {IDLE_WORKTREE_DAYS}+ days)"),
                idle,
                "No idle worktrees",
            ))
            .into_any_element()
    }
}
