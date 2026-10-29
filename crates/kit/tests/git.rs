use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kit::git::Git;
use kit::sync::SyncOutcome;
use kit::{branch, push, sync};

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Make a repo hermetic against the user's global git config.
fn configure(dir: &Path) {
    sh(dir, &["config", "user.name", "Test"]);
    sh(dir, &["config", "user.email", "test@example.com"]);
    sh(dir, &["config", "commit.gpgsign", "false"]);
    sh(dir, &["config", "core.hooksPath", "/dev/null"]);
}

fn commit(dir: &Path, file: &str, msg: &str) {
    std::fs::write(dir.join(file), msg).unwrap();
    sh(dir, &["add", file]);
    sh(dir, &["commit", "-m", msg]);
}

fn clone_of(origin: &Path, dest: &Path) {
    let status = Command::new("git")
        .arg("clone")
        .arg(origin)
        .arg(dest)
        .output()
        .expect("spawn git clone");
    assert!(status.status.success());
    configure(dest);
}

/// Returns (bare origin, clone) with one commit pushed to the local origin only.
fn fixture(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("kit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let origin = root.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    sh(&origin, &["init", "--bare", "-b", "main"]);
    let work = root.join("work");
    clone_of(&origin, &work);
    sh(&work, &["checkout", "-B", "main"]);
    commit(&work, "base.txt", "base");
    sh(&work, &["push", "-u", "origin", "main"]);
    (origin, work)
}

/// Second clone that pushes `n` commits to origin.
fn push_from_other(origin: &Path, n: usize) {
    let other = origin.parent().unwrap().join("other");
    clone_of(origin, &other);
    for i in 0..n {
        commit(&other, &format!("o{i}.txt"), &format!("remote {i}"));
    }
    sh(&other, &["push", "origin", "main"]);
}

#[test]
fn sync_fast_forwards_when_behind() {
    // Given a clone that is 2 commits behind origin
    let (origin, work) = fixture("ff");
    push_from_other(&origin, 2);
    let git = Git::open(&work).unwrap();
    // When syncing
    let outcome = sync::sync(&git, false).unwrap();
    // Then it fast-forwards to origin/main
    assert_eq!(outcome, SyncOutcome::FastForwarded { commits: 2 });
    assert_eq!(
        sh(&work, &["rev-parse", "HEAD"]),
        sh(&work, &["rev-parse", "origin/main"])
    );
}

#[test]
fn sync_dry_run_does_not_move_head() {
    // Given a clone that is 2 commits behind origin
    let (origin, work) = fixture("dry");
    push_from_other(&origin, 2);
    let before = sh(&work, &["rev-parse", "HEAD"]);
    let git = Git::open(&work).unwrap();
    // When syncing with dry_run
    let outcome = sync::sync(&git, true).unwrap();
    // Then HEAD is unchanged
    assert_eq!(outcome, SyncOutcome::WouldFastForward { commits: 2 });
    assert_eq!(sh(&work, &["rev-parse", "HEAD"]), before);
}

#[test]
fn sync_refuses_when_diverged() {
    // Given one local and one remote commit
    let (origin, work) = fixture("div");
    push_from_other(&origin, 1);
    commit(&work, "local.txt", "local");
    let before = sh(&work, &["rev-parse", "HEAD"]);
    let git = Git::open(&work).unwrap();
    // When syncing
    let err = sync::sync(&git, false).unwrap_err();
    // Then it refuses and leaves the local commit alone
    assert!(err.to_string().contains("diverged"), "{err}");
    assert_eq!(sh(&work, &["rev-parse", "HEAD"]), before);
    assert_eq!(sh(&work, &["log", "-1", "--format=%s"]), "local");
}

#[test]
fn sync_without_upstream_errors() {
    // Given a new local branch with no upstream
    let (_origin, work) = fixture("noup");
    sh(&work, &["switch", "-c", "lonely"]);
    let git = Git::open(&work).unwrap();
    // When syncing
    let err = sync::sync(&git, false).unwrap_err();
    // Then the error names the branch
    assert_eq!(err.to_string(), "no upstream configured for lonely");
}

#[test]
fn push_plan_lists_unpushed_oldest_first() {
    // Given two unpushed commits
    let (_origin, work) = fixture("plan");
    commit(&work, "a.txt", "a");
    commit(&work, "b.txt", "b");
    let git = Git::open(&work).unwrap();
    // When planning a push
    let plan = push::plan(&git).unwrap();
    // Then commits are oldest first with the upstream set
    let subjects: Vec<_> = plan.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["a", "b"]);
    assert_eq!(plan.upstream.as_deref(), Some("origin/main"));
    assert_eq!(plan.branch, "main");
}

#[test]
fn push_plan_without_upstream() {
    // Given a new branch with one commit
    let (_origin, work) = fixture("planno");
    sh(&work, &["switch", "-c", "feat"]);
    commit(&work, "f.txt", "f");
    let git = Git::open(&work).unwrap();
    // When planning
    let plan = push::plan(&git).unwrap();
    // Then there is no upstream and one commit
    assert_eq!(plan.upstream, None);
    assert_eq!(plan.commits.len(), 1);
}

#[test]
fn branch_refuses_dirty_tree() {
    // Given an untracked file
    let (_origin, work) = fixture("dirty");
    std::fs::write(work.join("junk.txt"), "x").unwrap();
    let git = Git::open(&work).unwrap();
    // When creating a branch
    let err = branch::create(&git, "feat/x", None).unwrap_err();
    // Then it refuses and the branch does not exist
    assert!(err.to_string().contains("uncommitted changes"), "{err}");
    assert!(sh(&work, &["branch", "--list", "feat/x"]).is_empty());
}

#[test]
fn branch_creates_from_base() {
    // Given a clean repo
    let (_origin, work) = fixture("base");
    let git = Git::open(&work).unwrap();
    // When creating a branch from main
    branch::create(&git, "feat/x", Some("main")).unwrap();
    // Then it is checked out
    assert_eq!(git.current_branch().unwrap(), "feat/x");
}

#[test]
fn errors_outside_repo() {
    // Given an empty temp dir
    let dir = std::env::temp_dir().join(format!("kit-norepo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // When opening
    let err = Git::open(&dir).unwrap_err();
    // Then the error is exact
    assert_eq!(
        err.to_string(),
        format!("not a git repository: {}", dir.display())
    );
}

#[test]
fn push_requires_yes_without_tty() {
    // Given an unpushed commit
    let (origin, work) = fixture("notty");
    commit(&work, "a.txt", "a");
    let before = sh(&origin, &["rev-parse", "main"]);
    // When running `fk push` with no terminal and no --yes
    let out = Command::new(env!("CARGO_BIN_EXE_fk"))
        .arg("push")
        .current_dir(&work)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    // Then it fails and origin is untouched
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--yes"));
    assert_eq!(sh(&origin, &["rev-parse", "main"]), before);
}
