//! Git state for a repo, read by shelling out to `git`.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: String,
    pub head: String,
    pub changed_files: usize,
    pub ahead: usize,
    pub behind: usize,
    pub last_commit: String,
}

/// Parses `git status --porcelain=v2 --branch`.
pub fn parse_status(text: &str) -> GitInfo {
    let mut info = GitInfo::default();
    for line in text.lines() {
        if let Some(oid) = line.strip_prefix("# branch.oid ") {
            info.head = oid.chars().take(7).collect();
        } else if let Some(head) = line.strip_prefix("# branch.head ") {
            info.branch = head.to_string();
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for part in ab.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    info.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    info.behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            info.changed_files += 1;
        }
    }
    info
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn read(repo: &Path) -> Result<GitInfo, String> {
    let mut info = parse_status(&git(repo, &["status", "--porcelain=v2", "--branch"])?);
    info.last_commit = git(repo, &["log", "-1", "--format=%s (%cr)"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Ok(info)
}

/// One changed file, from `git status --porcelain=v1 -z`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    /// The index (staged) column: `M`, `A`, `D`, `R`, `C`; None when clean.
    pub staged: Option<char>,
    /// The work tree column: `M`, `D`; None when clean.
    pub unstaged: Option<char>,
    pub untracked: bool,
    /// The old path of a rename or copy.
    pub from: Option<String>,
}

/// Parses `git status --porcelain=v1 -z` (renames carry their old path as
/// the next record).
pub fn parse_changes(text: &str) -> Vec<FileChange> {
    let mut out = Vec::new();
    let mut records = text.split('\0').filter(|r| !r.is_empty());
    while let Some(record) = records.next() {
        let mut chars = record.chars();
        let (Some(x), Some(y)) = (chars.next(), chars.next()) else {
            continue;
        };
        let Some(path) = record.get(3..) else {
            continue;
        };
        let column = |c: char| (c != ' ' && c != '?' && c != '!').then_some(c);
        let from = matches!(x, 'R' | 'C')
            .then(|| records.next().map(str::to_string))
            .flatten();
        if x == '!' {
            continue;
        }
        out.push(FileChange {
            path: path.to_string(),
            staged: column(x),
            unstaged: column(y),
            untracked: x == '?',
            from,
        });
    }
    out
}

/// Every changed file in the work tree.
pub fn changes(repo: &Path) -> Result<Vec<FileChange>, String> {
    git(
        repo,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .map(|s| parse_changes(&s))
}

/// What changed in one file: staged (index against HEAD) or not (work tree
/// against the index). An untracked file shows whole.
pub fn diff(repo: &Path, path: &str, staged: bool, untracked: bool) -> Result<String, String> {
    if untracked {
        // `--no-index` exits 1 when the files differ, which they always do.
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["diff", "--no-color", "--no-index", "--", "/dev/null", path])
            .output()
            .map_err(|e| format!("git: {e}"))?;
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let mut args = vec!["diff", "--no-color"];
    if staged {
        args.push("--cached");
    }
    args.extend(["--", path]);
    git(repo, &args)
}

/// `git add` these paths (all changes when empty).
pub fn stage(repo: &Path, paths: &[String]) -> Result<(), String> {
    let mut args = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(String::as_str));
    git(repo, &args).map(|_| ())
}

/// Takes these paths out of the index, keeping the work tree (all when empty).
pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), String> {
    let mut args = vec!["reset", "-q", "HEAD", "--"];
    args.extend(paths.iter().map(String::as_str));
    if paths.is_empty() {
        args.push(".");
    }
    git(repo, &args).map(|_| ())
}

/// Commits what is staged; the new commit's short hash and subject.
pub fn commit(repo: &Path, message: &str) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("A commit needs a message".into());
    }
    git(repo, &["commit", "-q", "-m", message])?;
    git(repo, &["log", "-1", "--format=%h %s"]).map(|s| s.trim().to_string())
}

/// Runs git with prompts off: a background fetch must never wait on a
/// password or a host-key question.
fn git_quiet(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `git fetch --prune` (all remotes' branches; nothing local changes).
pub fn fetch(repo: &Path) -> Result<(), String> {
    git_quiet(repo, &["fetch", "--quiet", "--prune"]).map(|_| ())
}

/// The current branch's upstream (`origin/main`), if it has one.
pub fn upstream(repo: &Path) -> Option<String> {
    git(
        repo,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// Fetches, then fast-forwards the current branch to its upstream. Never
/// rebases or merges: a branch that has diverged is left alone, with a
/// message saying so. Returns the new head (`abc1234 subject`).
pub fn pull_ff(repo: &Path) -> Result<String, String> {
    let upstream = upstream(repo).ok_or("this branch has no upstream to pull from")?;
    fetch(repo)?;
    git_quiet(repo, &["merge", "--ff-only", "--quiet", "@{u}"]).map_err(|e| {
        if e.contains("Not possible to fast-forward")
            || e.contains("fatal: Not possible")
            || e.contains("diverg")
        {
            format!("{upstream} and this branch have diverged: merge or rebase it yourself")
        } else {
            e
        }
    })?;
    git(repo, &["log", "-1", "--format=%h %s"]).map(|s| s.trim().to_string())
}

/// The command that pushes the current branch: `git push`, or with
/// `-u origin <branch>` the first time.
pub fn push_command(repo: &Path, branch: &str) -> String {
    if upstream(repo).is_some() {
        "git push".into()
    } else {
        format!("git push -u origin '{}'", branch.replace('\'', ""))
    }
}

/// The repo root containing `path`.
pub fn toplevel(path: &Path) -> Option<std::path::PathBuf> {
    git(path, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(|s| std::path::PathBuf::from(s.trim()))
}

/// The `origin` remote, raw.
pub fn remote(repo: &Path) -> Option<String> {
    git(repo, &["remote", "get-url", "origin"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `git@github.com:o/r.git` or `https://github.com/o/r.git` → `https://github.com/o/r`.
pub fn web_url(remote: &str) -> Option<String> {
    let remote = remote.trim().trim_end_matches(".git");
    if let Some(rest) = remote.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("https://{host}/{path}"));
    }
    if let Some(rest) = remote.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (host, path) = rest.split_once('/')?;
        let host = host.split(':').next()?;
        return Some(format!("https://{host}/{path}"));
    }
    if remote.starts_with("https://") || remote.starts_with("http://") {
        return Some(remote.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_fast_forwards_and_refuses_to_merge_a_diverged_branch() {
        // Given: an "origin" repo and a clone of it, origin one commit ahead
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin");
        let clone = dir.path().join("clone");
        let run = |repo: &Path, args: &[&str]| git(repo, args).unwrap();
        std::fs::create_dir_all(&origin).unwrap();
        run(&origin, &["init", "-q", "-b", "main"]);
        run(&origin, &["config", "user.email", "t@example.com"]);
        run(&origin, &["config", "user.name", "t"]);
        std::fs::write(origin.join("a.txt"), "1\n").unwrap();
        run(&origin, &["add", "a.txt"]);
        run(&origin, &["commit", "-q", "-m", "one"]);
        run(
            dir.path(),
            &["clone", "-q", origin.to_str().unwrap(), "clone"],
        );
        run(&clone, &["config", "user.email", "t@example.com"]);
        run(&clone, &["config", "user.name", "t"]);
        std::fs::write(origin.join("a.txt"), "2\n").unwrap();
        run(&origin, &["commit", "-qam", "two"]);

        // When: pull (a fast-forward), then diverge and pull again
        let pulled = pull_ff(&clone);
        std::fs::write(origin.join("a.txt"), "3\n").unwrap();
        run(&origin, &["commit", "-qam", "three"]);
        std::fs::write(clone.join("b.txt"), "local\n").unwrap();
        run(&clone, &["add", "b.txt"]);
        run(&clone, &["commit", "-qm", "local"]);
        let diverged = pull_ff(&clone);
        let head = run(&clone, &["log", "-1", "--format=%s"]);
        let push_known = push_command(&clone, "main");
        run(&clone, &["checkout", "-qb", "feat"]);
        let push_new = push_command(&clone, "feat");

        // Then
        assert!(pulled.unwrap().ends_with("two"));
        assert!(diverged.unwrap_err().contains("diverged"));
        assert_eq!(head.trim(), "local", "nothing was merged or rebased");
        assert_eq!(push_known, "git push");
        assert_eq!(push_new, "git push -u origin 'feat'");
    }

    #[test]
    fn parses_staged_unstaged_untracked_and_renames() {
        // Given: porcelain v1 -z, a rename followed by its old path
        let text = "M  staged.rs\0 M edited.rs\0MM both.rs\0?? new.rs\0R  new_name.rs\0old_name.rs\0!! ignored\0";

        // When
        let changes = parse_changes(text);

        // Then
        let summary: Vec<(&str, Option<char>, Option<char>, bool)> = changes
            .iter()
            .map(|c| (c.path.as_str(), c.staged, c.unstaged, c.untracked))
            .collect();
        assert_eq!(
            summary,
            [
                ("staged.rs", Some('M'), None, false),
                ("edited.rs", None, Some('M'), false),
                ("both.rs", Some('M'), Some('M'), false),
                ("new.rs", None, None, true),
                ("new_name.rs", Some('R'), None, false),
            ]
        );
        assert_eq!(changes[4].from.as_deref(), Some("old_name.rs"));
    }

    #[test]
    fn stage_diff_unstage_and_commit_in_a_real_repo() {
        // Given: a repo with one commit, then an edit and a new file
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let run = |args: &[&str]| git(repo, args).unwrap();
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "-q", "-m", "first"]);
        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        std::fs::write(repo.join("b.txt"), "new\n").unwrap();

        // When
        let before = changes(repo).unwrap();
        let untracked_diff = diff(repo, "b.txt", false, true).unwrap();
        stage(repo, &["a.txt".into()]).unwrap();
        let staged_diff = diff(repo, "a.txt", true, false).unwrap();
        unstage(repo, &["a.txt".into()]).unwrap();
        let after_unstage = changes(repo).unwrap();
        stage(repo, &[]).unwrap();
        let committed = commit(repo, "second").unwrap();
        let after_commit = changes(repo).unwrap();

        // Then
        assert_eq!(before.len(), 2);
        assert!(untracked_diff.contains("+new"), "{untracked_diff}");
        assert!(
            staged_diff.contains("-one") && staged_diff.contains("+two"),
            "{staged_diff}"
        );
        assert_eq!(
            after_unstage
                .iter()
                .find(|c| c.path == "a.txt")
                .unwrap()
                .staged,
            None
        );
        assert!(committed.ends_with("second"), "{committed}");
        assert!(after_commit.is_empty());
        assert!(commit(repo, "  ").is_err());
    }

    #[test]
    fn parses_branch_head_ahead_behind() {
        // Given
        let text = "# branch.oid af0c95f1234567\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +3 -1\n";

        // When
        let info = parse_status(text);

        // Then
        assert_eq!(info.branch, "main");
        assert_eq!(info.head, "af0c95f");
        assert_eq!((info.ahead, info.behind), (3, 1));
        assert_eq!(info.changed_files, 0);
    }

    #[test]
    fn counts_changed_and_untracked() {
        // Given
        let text = "# branch.head main\n1 .M N... 100644 100644 100644 a b src/a.rs\n? new.txt\n";

        // When
        let info = parse_status(text);

        // Then
        assert_eq!(info.changed_files, 2);
    }

    #[test]
    fn detached_head() {
        // Given
        let text = "# branch.oid 0123456789\n# branch.head (detached)\n";

        // When
        let info = parse_status(text);

        // Then
        assert_eq!(info.branch, "(detached)");
        assert_eq!(info.ahead, 0);
    }

    #[test]
    fn ssh_remote_to_https() {
        // Given
        let remotes = [
            "git@github.com:Pontusg45/fubbik.git",
            "ssh://git@github.com:22/Pontusg45/fubbik.git",
            "https://github.com/Pontusg45/fubbik.git",
        ];

        // When
        let urls: Vec<_> = remotes.iter().map(|r| web_url(r)).collect();

        // Then
        for url in urls {
            assert_eq!(url.as_deref(), Some("https://github.com/Pontusg45/fubbik"));
        }
    }
}
