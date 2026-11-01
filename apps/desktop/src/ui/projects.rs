//! Projects (⌘0): every registered repo as a card, and the computer's own
//! state beside them: CPU, memory, disk, battery, network, uptime, the
//! busiest processes and who holds which port.

use gpui::{AnyElement, FontWeight, Hsla, SharedString, relative};

use super::today::Section;
use super::views::repo_badge;
use super::widgets::{label, muted, pill, status_pill, top_highlight};
use super::*;
use crate::model::Status;
use crate::system::{self, Stats, bytes, percent};

/// Below this much free space the disk card turns red.
const LOW_DISK: u64 = 10 * 1024 * 1024 * 1024;

fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .shadow(theme::card_shadow())
        .relative()
        .overflow_hidden()
        .child(top_highlight())
}

/// A filled bar, `value` of 100.
fn bar(value: f32, color: Hsla) -> gpui::Div {
    div()
        .h(px(6.))
        .w_full()
        .rounded_full()
        .bg(theme::border())
        .child(
            div()
                .h_full()
                .rounded_full()
                .w(relative((value / 100.0).clamp(0.0, 1.0)))
                .bg(color),
        )
}

/// Cards in two columns (first, third… left; second, fourth… right).
fn two_columns(cards: Vec<AnyElement>) -> gpui::Div {
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (i, card) in cards.into_iter().enumerate() {
        if i % 2 == 0 {
            left.push(card)
        } else {
            right.push(card)
        }
    }
    let column = |cards: Vec<AnyElement>| {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(260.))
            .gap_3()
            .children(cards)
    };
    div()
        .flex()
        .flex_1()
        .items_start()
        .gap_3()
        .child(column(left))
        .child(column(right))
}

/// A wide history graph of percentages: a fixed number of slots, newest on
/// the right, so it fills in from the right as samples arrive.
fn graph(values: &[f32], color: Hsla) -> gpui::Div {
    let height = 36.0;
    let slots: usize = 60;
    let padded: Vec<f32> = std::iter::repeat_n(0.0, slots.saturating_sub(values.len()))
        .chain(values.iter().copied())
        .collect();
    div()
        .flex()
        .items_end()
        .gap(px(1.))
        .h(px(height))
        .w_full()
        .children(padded.into_iter().map(move |v| {
            div()
                .flex_1()
                .h(px((v / 100.0 * height).clamp(1.0, height)))
                .rounded_sm()
                .bg(if v > 0.0 {
                    color.opacity(0.75)
                } else {
                    theme::border()
                })
        }))
}

/// Green, then amber past 70%, red past 90%.
fn load_color(value: f32) -> Hsla {
    match value {
        v if v >= 90.0 => theme::danger(),
        v if v >= 70.0 => theme::status(Status::Degraded),
        _ => theme::status(Status::Running),
    }
}

fn stat_header(title: &str, value: String) -> gpui::Div {
    div()
        .flex()
        .items_baseline()
        .justify_between()
        .child(label(title.to_uppercase()))
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(value),
        )
}

impl Workspace {
    /// Home: the projects and the computer, filling the content area.
    pub(super) fn render_home(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .child(
                div()
                    .id("home")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_6()
                    .pt(px(44.))
                    .pb_6()
                    .child(self.render_projects(cx)),
            )
            .children(self.list_hint().map(|hint| {
                div()
                    .px_6()
                    .py_2()
                    .border_t_1()
                    .border_color(theme::border())
                    .child(muted(hint))
            }))
            .into_any_element()
    }

    /// The sidebar's first row: back to Home.
    pub(super) fn home_row(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("home-row")
            .flex()
            .items_center()
            .gap_2()
            .mx_2()
            .mt_3()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .when(self.home, |d| d.bg(theme::surface_hover()))
            .hover(|d| d.bg(theme::surface_hover()))
            .on_click(cx.listener(|this, _, _, cx| {
                this.go_home();
                cx.notify();
            }))
            .child(div().text_sm().text_color(theme::accent()).child("⌂"))
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Home"),
            )
            .child(muted(
                keymap::keys_for("show_home")
                    .map(|k| keymap::pretty(&k))
                    .unwrap_or_default(),
            ))
            .into_any_element()
    }

    /// Shows Home (⌘0).
    pub(super) fn go_home(&mut self) {
        if !self.home {
            self.list_cursor = None;
        }
        self.home = true;
        self.sidebar_nav = false;
    }

    pub(super) fn render_projects(&self, cx: &mut Context<Self>) -> AnyElement {
        let today = self.today_rows();
        let needs: Vec<(String, usize)> = self
            .repos
            .iter()
            .map(|r| {
                let name = r.entry.name();
                let n = today
                    .iter()
                    .filter(|row| row.section == Section::Needs && row.repo == name)
                    .count();
                (name, n)
            })
            .collect();
        let total_needs: usize = needs.iter().map(|(_, n)| n).sum();
        let cards: Vec<AnyElement> = (0..self.repos.len())
            .map(|i| self.project_card(i, needs[i].1, cx))
            .collect();
        let stats = system::stats();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_3()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Projects"),
                    )
                    .child(muted(format!(
                        "{} repos · {} live agent(s) · {} need you",
                        self.repos.len(),
                        self.live_agents(),
                        total_needs
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_4()
                    .child(two_columns(cards))
                    .child(self.render_computer(&stats)),
            )
            .into_any_element()
    }

    fn project_card(&self, index: usize, needs: usize, cx: &mut Context<Self>) -> AnyElement {
        let view = &self.repos[index];
        let name = view.entry.name();
        let highlighted = self.home && self.list_cursor == Some(index);
        let mut body = card()
            .when(highlighted, |c| c.border_color(theme::accent()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(repo_badge(&name, 22.))
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name.clone()),
                    )
                    .children(view.snapshot.as_ref().map(|s| status_pill(s.status()))),
            );
        let Some(snapshot) = &view.snapshot else {
            return body.child(muted("Checking…")).into_any_element();
        };
        if let Some(Ok(git)) = &snapshot.git {
            let mut line = vec![git.branch.clone()];
            if git.changed_files > 0 {
                line.push(format!("{} changed", git.changed_files));
            }
            if git.ahead > 0 || git.behind > 0 {
                line.push(format!("↑{} ↓{}", git.ahead, git.behind));
            }
            body = body.child(muted(line.join(" · ")));
            if !git.last_commit.is_empty() {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .overflow_hidden()
                        .child(git.last_commit.clone()),
                );
            }
        }

        let up = snapshot
            .processes
            .iter()
            .filter(|p| p.status.is_up())
            .count();
        let dots = snapshot.processes.iter().map(|p| {
            div()
                .size(px(7.))
                .rounded_full()
                .bg(theme::status(p.status))
        });
        let live = snapshot
            .sessions
            .iter()
            .filter(|s| s.state.is_live())
            .count();
        let waiting = snapshot
            .sessions
            .iter()
            .filter(|s| s.state == crate::sessions::SessionState::Idle)
            .count();
        let worktrees = match &snapshot.worktrees {
            Some(Ok(list)) => list.iter().filter(|w| !w.main).count(),
            _ => 0,
        };
        let metric = |title: &str, value: String| {
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(title.to_string()),
                )
                .child(div().text_sm().child(value))
        };
        body = body.child(
            div()
                .flex()
                .gap_4()
                .pt_1()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Processes"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(div().text_sm().child(if snapshot.processes.is_empty() {
                                    "—".to_string()
                                } else {
                                    format!("{up}/{}", snapshot.processes.len())
                                }))
                                .children(dots),
                        ),
                )
                .child(metric(
                    "Agents",
                    if waiting > 0 {
                        format!("{live} · {waiting} waiting")
                    } else {
                        live.to_string()
                    },
                ))
                .child(metric("Worktrees", worktrees.to_string())),
        );
        let mut tags = div().flex().flex_wrap().gap_1();
        if needs > 0 {
            tags = tags.child(pill(format!("{needs} need you"), theme::danger()));
        }
        if !snapshot.overlaps.is_empty() {
            tags = tags.child(pill(
                format!("{} overlap(s)", snapshot.overlaps.len()),
                theme::status(Status::Degraded),
            ));
        }
        if let Some(Ok(k)) = &view.knowledge {
            if k.chunks > 0 {
                tags = tags.child(pill(
                    format!("{} chunks", k.chunks),
                    theme::hue(theme::Hue::Yellow),
                ));
            }
            if k.requirements.failing > 0 {
                tags = tags.child(pill(
                    format!("{} failing", k.requirements.failing),
                    theme::danger(),
                ));
            }
        }
        body = body.child(tags);
        div()
            .id(("project-card", index))
            .w_full()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select(index);
                this.tab = Tab::Overview;
                cx.notify();
            }))
            .child(body)
            .into_any_element()
    }

    fn render_computer(&self, stats: &Stats) -> AnyElement {
        let column = div().w(px(340.)).flex_none().flex().flex_col().gap_3();
        if stats.at.is_none() {
            return column
                .child(card().child(label("COMPUTER")).child(muted("Measuring…")))
                .into_any_element();
        }
        let cpu_color = load_color(stats.cpu);
        let mem = stats.mem_percent();
        let mut column = column
            .child(
                card()
                    .child(label("COMPUTER"))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(stats.host.clone()),
                    )
                    .child(muted(format!("{} · {}", stats.os, stats.cpu_brand)))
                    .child(muted(format!("Up {}", system::duration(stats.uptime)))),
            )
            .child(
                card()
                    .child(stat_header("CPU", format!("{:.0}%", stats.cpu)))
                    .child(graph(&stats.cpu_history, cpu_color))
                    .child(div().flex().items_end().gap(px(2.)).h(px(16.)).children(
                        stats.cores.iter().map(|c| {
                            div()
                                .flex_1()
                                .h(px((c / 100.0 * 16.0).max(1.0)))
                                .rounded_sm()
                                .bg(load_color(*c).opacity(0.8))
                        }),
                    ))
                    .child(muted(format!(
                        "{} cores · load {:.2} {:.2} {:.2}",
                        stats.cores.len(),
                        stats.load.0,
                        stats.load.1,
                        stats.load.2
                    ))),
            )
            .child(
                card()
                    .child(stat_header("Memory", format!("{:.0}%", mem)))
                    .child(graph(&stats.mem_history, load_color(mem)))
                    .child(muted(format!(
                        "{} of {}{}",
                        bytes(stats.mem_used),
                        bytes(stats.mem_total),
                        if stats.swap_used > 0 {
                            format!(" · swap {}", bytes(stats.swap_used))
                        } else {
                            String::new()
                        }
                    ))),
            );
        if let Some(disk) = &stats.disk {
            let used = percent(disk.total.saturating_sub(disk.free), disk.total);
            let low = disk.free < LOW_DISK;
            column =
                column.child(
                    card()
                        .child(stat_header("Disk", format!("{} free", bytes(disk.free))))
                        .child(bar(
                            used,
                            if low {
                                theme::danger()
                            } else {
                                load_color(used)
                            },
                        ))
                        .child(muted(format!(
                            "{:.0}% of {} used · {}",
                            used,
                            bytes(disk.total),
                            disk.mount
                        )))
                        .when(low, |c| {
                            c.child(div().text_xs().text_color(theme::danger()).child(
                                "Low on space: builds and Postgres fail when the disk fills",
                            ))
                        }),
                );
        }
        let mut misc = card().child(label("POWER AND NETWORK"));
        if let Some(battery) = &stats.battery {
            misc = misc.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(80.)).child(bar(
                        battery.percent as f32,
                        if battery.percent < 20 {
                            theme::danger()
                        } else {
                            theme::status(Status::Running)
                        },
                    )))
                    .child(div().text_sm().child(format!("{}%", battery.percent)))
                    .child(muted(match &battery.remaining {
                        Some(left) => format!("{} · {left} left", battery.state),
                        None => battery.state.clone(),
                    })),
            );
        }
        misc = misc.child(muted(format!(
            "↓ {}/s   ↑ {}/s",
            bytes(stats.net_rx),
            bytes(stats.net_tx)
        )));
        column = column.child(misc);

        let proc_row = |p: &system::Proc| {
            div()
                .flex()
                .gap_2()
                .text_xs()
                .child(div().flex_1().overflow_hidden().child(p.name.clone()))
                .child(
                    div()
                        .w(px(48.))
                        .text_color(theme::text_muted())
                        .child(format!("{:.0}%", p.cpu)),
                )
                .child(
                    div()
                        .w(px(64.))
                        .text_color(theme::text_muted())
                        .child(bytes(p.memory)),
                )
        };
        column = column.child(
            card()
                .child(label("BUSIEST PROCESSES"))
                .children(stats.top_cpu.iter().map(proc_row))
                .child(div().h(px(1.)).bg(theme::border()))
                .child(label("MOST MEMORY"))
                .children(stats.top_memory.iter().map(proc_row)),
        );

        // Ports, named after the repo process that owns them when one does.
        let owner = |port: u16| {
            self.repos.iter().find_map(|r| {
                let s = r.snapshot.as_ref()?;
                s.processes
                    .iter()
                    .find(|p| p.spec.port == Some(port))
                    .map(|p| format!("{} › {}", r.entry.name(), p.spec.name))
            })
        };
        let ports = stats.ports.iter().map(|p| {
            let project = owner(p.port);
            div()
                .flex()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .w(px(52.))
                        .text_color(theme::accent())
                        .child(format!(":{}", p.port)),
                )
                .child(div().flex_1().overflow_hidden().child(p.command.clone()))
                .children(project.map(|name| div().text_color(theme::text_muted()).child(name)))
                .child(
                    div()
                        .w(px(56.))
                        .text_color(theme::text_muted())
                        .child(format!("pid {}", p.pid)),
                )
        });
        column = column.child(
            card()
                .child(label(SharedString::from(format!(
                    "LISTENING PORTS ({})",
                    stats.ports.len()
                ))))
                .children(ports),
        );
        column.into_any_element()
    }
}
