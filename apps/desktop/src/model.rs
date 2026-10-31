//! What the app knows about each repo at a moment: the result of one poll.

use std::path::Path;

use crate::git::GitInfo;
use crate::services::{DatabaseSpec, ProcessSpec};
use crate::sessions::AiSession;
use crate::worktree::Worktree;

pub const CPU_HISTORY: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Started by this app and healthy.
    Running,
    /// Up, but not started by this app (someone else holds the port).
    External,
    /// Up but failing its health check, or only partly up.
    Degraded,
    Stopped,
    /// The repo itself is broken: missing folder or unreadable services file.
    Error,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Running => "Running",
            Status::External => "External",
            Status::Degraded => "Degraded",
            Status::Stopped => "Stopped",
            Status::Error => "Error",
        }
    }

    pub fn is_up(self) -> bool {
        matches!(self, Status::Running | Status::External | Status::Degraded)
    }
}

/// Running only when everything is up; Stopped when nothing is.
pub fn roll_up(statuses: &[Status]) -> Status {
    if statuses.is_empty() {
        return Status::Stopped;
    }
    if statuses.contains(&Status::Error) {
        return Status::Error;
    }
    let up = statuses.iter().filter(|s| s.is_up()).count();
    if up == 0 {
        Status::Stopped
    } else if up == statuses.len() && !statuses.contains(&Status::Degraded) {
        Status::Running
    } else {
        Status::Degraded
    }
}

#[derive(Debug, Clone)]
pub struct ProcessState {
    pub spec: ProcessSpec,
    pub key: String,
    pub status: Status,
    pub pid: Option<u32>,
    /// Started by this app (false: an external process holds the port).
    pub owned: bool,
    pub cpu_history: Vec<f32>,
    pub rss_kb: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DatabaseState {
    pub spec: DatabaseSpec,
    pub status: Status,
}

#[derive(Debug, Clone, Default)]
pub struct RepoSnapshot {
    pub missing: bool,
    pub git: Option<Result<GitInfo, String>>,
    pub remote: Option<String>,
    /// None: no services file yet.
    pub has_services_file: bool,
    pub services_error: Option<String>,
    pub processes: Vec<ProcessState>,
    pub databases: Vec<DatabaseState>,
    /// None until first read; refreshed less often than the probes.
    pub worktrees: Option<Result<Vec<Worktree>, String>>,
    /// AI sessions in the repo or any of its worktrees, live first.
    pub sessions: Vec<AiSession>,
    /// Worktrees whose branches change the same files (merge conflicts ahead).
    pub overlaps: Vec<Overlap>,
    /// Runnable scripts: just recipes, package.json scripts, make targets.
    pub scripts: Vec<crate::scripts::Script>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlap {
    pub a: String,
    pub b: String,
    pub files: Vec<String>,
}

/// Pairs of worktrees changing the same files: (name, changed files) in,
/// overlapping pairs out.
pub fn overlaps(changed: &[(String, Vec<String>)]) -> Vec<Overlap> {
    let mut out = Vec::new();
    for (i, (a, a_files)) in changed.iter().enumerate() {
        for (b, b_files) in &changed[i + 1..] {
            let files: Vec<String> = a_files
                .iter()
                .filter(|f| b_files.contains(f))
                .cloned()
                .collect();
            if !files.is_empty() {
                out.push(Overlap {
                    a: a.clone(),
                    b: b.clone(),
                    files,
                });
            }
        }
    }
    out
}

impl RepoSnapshot {
    pub fn status(&self) -> Status {
        if self.missing || self.services_error.is_some() {
            return Status::Error;
        }
        let statuses: Vec<Status> = self
            .processes
            .iter()
            .map(|p| p.status)
            .chain(self.databases.iter().map(|d| d.status))
            .collect();
        roll_up(&statuses)
    }

    pub fn live_sessions(&self) -> usize {
        self.sessions.iter().filter(|s| s.state.is_live()).count()
    }

    pub fn worktree_paths(&self) -> Vec<std::path::PathBuf> {
        match &self.worktrees {
            Some(Ok(worktrees)) => worktrees.iter().map(|w| w.path.clone()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn processes_up(&self) -> usize {
        self.processes.iter().filter(|p| p.status.is_up()).count()
    }

    pub fn databases_up(&self) -> usize {
        self.databases.iter().filter(|d| d.status.is_up()).count()
    }
}

pub fn process_key(repo: &Path, name: &str) -> String {
    format!("{}::{name}", repo.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_up_all_running() {
        // Given
        let statuses = [Status::Running, Status::Running];

        // When
        let status = roll_up(&statuses);

        // Then
        assert_eq!(status, Status::Running);
    }

    #[test]
    fn roll_up_partial_is_degraded() {
        // Given
        let statuses = [Status::Running, Status::Stopped];

        // When
        let status = roll_up(&statuses);

        // Then
        assert_eq!(status, Status::Degraded);
    }

    #[test]
    fn external_counts_as_up() {
        // Given
        let statuses = [Status::External, Status::Running];

        // When
        let status = roll_up(&statuses);

        // Then
        assert_eq!(status, Status::Running);
    }

    #[test]
    fn finds_worktrees_changing_the_same_files() {
        // Given
        let changed = vec![
            (
                "task/a".to_string(),
                vec!["src/x.rs".to_string(), "src/y.rs".to_string()],
            ),
            ("task/b".to_string(), vec!["src/y.rs".to_string()]),
            ("task/c".to_string(), vec!["README.md".to_string()]),
        ];

        // When
        let found = overlaps(&changed);

        // Then
        assert_eq!(
            found,
            [Overlap {
                a: "task/a".into(),
                b: "task/b".into(),
                files: vec!["src/y.rs".into()]
            }]
        );
    }

    #[test]
    fn nothing_configured_is_stopped() {
        // Given
        let snapshot = RepoSnapshot::default();

        // When
        let status = snapshot.status();

        // Then
        assert_eq!(status, Status::Stopped);
    }
}
