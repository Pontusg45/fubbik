use anyhow::Result;
use serde::Serialize;

use crate::git::{Commit, Git};

#[derive(Debug, Clone, Serialize)]
pub struct PushPlan {
    pub branch: String,
    pub upstream: Option<String>,
    pub commits: Vec<Commit>,
}

/// Commits not on any remote, oldest first.
pub fn plan(git: &Git) -> Result<PushPlan> {
    let branch = git.current_branch()?;
    let upstream = git.upstream()?;
    let log = git.run(&[
        "log",
        "--reverse",
        "--format=%H%x09%s",
        "HEAD",
        "--not",
        "--remotes",
    ])?;
    let commits = log
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(sha, subject)| Commit {
            sha: sha.to_string(),
            subject: subject.to_string(),
        })
        .collect();
    Ok(PushPlan {
        branch,
        upstream,
        commits,
    })
}

pub fn execute(git: &Git, plan: &PushPlan) -> Result<()> {
    if plan.upstream.is_some() {
        git.run(&["push"])?;
    } else {
        git.run(&["push", "-u", "origin", &plan.branch])?;
    }
    Ok(())
}
