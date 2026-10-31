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
