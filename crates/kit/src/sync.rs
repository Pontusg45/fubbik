use anyhow::{Result, bail};
use serde::Serialize;

use crate::git::Git;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SyncOutcome {
    UpToDate,
    FastForwarded { commits: usize },
    WouldFastForward { commits: usize },
}

/// Fetch and fast-forward only. Never pulls or rebases.
pub fn sync(git: &Git, dry_run: bool) -> Result<SyncOutcome> {
    let branch = git.current_branch()?;
    let Some(upstream) = git.upstream()? else {
        bail!("no upstream configured for {branch}");
    };
    git.run(&["fetch", "--prune"])?;
    let counts = git.run(&["rev-list", "--left-right", "--count", "HEAD...@{u}"])?;
    let mut parts = counts.split_whitespace().map(|n| n.parse::<usize>());
    let (ahead, behind) = match (parts.next(), parts.next()) {
        (Some(Ok(a)), Some(Ok(b))) => (a, b),
        _ => bail!("unexpected rev-list output: {counts}"),
    };
    if ahead > 0 && behind > 0 {
        bail!("{branch} has diverged from {upstream}; refusing to rebase — resolve it by hand");
    }
    if behind == 0 {
        return Ok(SyncOutcome::UpToDate);
    }
    if dry_run {
        return Ok(SyncOutcome::WouldFastForward { commits: behind });
    }
    git.run(&["merge", "--ff-only", "@{u}"])?;
    Ok(SyncOutcome::FastForwarded { commits: behind })
}
