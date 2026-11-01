//! Plan tasks as work items: moving them across the board, ticking their
//! acceptance criteria, their timeline, what they cost, and closing them
//! when their branch lands on main.

use super::*;
use crate::activity;
use std::path::Path;

use crate::fubbik::{PlanDetail, Task};

/// What a task has cost so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effort {
    pub sessions: usize,
    pub tokens: u64,
    /// When work started (its claim, else its last update while in progress).
    pub since: Option<i64>,
}

/// Task statuses the API knows.
const TASK_STATUSES: [&str; 5] = ["pending", "in_progress", "blocked", "done", "skipped"];

impl Workspace {
    pub(super) fn find_task(
        &self,
        repo: usize,
        plan_id: &str,
        task_id: &str,
    ) -> Option<(PlanDetail, Task)> {
        let knowledge = self.repos.get(repo)?.knowledge.as_ref()?.as_ref().ok()?;
        let plan = knowledge.plan_details.iter().find(|p| p.id == plan_id)?;
        let task = plan.tasks.iter().find(|t| t.id == task_id)?;
        Some((plan.clone(), task.clone()))
    }

    /// The board run holding a task, and when it claimed it.
    pub(super) fn task_holder(
        &self,
        repo: usize,
        plan_id: &str,
        task_id: &str,
    ) -> Option<(crate::fubbik::Run, Option<String>)> {
        let knowledge = self.repos.get(repo)?.knowledge.as_ref()?.as_ref().ok()?;
        knowledge
            .boards
            .iter()
            .filter(|b| b.plan_id == plan_id)
            .find_map(|board| {
                let claim = board
                    .claims
                    .iter()
                    .find(|c| c.task_id == task_id && !c.expired)?;
                let run = board.runs.iter().find(|r| r.id == claim.run_id)?;
                Some((run.clone(), claim.claimed_at.clone()))
            })
    }

    /// Sessions that worked on a task: the one holding it, and any in its
    /// worktree.
    pub(super) fn task_effort(&self, repo: usize, plan_id: &str, task: &Task) -> Effort {
        let holder = self.task_holder(repo, plan_id, &task.id);
        let holder_session = holder
            .as_ref()
            .and_then(|(run, _)| run.session_id().map(str::to_string));
        let sessions: Vec<&crate::sessions::AiSession> = self
            .repos
            .get(repo)
            .and_then(|r| r.snapshot.as_ref())
            .map(|s| {
                s.sessions
                    .iter()
                    .filter(|s| {
                        holder_session.as_deref() == Some(s.id.as_str())
                            || task
                                .link
                                .worktree
                                .as_deref()
                                .is_some_and(|w| s.cwd == Path::new(w))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let since = holder
            .and_then(|(_, at)| at)
            .or_else(|| {
                (task.status == "in_progress")
                    .then(|| task.updated_at.clone())
                    .flatten()
            })
            .and_then(|at| activity::parse_rfc3339(&at));
        Effort {
            sessions: sessions.len(),
            tokens: sessions.iter().filter_map(|s| s.tokens).sum(),
            since,
        }
    }

    /// Moves a task to another column (status). The user deciding needs no
    /// board claim.
    pub(super) fn set_task_status(
        &mut self,
        plan: String,
        task: String,
        status: &'static str,
        cx: &mut Context<Self>,
    ) {
        // Where it was, so the toast can undo the move.
        let previous: Option<&'static str> = self
            .repos
            .iter()
            .filter_map(|r| r.knowledge.as_ref()?.as_ref().ok())
            .flat_map(|k| k.plan_details.iter())
            .find(|p| p.id == plan)
            .and_then(|p| p.tasks.iter().find(|t| t.id == task))
            .and_then(|t| TASK_STATUSES.iter().copied().find(|s| *s == t.status))
            .filter(|s| *s != status);
        let client = self.client.clone();
        let (undo_plan, undo_task) = (plan.clone(), task.clone());
        let work = cx
            .background_executor()
            .spawn(async move { client.set_task_status(&plan, &task, status) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match (result, previous) {
                    (Ok(()), Some(previous)) => this.show_alert(Alert {
                        message: format!("Moved to {} — click to undo", status.replace('_', " ")),
                        target: Command::SetTaskStatus {
                            plan: undo_plan.clone(),
                            task: undo_task.clone(),
                            status: previous,
                        },
                    }),
                    (Ok(()), None) => {
                        this.show_info(format!("Moved to {}", status.replace('_', " ")))
                    }
                    (Err(error), _) => this.show_error(error),
                }
                this.poll_now(true, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn toggle_criterion(
        &mut self,
        repo: usize,
        plan: String,
        task_id: String,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some((_, task)) = self.find_task(repo, &plan, &task_id) else {
            return;
        };
        let mut criteria = task.acceptance.clone();
        let Some(criterion) = criteria.get_mut(index) else {
            return;
        };
        criterion.1 = !criterion.1;
        // Show it at once; the next knowledge poll confirms it.
        if let Some(Ok(knowledge)) = self.repos.get_mut(repo).and_then(|r| r.knowledge.as_mut())
            && let Some(t) = knowledge
                .plan_details
                .iter_mut()
                .find(|p| p.id == plan)
                .and_then(|p| p.tasks.iter_mut().find(|t| t.id == task_id))
        {
            t.acceptance = criteria.clone();
        }
        let client = self.client.clone();
        let task = cx
            .background_executor()
            .spawn(async move { client.set_acceptance(&plan, &task_id, &criteria) });
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                this.update(cx, |this, cx| {
                    this.show_error(format!("Could not save the checklist: {error}"));
                    this.poll_now(true, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Everything known about a task, in time order: created, claimed, board
    /// notes, commits on its branch, merged, last update.
    pub(super) fn task_timeline(
        &mut self,
        repo: usize,
        plan_id: String,
        task_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some((plan, task)) = self.find_task(repo, &plan_id, &task_id) else {
            self.show_error("That task is no longer in the plan");
            return;
        };
        let Some(view) = self.repos.get(repo) else {
            return;
        };
        let repo_path = view.entry.path.clone();
        // The repo's default branch (origin/HEAD), not whatever the main
        // checkout happens to be on.
        let main = worktree::default_branch(&repo_path)
            .or_else(|| {
                view.snapshot
                    .as_ref()
                    .and_then(|s| s.worktrees.as_ref()?.as_ref().ok()?.first()?.branch.clone())
            })
            .unwrap_or_else(|| "main".into());
        let mut actions = vec![(
            "Open in fubbik".to_string(),
            Command::OpenWeb(format!("/plans/{plan_id}")),
        )];
        if let Some(path) = view.snapshot.as_ref().and_then(|s| {
            s.worktrees
                .as_ref()?
                .as_ref()
                .ok()?
                .iter()
                .find(|w| task.link.branch.is_some() && w.branch == task.link.branch)
                .map(|w| w.path.clone())
        }) {
            actions.push(("Shell in its worktree".to_string(), Command::ShellAt(path)));
        }
        if task.status == "pending" {
            actions.push((
                "Start".to_string(),
                Command::StartTask {
                    repo,
                    plan: plan_id.clone(),
                    task: task_id.clone(),
                    new_worktree: true,
                    background: false,
                },
            ));
        }
        self.panel = Some(Panel {
            title: format!("{} — {}", task.title, plan.title),
            body: Loadable::Loading,
            actions,
        });
        let client = self.client.clone();
        let work = cx.background_executor().spawn(async move {
            let mut events: Vec<(i64, String)> = Vec::new();
            let at = |events: &mut Vec<(i64, String)>, time: Option<&str>, text: String| {
                if let Some(t) = time.and_then(activity::parse_rfc3339) {
                    events.push((t, text));
                }
            };
            at(&mut events, task.created_at.as_deref(), "created".into());
            // Each source on its own: a missing board still shows the commits.
            let mut notes = Vec::new();
            let board = match client.board(&plan_id) {
                Ok(board) => board,
                Err(error) => {
                    notes.push(format!("(board unavailable: {error})"));
                    None
                }
            };
            if let Some(board) = board {
                for claim in board.claims.iter().filter(|c| c.task_id == task_id) {
                    let handle = board
                        .runs
                        .iter()
                        .find(|r| r.id == claim.run_id)
                        .map_or("an agent", |r| r.handle.as_str());
                    at(
                        &mut events,
                        claim.claimed_at.as_deref(),
                        format!(
                            "claimed by {handle}{}",
                            if claim.expired {
                                " (lease expired)"
                            } else {
                                ""
                            }
                        ),
                    );
                }
                for entry in board
                    .entries
                    .iter()
                    .filter(|e| e.task_id.as_deref() == Some(task_id.as_str()))
                {
                    at(
                        &mut events,
                        Some(entry.created_at.as_str()),
                        format!(
                            "{}: {}",
                            entry.kind,
                            entry.body.lines().next().unwrap_or_default()
                        ),
                    );
                }
            }
            if let Some(branch) = &task.link.branch {
                let log = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&repo_path)
                    .args([
                        "log",
                        "--format=%h%x09%at%x09%s",
                        &format!("{main}..{branch}"),
                    ])
                    .output();
                let log = match log {
                    Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
                    Ok(o) => {
                        notes.push(format!(
                            "(git log {main}..{branch} failed: {})",
                            String::from_utf8_lossy(&o.stderr)
                                .lines()
                                .next()
                                .unwrap_or_default()
                        ));
                        String::new()
                    }
                    Err(error) => {
                        notes.push(format!("(git log failed: {error})"));
                        String::new()
                    }
                };
                for commit in activity::parse_log(&log) {
                    events.push((
                        commit.time,
                        format!("commit {} {}", commit.hash, commit.subject),
                    ));
                }
                if let Some(base) = &task.link.base_commit
                    && worktree::task_merged(&repo_path, branch, base, &main)
                {
                    events.push((activity::now(), format!("{branch} is merged into {main}")));
                }
            }
            at(
                &mut events,
                task.updated_at.as_deref(),
                format!("last update (now {})", task.status.replace('_', " ")),
            );
            events.sort_by_key(|(t, _)| *t);
            let mut text = String::new();
            if let Some(branch) = &task.link.branch {
                text.push_str(&format!("Branch {branch}\n\n"));
            }
            for (t, what) in events {
                text.push_str(&format!("{}  {what}\n", activity::format_time(t)));
            }
            if text.is_empty() {
                text.push_str("Nothing recorded for this task yet.");
            }
            for note in notes {
                text.push_str(&format!("\n{note}"));
            }
            Ok::<String, String>(text)
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
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

    /// A short "what changed" note for a finished task, in the repo's space.
    pub(super) fn save_task_note(
        &mut self,
        repo: usize,
        plan_id: String,
        task_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some((plan, task)) = self.find_task(repo, &plan_id, &task_id) else {
            return;
        };
        let Some(space) = self.repos.get(repo).and_then(|r| r.space_id()) else {
            self.show_error("This repo has no fubbik space");
            return;
        };
        let effort = self.task_effort(repo, &plan_id, &task);
        let mut content = format!(
            "Task “{}” of plan “{}” is done.\n\n",
            task.title, plan.title
        );
        if let Some(description) = &task.description {
            content.push_str(&format!("{description}\n\n"));
        }
        for (criterion, done) in &task.acceptance {
            content.push_str(&format!(
                "- [{}] {criterion}\n",
                if *done { "x" } else { " " }
            ));
        }
        if let Some(branch) = &task.link.branch {
            content.push_str(&format!("\nBranch: `{branch}`\n"));
        }
        if effort.sessions > 0 {
            content.push_str(&format!(
                "Agent sessions: {} · tokens: {}\n",
                effort.sessions, effort.tokens
            ));
        }
        let title = format!("Done: {}", task.title);
        self.api_action(
            "Saved a note for the task",
            move |c| c.quick_note(&space, &title, &content),
            cx,
        );
    }

    /// After knowledge arrives: tasks whose branch landed on main are marked
    /// done, their worktree is removed when clean, and a toast offers to
    /// save a note.
    pub(super) fn close_merged_tasks(&mut self, cx: &mut Context<Self>) {
        let mut candidates = Vec::new();
        for (index, view) in self.repos.iter().enumerate() {
            let (Some(Ok(knowledge)), Some(snapshot)) = (&view.knowledge, &view.snapshot) else {
                continue;
            };
            let Some(main) = snapshot
                .worktrees
                .as_ref()
                .and_then(|w| w.as_ref().ok()?.first()?.branch.clone())
            else {
                continue;
            };
            for plan in &knowledge.plan_details {
                for task in plan
                    .tasks
                    .iter()
                    .filter(|t| !matches!(t.status.as_str(), "done" | "skipped"))
                {
                    if let (Some(branch), Some(base)) = (&task.link.branch, &task.link.base_commit)
                    {
                        candidates.push((
                            index,
                            view.entry.path.clone(),
                            main.clone(),
                            plan.id.clone(),
                            task.clone(),
                            branch.clone(),
                            base.clone(),
                        ));
                    }
                }
            }
        }
        if candidates.is_empty() {
            return;
        }
        let client = self.client.clone();
        let work = cx.background_executor().spawn(async move {
            let mut closed = Vec::new();
            for (repo, path, main, plan, task, branch, base) in candidates {
                if !worktree::task_merged(&path, &branch, &base, &main) {
                    continue;
                }
                if client.set_task_status(&plan, &task.id, "done").is_err() {
                    continue;
                }
                // `git worktree remove` refuses a dirty tree: nothing is lost.
                let removed = task.link.worktree.as_deref().is_some_and(|w| {
                    Path::new(w).is_dir() && worktree::remove(&path, Path::new(w)).is_ok()
                });
                closed.push((repo, plan, task, removed));
            }
            closed
        });
        cx.spawn(async move |this, cx| {
            let closed = work.await;
            this.update(cx, |this, cx| {
                for (repo, plan, task, removed) in closed {
                    let extra = if removed { ", worktree removed" } else { "" };
                    launch::notify(
                        "Task done",
                        &format!("{} landed on main{extra}", task.title),
                    );
                    this.show_alert(Alert {
                        message: format!(
                            "“{}” landed on main{extra} — click to save a note",
                            task.title
                        ),
                        target: Command::SaveTaskNote {
                            repo,
                            plan,
                            task: task.id.clone(),
                        },
                    });
                    this.refresh_worktrees = true;
                }
                this.poll_now(true, cx);
            })
            .ok();
        })
        .detach();
    }
}
