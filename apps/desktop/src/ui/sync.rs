//! Git sync with the remote: a quiet background fetch every 15 minutes (so
//! ahead/behind is true), "Pull" as a fast-forward only (never a rebase or
//! merge), and "Push" behind a confirmation, run in a terminal tab where its
//! hooks and credential prompts show.

use std::time::{Duration, Instant};

use super::*;

/// How often each repo is fetched in the background.
const FETCH_EVERY: Duration = Duration::from_secs(15 * 60);

/// One repo's sync state.
#[derive(Default)]
pub struct SyncState {
    pub fetched_at: Option<Instant>,
    /// "Fetching…" / "Pulling…" while it runs.
    pub busy: Option<&'static str>,
    /// The last fetch's error (offline, no access), shown as a hint.
    pub fetch_error: Option<String>,
}

impl Workspace {
    /// Fetches the repos whose last fetch is older than [`FETCH_EVERY`]
    /// (`force`: this one now).
    pub(super) fn fetch_due(&mut self, force: Option<usize>, cx: &mut Context<Self>) {
        for index in 0..self.repos.len() {
            let view = &self.repos[index];
            let has_remote = view
                .snapshot
                .as_ref()
                .is_some_and(|s| s.remote.is_some() && !s.missing);
            let due = force == Some(index)
                || (force.is_none()
                    && view
                        .sync
                        .fetched_at
                        .is_none_or(|at| at.elapsed() >= FETCH_EVERY));
            if !has_remote || !due || view.sync.busy.is_some() {
                continue;
            }
            self.run_sync(index, "Fetching…", false, cx);
        }
    }

    fn run_sync(&mut self, index: usize, busy: &'static str, pull: bool, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get_mut(index) else {
            return;
        };
        view.sync.busy = Some(busy);
        let path = view.entry.path.clone();
        let work = {
            let path = path.clone();
            cx.background_executor().spawn(async move {
                if pull {
                    crate::git::pull_ff(&path).map(Some)
                } else {
                    crate::git::fetch(&path).map(|_| None)
                }
            })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                let Some(view) = this.repos.iter_mut().find(|r| r.entry.path == path) else {
                    return;
                };
                view.sync.busy = None;
                view.sync.fetched_at = Some(Instant::now());
                let name = view.entry.name();
                match (result, pull) {
                    (Ok(Some(head)), _) => this.show_info(format!("{name}: up to date at {head}")),
                    (Ok(None), _) => view.sync.fetch_error = None,
                    (Err(error), true) => this.show_error(format!("{name}: pull failed — {error}")),
                    (Err(error), false) => view.sync.fetch_error = Some(error),
                }
                // Ahead/behind come from the next poll.
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn pull(&mut self, index: usize, cx: &mut Context<Self>) {
        self.run_sync(index, "Pulling…", true, cx);
    }

    /// Asks first, then pushes in a terminal tab.
    pub(super) fn push(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(view) = self.repos.get(index) else {
            return;
        };
        let Some(Ok(git)) = view.snapshot.as_ref().and_then(|s| s.git.clone()) else {
            self.show_error("No git state for this repo yet");
            return;
        };
        let path = view.entry.path.clone();
        let command = crate::git::push_command(&path, &git.branch);
        let what = if command.contains("-u origin") {
            format!("Publish {} to origin?", git.branch)
        } else {
            format!("Push {} commit(s) on {}?", git.ahead, git.branch)
        };
        let confirm = Command::AskFirst(Box::new(super::palette::Confirm {
            title: what,
            detail: format!(
                "Runs `{command}` in {} in a terminal tab, where hooks and credential prompts show.",
                display_path(&path)
            ),
            action: "Push".into(),
            danger: false,
            command: Command::RunIn {
                dir: path,
                label: format!("push {}", git.branch),
                command,
            },
        }));
        self.run(confirm, cx);
    }
}
