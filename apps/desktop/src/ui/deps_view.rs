//! Home → Dependencies: what every registered repo declares, side by side,
//! colored by how far each repo is behind the newest version any of them
//! uses (`src/deps.rs`). A row opens to the manifests that declare it, each
//! a click from the editor.

use std::time::{Duration, Instant};

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, loading, muted, pill, segment};
use super::*;
use crate::deps::{self, Align, Eco, Row};
use crate::theme::Hue;

/// Rows drawn at most (the filters narrow it down).
const DEPS_ROWS: usize = 300;
const NAME_W: f32 = 340.;
const CELL_W: f32 = 150.;

/// The scan of every repo, compared.
#[derive(Debug, Clone)]
pub struct DepsReport {
    pub repos: Vec<(String, PathBuf)>,
    pub rows: Vec<Row>,
    pub took: Duration,
}

/// Which rows the table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepsFilter {
    /// Different majors (breaking) only.
    #[default]
    Breaking,
    /// Breaking and minor drift.
    Misaligned,
    /// Everything used by two or more repos.
    Shared,
}

/// The table's view state.
#[derive(Default)]
pub struct DepsState {
    pub report: Option<Loadable<DepsReport>>,
    pub filter: DepsFilter,
    pub eco: Option<Eco>,
    pub open: Option<(Eco, String)>,
    pub scanning: bool,
}

fn lag_color(lag: Align) -> gpui::Hsla {
    match lag {
        Align::Breaking => theme::danger(),
        Align::Drift => theme::hue(Hue::Yellow),
        Align::Unknown => theme::text_muted(),
        Align::Aligned => theme::text(),
    }
}

impl Workspace {
    pub(super) fn scan_deps(&mut self, cx: &mut Context<Self>) {
        if self.deps.scanning {
            return;
        }
        self.deps.scanning = true;
        if !matches!(self.deps.report, Some(Loadable::Done(Ok(_)))) {
            self.deps.report = Some(Loadable::Loading);
        }
        let repos: Vec<(String, PathBuf)> = self
            .repos
            .iter()
            .map(|r| (r.entry.name(), r.entry.path.clone()))
            .collect();
        let work = cx.background_executor().spawn(async move {
            let started = Instant::now();
            let scans: Vec<Vec<deps::Decl>> = std::thread::scope(|scope| {
                let handles: Vec<_> = repos
                    .iter()
                    .map(|(_, path)| scope.spawn(move || deps::scan(path)))
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap_or_default())
                    .collect()
            });
            DepsReport {
                rows: deps::compare(&scans, 2),
                repos,
                took: started.elapsed(),
            }
        });
        cx.spawn(async move |this, cx| {
            let report = work.await;
            this.update(cx, |this, cx| {
                this.deps.scanning = false;
                this.deps.report = Some(Loadable::Done(Ok(report)));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_deps(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.deps.report.is_none() {
            self.scan_deps(cx);
        }
        let report = match &self.deps.report {
            Some(Loadable::Done(Ok(report))) => report.clone(),
            Some(Loadable::Done(Err(error))) => return muted(error.clone()).into_any_element(),
            _ => return loading("Reading every repo's manifests…").into_any_element(),
        };
        if report.repos.len() < 2 {
            return muted("Register two or more repos to compare their dependencies")
                .into_any_element();
        }
        let count = |f: &dyn Fn(&Row) -> bool| report.rows.iter().filter(|r| f(r)).count();
        let breaking = count(&|r| r.align == Align::Breaking);
        let misaligned = count(&|r| matches!(r.align, Align::Breaking | Align::Drift));

        // Filters: how misaligned, and which ecosystem.
        let filter = self.deps.filter;
        let chip = |id: &'static str, label: String, value: DepsFilter, cx: &mut Context<Self>| {
            segment(id, label, filter == value).on_click(cx.listener(move |this, _, _, cx| {
                this.deps.filter = value;
                cx.notify();
            }))
        };
        let eco = self.deps.eco;
        let eco_chip =
            |id: &'static str, label: &'static str, value: Option<Eco>, cx: &mut Context<Self>| {
                segment(id, label, eco == value).on_click(cx.listener(move |this, _, _, cx| {
                    this.deps.eco = value;
                    cx.notify();
                }))
            };
        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(chip(
                "deps-breaking",
                format!("Different majors ({breaking})"),
                DepsFilter::Breaking,
                cx,
            ))
            .child(chip(
                "deps-misaligned",
                format!("Any difference ({misaligned})"),
                DepsFilter::Misaligned,
                cx,
            ))
            .child(chip(
                "deps-shared",
                format!("All shared ({})", report.rows.len()),
                DepsFilter::Shared,
                cx,
            ))
            .child(div().w(px(12.)))
            .child(eco_chip("deps-eco-all", "All", None, cx))
            .child(eco_chip(
                "deps-eco-tool",
                "Toolchain",
                Some(Eco::Toolchain),
                cx,
            ))
            .child(eco_chip("deps-eco-npm", "npm", Some(Eco::Npm), cx))
            .child(eco_chip("deps-eco-cargo", "cargo", Some(Eco::Cargo), cx))
            .child(div().flex_1())
            .child(muted(format!(
                "{} repos · read in {} ms",
                report.repos.len(),
                report.took.as_millis()
            )))
            .child(
                button(
                    "deps-rescan",
                    if self.deps.scanning {
                        "Reading…"
                    } else {
                        "Rescan"
                    },
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.scan_deps(cx);
                    cx.notify();
                })),
            );

        let rows: Vec<&Row> = report
            .rows
            .iter()
            .filter(|r| match filter {
                DepsFilter::Breaking => r.align == Align::Breaking,
                DepsFilter::Misaligned => matches!(r.align, Align::Breaking | Align::Drift),
                DepsFilter::Shared => true,
            })
            .filter(|r| eco.is_none_or(|e| r.eco == e))
            .collect();
        let shown = rows.len().min(DEPS_ROWS);

        let header = div()
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .w(px(NAME_W))
                    .flex_none()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child("DEPENDENCY"),
            )
            .children(report.repos.iter().map(|(name, _)| {
                div()
                    .w(px(CELL_W))
                    .flex_none()
                    .truncate()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::name_color(name))
                    .child(name.clone())
            }));

        let open = self.deps.open.clone();
        let mut lines: Vec<AnyElement> = Vec::new();
        for (i, row) in rows.into_iter().take(DEPS_ROWS).enumerate() {
            let key = (row.eco, row.name.clone());
            let expanded = open.as_ref() == Some(&key);
            let badge = match row.align {
                Align::Breaking => Some(pill(row.align.label(), theme::danger())),
                Align::Drift => Some(pill(row.align.label(), theme::hue(Hue::Yellow))),
                Align::Unknown => Some(pill(row.align.label(), theme::text_muted())),
                Align::Aligned => None,
            };
            let cells = (0..report.repos.len()).map(|r| {
                let specs = row.specs(r);
                div()
                    .w(px(CELL_W))
                    .flex_none()
                    .pr_2()
                    .truncate()
                    .text_sm()
                    .text_color(if specs.is_empty() {
                        theme::text_muted().opacity(0.5)
                    } else {
                        lag_color(row.lag(r))
                    })
                    .child(if specs.is_empty() {
                        "—".to_string()
                    } else {
                        specs
                    })
            });
            lines.push(
                div()
                    .id(("dep", i))
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(expanded, |d| d.bg(theme::surface_hover()))
                    .hover(|d| d.bg(theme::surface_hover()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.deps.open = if this.deps.open.as_ref() == Some(&key) {
                            None
                        } else {
                            Some(key.clone())
                        };
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(NAME_W))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_2()
                            .pr_2()
                            .child(div().min_w_0().truncate().text_sm().child(row.name.clone()))
                            .child(muted(row.eco.label()))
                            .children(badge),
                    )
                    .children(cells)
                    .into_any_element(),
            );
            if expanded {
                lines.push(self.render_dep_sources(i, row, &report, cx));
            }
        }

        let legend = div()
            .flex()
            .gap_3()
            .child(div().text_xs().text_color(theme::danger()).child("■ a major behind the newest"))
            .child(
                div()
                    .text_xs()
                    .text_color(theme::hue(Hue::Yellow))
                    .child("■ a minor behind"),
            )
            .child(muted("Declared requirements (package.json, Cargo.toml, toolchain pins), not lockfiles. Click a row for its files."));

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(toolbar)
            .child(legend)
            .child(
                div()
                    .id("deps-table")
                    .overflow_x_scroll()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::surface())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(NAME_W + CELL_W * report.repos.len() as f32 + 24.))
                            .child(header)
                            .children(if lines.is_empty() {
                                vec![
                                    div()
                                        .px_3()
                                        .py_3()
                                        .child(muted("Nothing here: these repos agree"))
                                        .into_any_element(),
                                ]
                            } else {
                                lines
                            }),
                    ),
            )
            .when(
                shown < report.rows.len() && filter == DepsFilter::Shared,
                |d| {
                    d.child(muted(format!(
                        "Showing {shown}; narrow it with the filters"
                    )))
                },
            )
            .into_any_element()
    }

    /// Under an opened row: each repo's manifests that declare it.
    fn render_dep_sources(
        &self,
        i: usize,
        row: &Row,
        report: &DepsReport,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut list = div().flex().flex_col().gap_0p5().pl(px(24.)).pr_3().pb_2();
        for (r, decls) in row.per_repo.iter().enumerate() {
            if decls.is_empty() {
                continue;
            }
            let (repo, root) = &report.repos[r];
            for (d, decl) in decls.iter().enumerate() {
                let path = root.join(&decl.file);
                list = list.child(
                    div()
                        .id(SharedString::from(format!("dep-src-{i}-{r}-{d}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .hover(|d| d.text_color(theme::accent()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.run(Command::OpenEditorAt(path.clone()), cx)
                        }))
                        .child(
                            div()
                                .w(px(130.))
                                .truncate()
                                .text_xs()
                                .text_color(theme::name_color(repo))
                                .child(repo.clone()),
                        )
                        .child(div().text_xs().child(decl.file.display().to_string()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(lag_color(row.lag(r)))
                                .child(decl.spec.clone()),
                        )
                        .when(decl.dev, |d| d.child(muted("dev"))),
                );
            }
        }
        list.into_any_element()
    }
}
