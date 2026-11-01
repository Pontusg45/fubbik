//! fubbik actions: API calls, agent launches, setup fixes, on-demand scans.
//! Each runs on the background executor and refreshes what it changed.

use std::path::PathBuf;

use gpui::{Context, FontWeight, IntoElement, div, prelude::*, px};

use super::palette::Command;
use super::widgets::button;
use super::{BranchContext, Loadable, Panel, Workspace};
use crate::fubbik::FubbikClient;
use crate::sessions::AiSession;
use crate::{agent_task, claude_md, gaps, launch, setup, theme, worktree};

/// Files sent to the per-file lookup for a branch (one request each).
const LINKED_FILE_LIMIT: usize = 40;
/// A branch this many commits behind main gets a warning in its review.
const FAR_BEHIND: usize = 50;
/// Files sent to the `for-files` context lookup.
const CONTEXT_FILE_LIMIT: usize = 60;

impl Workspace {
    /// Runs an API call off the main thread, then refreshes knowledge.
    pub(super) fn api_action(
        &mut self,
        done: impl Into<String> + Send + 'static,
        work: impl FnOnce(&FubbikClient) -> Result<(), String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let client = self.client.clone();
        let done: String = done.into();
        let task = cx.background_executor().spawn(async move { work(&client) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.show_info(done),
                    Err(error) => this.show_error(error),
                }
                this.poll_now(true, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn selected_space(&self) -> Option<String> {
        self.repos.get(self.selected).and_then(|r| r.space_id())
    }

    pub(super) fn search_chunks(&mut self, cx: &mut Context<Self>) {
        let query = self.palette_field.text.clone();
        self.palette_search_error = None;
        let Some(terms) = query.strip_prefix('?').map(str::trim) else {
            self.palette_search.clear();
            self.palette_searching = false;
            return;
        };
        if terms.chars().count() < 2 {
            self.palette_search.clear();
            self.palette_searching = false;
            return;
        }
        self.palette_searching = true;
        let client = self.client.clone();
        let space = self.selected_space();
        let terms = terms.to_string();
        let task = cx
            .background_executor()
            .spawn(async move { client.search(&terms, space.as_deref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                // Drop results for a query the user has typed past.
                if this.palette_field.text == query {
                    this.palette_searching = false;
                    match result {
                        Ok(found) => this.palette_search = found,
                        Err(error) => {
                            this.palette_search.clear();
                            this.palette_search_error = Some(error.to_string());
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn start_task(
        &mut self,
        repo: usize,
        plan_id: &str,
        task_id: &str,
        new_worktree: bool,
        background: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let found = view
            .knowledge
            .as_ref()
            .and_then(|k| k.as_ref().ok())
            .and_then(|k| k.plan_details.iter().find(|p| p.id == plan_id))
            .and_then(|plan| {
                Some((
                    plan.clone(),
                    plan.tasks.iter().find(|t| t.id == task_id)?.clone(),
                ))
            });
        let Some((plan, task)) = found else {
            self.show_error("That task is no longer in the plan");
            return;
        };
        let client = self.client.clone();
        let path = view.entry.path.clone();
        self.show_info(format!("Starting Claude on “{}”…", task.title));
        self.refresh_worktrees = true;
        let work = cx.background_executor().spawn(async move {
            agent_task::start(&client, &path, &plan, &task, new_worktree).map(|s| (s, task.title))
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok((started, title)) => {
                        let place = started
                            .cwd
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        this.show_info(format!("Claude is on “{title}” in {place}"));
                        // The new worktree must be in the snapshot for the tab to show under this repo.
                        this.refresh_worktrees = true;
                        this.launch_agent(
                            started.cwd,
                            format!("Task: {title}"),
                            started.command,
                            background,
                            cx,
                        );
                    }
                    Err(error) => this.show_error(format!("Could not start the task: {error}")),
                }
                this.poll_now(true, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn dismiss_stale(&mut self, flag_id: String, cx: &mut Context<Self>) {
        self.api_action("Dismissed", move |c| c.dismiss_stale(&flag_id), cx);
    }

    pub(super) fn review(&mut self, proposal_id: String, approve: bool, cx: &mut Context<Self>) {
        let done = if approve {
            "Approved — the chunk is updated"
        } else {
            "Rejected"
        };
        self.api_action(done, move |c| c.review_proposal(&proposal_id, approve), cx);
    }

    pub(super) fn enrich_all(&mut self, cx: &mut Context<Self>) {
        if self.enriching.is_some() {
            return;
        }
        self.enriching = Some(std::time::Instant::now());
        self.show_info("Enriching… this can take minutes");
        let client = self.client.clone();
        let task = cx
            .background_executor()
            .spawn(async move { client.enrich_all() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.enriching = None;
                match result {
                    Ok(n) => this.show_info(format!("Enriched {n} chunks")),
                    Err(error) => {
                        this.show_error(format!("Enrichment failed (is Ollama running?): {error}"))
                    }
                }
                this.poll_now(true, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn toggle_feature(
        &mut self,
        repo: usize,
        feature_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(Ok(knowledge)) = self.repos.get(repo).and_then(|r| r.knowledge.as_ref()) else {
            return;
        };
        let mut active = knowledge.active_features.clone();
        if let Some(i) = active.iter().position(|id| *id == feature_id) {
            active.remove(i);
        } else {
            active.push(feature_id);
        }
        self.api_action(
            "Active features updated",
            move |c| c.set_active_features(&active),
            cx,
        );
    }

    pub(super) fn set_requirement(
        &mut self,
        id: String,
        status: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.api_action(
            format!("Marked {status}"),
            move |c| c.set_requirement_status(&id, status),
            cx,
        );
    }

    pub(super) fn save_session_note(
        &mut self,
        repo: usize,
        session: AiSession,
        cx: &mut Context<Self>,
    ) {
        let Some(space) = self.repos.get(repo).and_then(|r| r.space_id()) else {
            self.show_error("This repo has no fubbik space");
            return;
        };
        let title = format!("Session: {}", session.title);
        let content = format!(
            "{} session `{}`\n\n- Directory: {}\n- Branch: {}\n- Resume: `{}`\n",
            session.agent.label(),
            session.id,
            session.cwd.display(),
            session.branch.as_deref().unwrap_or("—"),
            session.agent.resume_command(&session.id),
        );
        self.api_action(
            "Session saved as a note",
            move |c| c.quick_note(&space, &title, &content),
            cx,
        );
    }

    // ── setup ──────────────────────────────────────────────────────────

    pub(super) fn sync_claude_md(&mut self, repo: usize, root: PathBuf, cx: &mut Context<Self>) {
        let Some(sync) = self
            .repos
            .get(repo)
            .and_then(|r| r.setup.as_ref())
            .and_then(|s| s.claude_md.clone())
        else {
            return;
        };
        let Ok(generated) = sync.generated else {
            self.show_error("fubbik could not generate CLAUDE.md");
            return;
        };
        self.run_background(cx, "CLAUDE.md written", move || {
            claude_md::write(&root, &sync.config, &generated).map(|_| ())
        });
        self.poll_now(true, cx);
    }

    pub(super) fn install_hook(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.run_background(cx, "Pre-commit hook installed", move || {
            setup::install_hook(&root)
        });
        self.poll_now(true, cx);
    }

    pub(super) fn install_mcp(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(bin) = self.global.as_ref().and_then(|g| g.fubbik_bin.clone()) else {
            self.show_error("Build the Rust fubbik binary first");
            return;
        };
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let path = view.entry.path.clone();
        let url = self.config.fubbik_url();
        self.run_background(
            cx,
            "Added fubbik to .mcp.json — restart Claude sessions to load it",
            move || setup::install_mcp(&path, &bin, &url).map(|_| ()),
        );
        self.poll_now(true, cx);
    }

    /// `cargo build --release -p fubbik`, as a logged task in that checkout.
    pub(super) fn build_fubbik(&mut self, checkout: PathBuf, cx: &mut Context<Self>) {
        let key = crate::model::process_key(&checkout, "build-fubbik");
        self.log_key = Some(key.clone());
        self.user_stopped.insert(key.clone());
        let supervisor = self.supervisor.clone();
        self.run_background(cx, "Building fubbik — see Logs", move || {
            supervisor
                .start(&key, "cargo build --release -p fubbik", &checkout)
                .map(|_| ())
        });
    }

    // ── on-demand scans ────────────────────────────────────────────────

    pub(super) fn load_branch_context(
        &mut self,
        repo: usize,
        root: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.repos.get_mut(repo) else {
            return;
        };
        if self.expanded_worktree.as_ref() == Some(&root) && view.branch_context.contains_key(&root)
        {
            self.expanded_worktree = None; // second click collapses
            return;
        }
        self.expanded_worktree = Some(root.clone());
        view.branch_context.insert(root.clone(), Loadable::Loading);
        let main_branch = view
            .snapshot
            .as_ref()
            .and_then(|s| s.worktrees.as_ref()?.as_ref().ok()?.first()?.branch.clone());
        let space = view.space_id();
        let client = self.client.clone();
        let key = root.clone();
        let task = cx.background_executor().spawn(async move {
            let files = worktree::changed_paths(&root, main_branch.as_deref());
            let sections = client.context_for_files(
                &files[..files.len().min(CONTEXT_FILE_LIMIT)],
                space.as_deref(),
            )?;
            let mut linked: Vec<(crate::fubbik::ChunkRef, Vec<String>)> = Vec::new();
            for file in files.iter().take(LINKED_FILE_LIMIT) {
                for chunk in client.chunks_for_file(file, space.as_deref())? {
                    if !matches!(
                        chunk.match_reason.as_deref(),
                        Some("file-ref" | "applies-to")
                    ) {
                        continue;
                    }
                    match linked.iter_mut().find(|(c, _)| c.id == chunk.id) {
                        Some((_, matched)) => matched.push(file.clone()),
                        None => linked.push((chunk, vec![file.clone()])),
                    }
                }
            }
            Ok(BranchContext {
                files,
                sections,
                linked,
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.repos.get_mut(repo) {
                    view.branch_context.insert(key, Loadable::Done(result));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn scan_gaps(&mut self, repo: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get_mut(repo) else {
            return;
        };
        view.gaps = Some(Loadable::Loading);
        let client = self.client.clone();
        let root = view.entry.path.clone();
        let space = view.space_id();
        let task = cx
            .background_executor()
            .spawn(async move { gaps::scan(&client, &root, space.as_deref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.repos.get_mut(repo) {
                    view.gaps = Some(Loadable::Done(result));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A file dropped on the window: its fubbik context in a panel.
    pub(super) fn show_file_context(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        let owner = std::iter::once(self.selected)
            .chain(0..self.repos.len())
            .find(|&i| {
                self.repos
                    .get(i)
                    .is_some_and(|r| path.starts_with(&r.entry.path))
            });
        let Some(owner) = owner else {
            self.show_error("That file is not inside a registered repo");
            return;
        };
        let view = &self.repos[owner];
        let relative = path
            .strip_prefix(&view.entry.path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.panel = Some(Panel {
            title: format!("Why {relative} is the way it is"),
            body: Loadable::Loading,
            actions: Vec::new(),
        });
        let client = self.client.clone();
        let space = view.space_id();
        let task = cx.background_executor().spawn(async move {
            let text = client.context_for_file_md(&relative, space.as_deref())?;
            Ok(if text.trim().is_empty() {
                format!("No chunks relate to {relative} yet.")
            } else {
                text
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(panel) = &mut this.panel {
                    panel.body = Loadable::Done(result);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Test processes (`[process.report]`) that finished: report pass/fail.
    pub(super) fn report_test_exits(&mut self, cx: &mut Context<Self>) {
        for (key, code) in self.supervisor.take_exits() {
            self.last_exit
                .insert(key.clone(), (code, std::time::Instant::now()));
            self.pending.remove(&key);
            let found = self.repos.iter().find_map(|view| {
                let process = view
                    .snapshot
                    .as_ref()?
                    .processes
                    .iter()
                    .find(|p| p.key == key)?;
                Some((view.entry.name(), process.spec.clone()))
            });
            let Some((repo_name, spec)) = found else {
                continue;
            };
            let Some(report) = spec.report.clone() else {
                continue;
            };
            let pass = code == Some(0);
            let verdict = if pass { "passed" } else { "failed" };
            launch::notify(&repo_name, &format!("{} {verdict}", spec.name));
            let detail = match code {
                Some(code) => format!("exit code {code}"),
                None => "killed by a signal".into(),
            };
            let test_ref = report.test_ref.clone().unwrap_or(spec.command.clone());
            self.api_action(
                format!("{} {verdict} — reported to fubbik", spec.name),
                move |c| {
                    if let Some(requirement) = &report.requirement {
                        c.set_requirement_status(
                            requirement,
                            if pass { "passing" } else { "failing" },
                        )?;
                    }
                    if let (Some(matrix), Some(cell)) = (&report.matrix, &report.cell) {
                        c.report_test(matrix, cell, &test_ref, pass, &detail)?;
                    }
                    Ok(())
                },
                cx,
            );
        }
    }

    pub(super) fn render_panel(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let panel = self.panel.as_ref()?;
        let body = match &panel.body {
            Loadable::Loading => super::widgets::loading("Loading…").into_any_element(),
            Loadable::Done(Err(error)) => div()
                .text_sm()
                .text_color(theme::danger())
                .child(error.clone())
                .into_any_element(),
            Loadable::Done(Ok(text)) => div()
                .flex()
                .flex_col()
                .font_family("Menlo")
                .text_xs()
                .children(text.lines().map(|line| {
                    // Diffs read better colored; other text is unaffected.
                    let color = if line.starts_with("+++") || line.starts_with("---") {
                        theme::text_muted()
                    } else if line.starts_with('+') {
                        theme::status(crate::model::Status::Running)
                    } else if line.starts_with('-') {
                        theme::danger()
                    } else if line.starts_with("@@") {
                        theme::accent()
                    } else {
                        theme::text()
                    };
                    div()
                        .whitespace_nowrap()
                        .text_color(color)
                        .child(line.to_string())
                }))
                .into_any_element(),
        };
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .justify_center()
                .pt(px(70.))
                .bg(theme::overlay())
                .child(
                    div()
                        .id("panel")
                        // Sized to the window, within reason.
                        .w(gpui::relative(0.8))
                        .max_w(px(1100.))
                        .h(gpui::relative(0.78))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::elevated())
                        .shadow(theme::frame_shadow())
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.panel = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(panel.title.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_1()
                                        .children(panel.actions.iter().enumerate().map(
                                            |(i, (label, command))| {
                                                let command = command.clone();
                                                button(("panel-action", i), label.clone()).on_click(
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.panel = None;
                                                        this.run(command.clone(), cx);
                                                    }),
                                                )
                                            },
                                        ))
                                        .child(button("panel-close", "Close  esc").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.panel = None;
                                                cx.notify();
                                            }),
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .id("panel-body")
                                .flex_1()
                                .overflow_scroll()
                                .child(body),
                        ),
                ),
        )
    }

    pub(super) fn toggle_reply(&mut self, pane: crate::tmux::Pane, window: &mut gpui::Window) {
        let same = self
            .reply_target
            .as_ref()
            .is_some_and(|t| t.id == pane.id && t.server == pane.server);
        self.reply_field.text.clear();
        if same {
            self.reply_target = None;
            self.focus.focus(window);
        } else {
            self.reply_target = Some(pane);
            self.reply_field.focus.focus(window);
        }
    }

    /// Types the reply into the agent's pane and presses Enter.
    pub(super) fn send_reply(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.reply_target.clone() else {
            return;
        };
        let text = self.reply_field.text.clone();
        if text.trim().is_empty() {
            return;
        }
        // Cleared only once it reached the agent; a failed send keeps it.
        self.run_background_then(
            cx,
            "Sent",
            move || crate::tmux::send(&pane, &text),
            |this, ok| {
                if ok {
                    this.reply_field.text.clear();
                }
            },
        );
    }

    /// Shows an agent's pane: its tab when it is one of the app's sessions,
    /// else a new tab attached to the user's own tmux session.
    pub(super) fn open_pane(
        &mut self,
        pane: crate::tmux::Pane,
        cwd: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if pane.server.is_fubbik()
            && let Some(tab) = self
                .terminals
                .iter()
                .find(|t| t.tmux.as_deref() == Some(pane.session.as_str()))
        {
            self.active_terminal = Some(tab.id);
        } else {
            let label = format!("tmux {}", pane.session);
            let command = format!("tmux attach -t '={}'", pane.session.replace('\'', ""));
            self.open_terminal_with(cwd, label, Some(command), false, true, cx);
        }
        self.tab = super::Tab::Terminal;
        self.focus_terminal = true;
    }

    /// A worktree's branch against main, in the panel, with "Open PR" and
    /// "Merge into main" (both run in a terminal tab, where their prompts
    /// and output can be seen).
    pub(super) fn review_worktree(&mut self, repo: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let worktrees = view
            .snapshot
            .as_ref()
            .and_then(|s| s.worktrees.as_ref()?.as_ref().ok().cloned())
            .unwrap_or_default();
        let (Some(main), Some(this)) = (
            worktrees.iter().find(|w| w.main),
            worktrees.iter().find(|w| w.path == path),
        ) else {
            self.show_error("That worktree is no longer listed");
            return;
        };
        let (Some(main_branch), Some(branch)) = (main.branch.clone(), this.branch.clone()) else {
            self.show_error("Review needs both the main worktree and this one on a branch");
            return;
        };
        let main_path = main.path.clone();
        self.panel = Some(Panel {
            title: if this.behind > FAR_BEHIND {
                format!(
                    "{branch} → {main_branch}  ⚠ {} commits behind {main_branch}: merge or rebase it first",
                    this.behind
                )
            } else {
                format!("{branch} → {main_branch}")
            },
            body: Loadable::Loading,
            actions: vec![
                (
                    "Open PR".into(),
                    Command::RunIn {
                        dir: path.clone(),
                        label: format!("PR {branch}"),
                        command: "gh pr create --fill".into(),
                    },
                ),
                (
                    format!("Merge into {main_branch}…"),
                    Command::AskFirst(Box::new(super::palette::Confirm {
                        title: format!("Merge {branch} into {main_branch}?"),
                        detail: if this.behind > FAR_BEHIND {
                            format!(
                                "{branch} is {} commits behind {main_branch}: expect conflicts (rebase it first). Runs git merge --no-ff in the main checkout, in a terminal tab.",
                                this.behind
                            )
                        } else {
                            format!(
                                "Runs git merge --no-ff in the main checkout ({}), in a terminal tab.",
                                super::display_path(&main_path)
                            )
                        },
                        action: "Merge".into(),
                        danger: this.behind > FAR_BEHIND,
                        command: Command::RunIn {
                            dir: main_path,
                            label: format!("Merge {branch}"),
                            command: format!("git merge --no-ff '{}'", branch.replace('\'', "")),
                        },
                    })),
                ),
            ],
        });
        let task = cx
            .background_executor()
            .spawn(async move { worktree::review(&path, &main_branch) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(panel) = &mut this.panel {
                    panel.body = Loadable::Done(result);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
