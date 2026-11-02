//! The Tests tab: each runner a repo has (cargo nextest or cargo test,
//! vitest), run in the background; the failing tests with what they printed,
//! "Open" at the failing line, "Rerun failed", and "Send to agent".

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, disabled_button, loading, muted, pill, section};
use super::*;
use crate::test_runs::{self, Report, Runner};

/// One runner's state in one repo.
#[derive(Default)]
pub struct RunState {
    pub running: Option<Instant>,
    pub report: Option<Report>,
    pub finished: Option<Instant>,
    /// The last run was a rerun of the failures only.
    pub partial: bool,
}

#[derive(Default)]
pub struct TestsState {
    /// Runners per repo (detecting asks cargo for nextest: done once).
    pub runners: HashMap<PathBuf, Loadable<Vec<Runner>>>,
    pub runs: HashMap<(PathBuf, Runner), RunState>,
}

impl Workspace {
    fn detect_runners(&mut self, repo: PathBuf, cx: &mut Context<Self>) {
        self.tests.runners.insert(repo.clone(), Loadable::Loading);
        let work = {
            let repo = repo.clone();
            cx.background_executor()
                .spawn(async move { test_runs::detect(&repo) })
        };
        cx.spawn(async move |this, cx| {
            let runners = work.await;
            this.update(cx, |this, cx| {
                this.tests.runners.insert(repo, Loadable::Done(Ok(runners)));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Runs a runner (all tests, or `only` these failures) in the background.
    pub(super) fn run_tests(
        &mut self,
        repo: PathBuf,
        runner: Runner,
        only: Vec<test_runs::Failure>,
        cx: &mut Context<Self>,
    ) {
        let state = self.tests.runs.entry((repo.clone(), runner)).or_default();
        if state.running.is_some() {
            return;
        }
        state.running = Some(Instant::now());
        let partial = !only.is_empty();
        let work = {
            let repo = repo.clone();
            cx.background_executor()
                .spawn(async move { test_runs::run(runner, &repo, &only) })
        };
        cx.spawn(async move |this, cx| {
            let report = work.await;
            this.update(cx, |this, cx| {
                let name = this
                    .repos
                    .iter()
                    .find(|r| r.entry.path == repo)
                    .map(|r| r.entry.name())
                    .unwrap_or_default();
                let summary = match (&report.error, report.failed) {
                    (Some(_), _) => format!("{} did not run", runner.label()),
                    (None, 0) => format!("{}: {} passed", runner.label(), report.passed),
                    (None, n) => {
                        format!("{}: {n} failed, {} passed", runner.label(), report.passed)
                    }
                };
                crate::mcp_events::record(
                    Some(&name),
                    if report.failed > 0 || report.error.is_some() {
                        "tests_failed"
                    } else {
                        "tests_passed"
                    },
                    summary.clone(),
                );
                if report.failed > 0 || report.error.is_some() {
                    this.show_error(format!("{name}: {summary}"));
                } else {
                    this.show_info(format!("{name}: {summary}"));
                }
                let state = this.tests.runs.entry((repo, runner)).or_default();
                state.running = None;
                state.finished = Some(Instant::now());
                state.partial = partial;
                state.report = Some(report);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Starts an agent in the repo with the failures as its prompt.
    fn send_failures_to_agent(&mut self, repo: PathBuf, runner: Runner, cx: &mut Context<Self>) {
        let Some(report) = self
            .tests
            .runs
            .get(&(repo.clone(), runner))
            .and_then(|s| s.report.clone())
        else {
            return;
        };
        let name = repo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let prompt = test_runs::fix_prompt(runner, &name, &report);
        let session = crate::agent_task::uuid_v4();
        let file = std::env::temp_dir().join(format!("fubbik-tests-{session}.md"));
        if let Err(error) = std::fs::write(&file, prompt) {
            self.show_error(error.to_string());
            return;
        }
        let command = format!(
            "claude --session-id {session} \"$(cat '{}')\"",
            file.display()
        );
        self.launch_agent(repo, format!("fix tests — {name}"), command, false, cx);
    }

    /// Repos whose last full run has failing tests (for Today).
    pub(super) fn failing_test_runs(&self) -> Vec<(PathBuf, Runner, usize)> {
        self.tests
            .runs
            .iter()
            .filter_map(|((repo, runner), state)| {
                let report = state.report.as_ref()?;
                (report.failed > 0 && !state.partial)
                    .then(|| (repo.clone(), *runner, report.failed))
            })
            .collect()
    }

    pub(super) fn render_tests(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let repo = self.repos[index].entry.path.clone();
        if !self.tests.runners.contains_key(&repo) {
            self.detect_runners(repo.clone(), cx);
        }
        let runners = match self.tests.runners.get(&repo) {
            Some(Loadable::Done(Ok(runners))) => runners.clone(),
            _ => return loading("Finding the test runners…").into_any_element(),
        };
        if runners.is_empty() {
            return muted("No test runner found: no Cargo.toml, and no vitest in package.json")
                .into_any_element();
        }
        let cards: Vec<AnyElement> = runners
            .into_iter()
            .map(|runner| self.render_runner(index, &repo, runner, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .children(cards)
            .into_any_element()
    }

    fn render_runner(
        &self,
        index: usize,
        repo: &PathBuf,
        runner: Runner,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.tests.runs.get(&(repo.clone(), runner));
        let running = state.and_then(|s| s.running);
        let report = state.and_then(|s| s.report.clone());
        let id =
            |what: &str| SharedString::from(format!("tests-{what}-{index}-{}", runner.label()));
        let (repo_a, repo_b, repo_c) = (repo.clone(), repo.clone(), repo.clone());
        let mut header = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(runner.label()),
            )
            .child(div().flex_1())
            .child(match running {
                Some(started) => disabled_button(
                    id("run"),
                    format!("Running… {}s", started.elapsed().as_secs()),
                ),
                None => {
                    button(id("run"), "Run all").on_click(cx.listener(move |this, _, _, cx| {
                        this.run_tests(repo_a.clone(), runner, Vec::new(), cx);
                        cx.notify();
                    }))
                }
            });
        if let Some(r) = &report
            && r.failed > 0
            && running.is_none()
        {
            let failures = r.failures.clone();
            header = header
                .child(
                    button(id("rerun"), format!("Rerun {} failed", r.failures.len())).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.run_tests(repo_b.clone(), runner, failures.clone(), cx);
                            cx.notify();
                        }),
                    ),
                )
                .child(button(id("agent"), "Send to agent").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.send_failures_to_agent(repo_c.clone(), runner, cx);
                        cx.notify();
                    },
                )));
        }
        let summary: AnyElement = match (&report, state.and_then(|s| s.finished)) {
            (None, _) => muted("Not run yet in this session").into_any_element(),
            (Some(r), finished) => {
                let ago = finished
                    .map(|f| format!(" · {}s ago", f.elapsed().as_secs()))
                    .unwrap_or_default();
                let partial = if state.is_some_and(|s| s.partial) {
                    " (the failures only)"
                } else {
                    ""
                };
                match (&r.error, r.failed) {
                    (Some(_), _) => div()
                        .flex()
                        .gap_2()
                        .child(pill("did not run", theme::danger()))
                        .child(muted(format!("see below{ago}")))
                        .into_any_element(),
                    (None, 0) => div()
                        .flex()
                        .gap_2()
                        .child(pill(
                            format!("{} passed", r.passed),
                            theme::status(Status::Running),
                        ))
                        .child(muted(format!("{} ignored{partial}{ago}", r.ignored)))
                        .into_any_element(),
                    (None, n) => div()
                        .flex()
                        .gap_2()
                        .child(pill(format!("{n} failed"), theme::danger()))
                        .child(muted(format!(
                            "{} passed · {} ignored{partial}{ago}",
                            r.passed, r.ignored
                        )))
                        .into_any_element(),
                }
            }
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(r) = &report {
            if let Some(error) = &r.error {
                rows.push(
                    div()
                        .px_3()
                        .py_2()
                        .font_family("Menlo")
                        .text_xs()
                        .text_color(theme::danger())
                        .child(error.clone())
                        .into_any_element(),
                );
            }
            for (k, failure) in r.failures.iter().enumerate() {
                let open = failure.location().map(|(path, line)| {
                    let file = if std::path::Path::new(&path).is_absolute() {
                        PathBuf::from(&path)
                    } else {
                        repo.join(&path)
                    };
                    button(
                        SharedString::from(format!("test-open-{index}-{k}")),
                        format!(
                            "Open {}:{line}",
                            file.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run(Command::OpenAtLine(file.clone(), line), cx)
                    }))
                });
                let message: String = failure
                    .message
                    .lines()
                    .take(8)
                    .collect::<Vec<_>>()
                    .join("\n");
                rows.push(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().size(px(8.)).rounded_full().bg(theme::danger()))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_sm()
                                        .child(failure.name.clone()),
                                )
                                .children(open),
                        )
                        .when(!message.is_empty(), |d| {
                            d.child(
                                div()
                                    .pl_4()
                                    .font_family("Menlo")
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child(message),
                            )
                        })
                        .into_any_element(),
                );
            }
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(header)
            .child(summary)
            .when(!rows.is_empty(), |d| d.child(section("Failures", rows)))
            .into_any_element()
    }
}
