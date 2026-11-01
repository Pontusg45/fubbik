//! Pull request and CI status per repo, refreshed in the background every
//! few minutes (and on Refresh) for repos with a GitHub remote, through `gh`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::*;
use crate::github::PrStatus;

/// How often a repo's PRs are read again.
const PR_EVERY: Duration = Duration::from_secs(180);

/// One repo's PRs by branch, with when they were read.
#[derive(Default)]
pub struct RepoPrs {
    pub by_branch: Option<Result<HashMap<String, PrStatus>, String>>,
    pub at: Option<Instant>,
    pub loading: bool,
}

impl Workspace {
    /// Reads PRs for repos whose last read is older than [`PR_EVERY`]
    /// (`force`: all of them). Repos without a GitHub remote, or without
    /// `gh`, are skipped.
    pub(super) fn refresh_prs(&mut self, force: bool, cx: &mut Context<Self>) {
        if !self.gh_available {
            return;
        }
        for index in 0..self.repos.len() {
            let view = &mut self.repos[index];
            let github = view
                .snapshot
                .as_ref()
                .and_then(|s| s.remote.as_deref())
                .is_some_and(|r| r.contains("github.com"));
            let due = force || view.prs.at.is_none_or(|at| at.elapsed() >= PR_EVERY);
            if !github || !due || view.prs.loading {
                continue;
            }
            view.prs.loading = true;
            let path = view.entry.path.clone();
            let work = {
                let path = path.clone();
                cx.background_executor()
                    .spawn(async move { crate::github::pull_requests(&path) })
            };
            cx.spawn(async move |this, cx| {
                let result = work.await;
                this.update(cx, |this, cx| {
                    if let Some(view) = this.repos.iter_mut().find(|r| r.entry.path == path) {
                        view.prs.loading = false;
                        view.prs.at = Some(Instant::now());
                        // A failed read keeps the last good one on screen.
                        if result.is_ok() || !matches!(view.prs.by_branch, Some(Ok(_))) {
                            view.prs.by_branch = Some(result);
                        }
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    /// The latest PR for a branch of repo `index`, if read.
    pub(super) fn pr_for(&self, index: usize, branch: &str) -> Option<&PrStatus> {
        match &self.repos.get(index)?.prs.by_branch {
            Some(Ok(map)) => map.get(branch),
            _ => None,
        }
    }
}
