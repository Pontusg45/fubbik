use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
}

/// Thin wrapper over the `git` binary, scoped to one working directory.
#[derive(Debug)]
pub struct Git {
    dir: PathBuf,
}

impl Git {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Git> {
        let dir = dir.into();
        let git = Git { dir };
        if git.run(&["rev-parse", "--git-dir"]).is_err() {
            bail!("not a git repository: {}", git.dir.display());
        }
        Ok(git)
    }

    /// Run git and return trimmed stdout. On failure the error carries git's stderr.
    pub fn run(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .output()
            .context("failed to run git")?;
        if !out.status.success() {
            return Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn current_branch(&self) -> Result<String> {
        self.run(&["symbolic-ref", "--short", "HEAD"])
            .map_err(|_| anyhow!("detached HEAD; check out a branch first"))
    }

    pub fn is_clean(&self) -> Result<bool> {
        Ok(self.run(&["status", "--porcelain"])?.is_empty())
    }

    pub fn upstream(&self) -> Result<Option<String>> {
        Ok(self
            .run(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
            .ok())
    }
}
