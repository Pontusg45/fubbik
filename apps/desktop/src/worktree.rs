//! A repo's git worktrees, from `git worktree list --porcelain`, each with its
//! own dirty count and its distance from the main worktree's branch.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: String,
    /// None when detached or bare.
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
    /// The first entry: the main working tree.
    pub main: bool,
    pub missing: bool,
    pub changed_files: usize,
    /// Commits on this worktree's HEAD that the main branch lacks, and the reverse.
    pub ahead: usize,
    pub behind: usize,
}

impl Worktree {
    pub fn name(&self) -> String {
        match &self.branch {
            Some(branch) => branch.clone(),
            None if self.bare => "(bare)".into(),
            None => format!("(detached {})", self.head),
        }
    }

    /// Clean, and everything on its branch is already on the main branch:
    /// removing it loses nothing.
    pub fn merged(&self) -> bool {
        !self.main && !self.missing && self.ahead == 0 && self.changed_files == 0
    }
}

pub fn parse_list(text: &str) -> Vec<Worktree> {
    let mut worktrees: Vec<Worktree> = Vec::new();
    for line in text.lines() {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "worktree" {
            worktrees.push(Worktree {
                path: PathBuf::from(value),
                main: worktrees.is_empty(),
                ..Default::default()
            });
            continue;
        }
        let Some(current) = worktrees.last_mut() else {
            continue;
        };
        match key {
            "HEAD" => current.head = value.chars().take(7).collect(),
            "branch" => {
                current.branch = Some(
                    value
                        .strip_prefix("refs/heads/")
                        .unwrap_or(value)
                        .to_string(),
                )
            }
            "bare" => current.bare = true,
            "detached" => current.detached = true,
            "locked" => current.locked = Some(value.to_string()),
            "prunable" => current.prunable = Some(value.to_string()),
            _ => {}
        }
    }
    worktrees
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `rev-list --left-right --count main...HEAD` → (behind, ahead).
fn parse_left_right(text: &str) -> (usize, usize) {
    let mut parts = text.split_whitespace().map(|n| n.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

pub fn read(repo: &Path) -> Result<Vec<Worktree>, String> {
    let mut worktrees = parse_list(&git(repo, &["worktree", "list", "--porcelain"])?);
    let main_branch = worktrees.first().and_then(|w| w.branch.clone());
    for worktree in &mut worktrees {
        if worktree.bare {
            continue;
        }
        if !worktree.path.is_dir() {
            worktree.missing = true;
            continue;
        }
        if let Ok(status) = git(&worktree.path, &["status", "--porcelain"]) {
            worktree.changed_files = status.lines().filter(|l| !l.is_empty()).count();
        }
        if let Some(main) = &main_branch
            && !worktree.main
            && let Ok(counts) = git(
                &worktree.path,
                &[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("{main}...HEAD"),
                ],
            )
        {
            (worktree.behind, worktree.ahead) = parse_left_right(&counts);
        }
    }
    Ok(worktrees)
}

/// Files this worktree changed: committed since it left `main_branch`
/// (`git diff --name-only main...HEAD`) plus uncommitted and untracked ones.
pub fn changed_paths(worktree: &Path, main_branch: Option<&str>) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    if let Some(main) = main_branch
        && let Ok(text) = git(
            worktree,
            &["diff", "--name-only", &format!("{main}...HEAD")],
        )
    {
        paths.extend(text.lines().map(str::to_string));
    }
    if let Ok(text) = git(
        worktree,
        &["status", "--porcelain", "--untracked-files=all"],
    ) {
        paths.extend(text.lines().filter_map(|line| {
            let path = line.get(3..)?;
            // A rename reads "old -> new".
            Some(
                path.rsplit(" -> ")
                    .next()
                    .unwrap_or(path)
                    .trim_matches('"')
                    .to_string(),
            )
        }));
    }
    paths.retain(|p| !p.is_empty());
    paths.sort();
    paths.dedup();
    paths
}

/// A new worktree on a new branch, next to the repo: `<parent>/<repo>-<branch>`.
pub fn create(repo: &Path, branch: &str) -> Result<PathBuf, String> {
    let branch = branch.trim();
    if branch.is_empty() || branch.contains(char::is_whitespace) || branch.starts_with('-') {
        return Err("Branch name must be one word and not start with '-'".into());
    }
    let repo_name = repo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir_name = format!("{repo_name}-{}", branch.replace('/', "-"));
    let path = repo.parent().unwrap_or(repo).join(dir_name);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let path_arg = path.to_string_lossy().into_owned();
    git(repo, &["worktree", "add", "-b", branch, &path_arg])?;
    Ok(path)
}

/// `git worktree remove` without `--force`: git refuses a dirty worktree.
pub fn remove(repo: &Path, path: &Path) -> Result<(), String> {
    let path_arg = path.to_string_lossy().into_owned();
    git(repo, &["worktree", "remove", &path_arg]).map(|_| ())
}

pub fn prune(repo: &Path) -> Result<(), String> {
    git(repo, &["worktree", "prune", "-v"]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_main_branch_detached_and_prunable() {
        // Given
        let text = "worktree /code/app\nHEAD af0c95f1234\nbranch refs/heads/main\n\n\
                    worktree /code/app-feat\nHEAD 1234567890\nbranch refs/heads/feat/x\nlocked\n\n\
                    worktree /code/gone\nHEAD abcdef0123\ndetached\nprunable gitdir file points to non-existent location\n";

        // When
        let worktrees = parse_list(text);

        // Then
        assert_eq!(worktrees.len(), 3);
        assert!(worktrees[0].main);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[0].head, "af0c95f");
        assert_eq!(worktrees[1].name(), "feat/x");
        assert_eq!(worktrees[1].locked.as_deref(), Some(""));
        assert!(worktrees[2].detached);
        assert_eq!(worktrees[2].name(), "(detached abcdef0)");
        assert!(worktrees[2].prunable.is_some());
    }

    #[test]
    fn parses_left_right_counts() {
        // Given
        let text = "2\t5\n";

        // When
        let (behind, ahead) = parse_left_right(text);

        // Then
        assert_eq!((behind, ahead), (2, 5));
    }

    #[test]
    fn create_and_remove_in_a_real_repo() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("app");
        std::fs::create_dir(&repo).unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(&repo)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ]);

        // When
        let path = create(&repo, "feat/x").unwrap();
        let listed = read(&repo).unwrap();
        remove(&repo, &path).unwrap();
        let after = read(&repo).unwrap();

        // Then
        assert_eq!(path.file_name().unwrap(), "app-feat-x");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[1].branch.as_deref(), Some("feat/x"));
        assert!(listed[1].merged());
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn changed_paths_cover_commits_and_working_tree() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("app");
        std::fs::create_dir(&repo).unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(&repo)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        let commit = |msg: &str| {
            run(&[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                msg,
            ])
        };
        run(&["init", "-q", "-b", "main"]);
        std::fs::write(repo.join("base.txt"), "x").unwrap();
        run(&["add", "."]);
        commit("base");
        run(&["checkout", "-q", "-b", "feat"]);
        std::fs::write(repo.join("committed.rs"), "x").unwrap();
        run(&["add", "."]);
        commit("feat");
        std::fs::write(repo.join("untracked.md"), "x").unwrap();

        // When
        let paths = changed_paths(&repo, Some("main"));

        // Then
        assert_eq!(paths, ["committed.rs", "untracked.md"]);
    }

    #[test]
    fn rejects_bad_branch_names() {
        // Given
        let dir = tempfile::tempdir().unwrap();

        // When
        let results = ["", "two words", "-x"].map(|name| create(dir.path(), name));

        // Then
        assert!(results.iter().all(Result::is_err));
    }
}
