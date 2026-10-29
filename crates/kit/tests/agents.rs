use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use kit::agents::{self, LinkAction};

/// Temp dir removed on drop, including on assertion failure.
struct Repo(PathBuf);

impl Repo {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fk-agents-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Repo(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn skill(&self, rel: &str) {
        let dir = self.0.join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
    }
    fn mkdir(&self, rel: &str) {
        fs::create_dir_all(self.0.join(rel)).unwrap();
    }
    fn link(&self, rel: &str, target: &str) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        symlink(target, p).unwrap();
    }
    fn read_link(&self, rel: &str) -> PathBuf {
        fs::read_link(self.0.join(rel)).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn layout(name: &str) -> Repo {
    let r = Repo::new(name);
    r.skill("agents/skills/generic");
    r.skill("agents/project/skills/mine");
    r.mkdir("agents/commands");
    r
}

#[test]
fn link_creates_relative_symlinks() {
    // Given a repo with a generic skill, a project skill and commands
    let r = layout("create");
    // When linking
    let actions = agents::link(r.path(), false).unwrap();
    // Then relative symlinks exist for all three
    assert_eq!(actions.len(), 3);
    assert!(
        actions
            .iter()
            .all(|a| matches!(a, LinkAction::Created { .. }))
    );
    assert_eq!(
        r.read_link(".claude/skills/generic"),
        Path::new("../../agents/skills/generic")
    );
    assert_eq!(
        r.read_link(".claude/skills/mine"),
        Path::new("../../agents/project/skills/mine")
    );
    assert_eq!(
        r.read_link(".claude/commands"),
        Path::new("../agents/commands")
    );
    assert!(r.path().join(".claude/skills/mine/SKILL.md").exists());
}

#[test]
fn link_is_idempotent() {
    // Given a repo that has already been linked
    let r = layout("idem");
    agents::link(r.path(), false).unwrap();
    // When linking again
    let actions = agents::link(r.path(), false).unwrap();
    // Then everything is AlreadyLinked
    assert_eq!(actions.len(), 3);
    assert!(
        actions
            .iter()
            .all(|a| matches!(a, LinkAction::AlreadyLinked { .. }))
    );
}

#[test]
fn link_replaces_a_stale_symlink() {
    // Given a skill link pointing at its old location
    let r = layout("stale");
    r.link(".claude/skills/mine", "../../agents/skills/mine");
    // When linking
    let actions = agents::link(r.path(), false).unwrap();
    // Then it is repointed and reported as Replaced
    assert!(actions.iter().any(|a| matches!(
        a,
        LinkAction::Replaced { link, old_target }
            if link.ends_with(".claude/skills/mine")
                && old_target == Path::new("../../agents/skills/mine")
    )));
    assert_eq!(
        r.read_link(".claude/skills/mine"),
        Path::new("../../agents/project/skills/mine")
    );
}

#[test]
fn link_prunes_dangling_agents_links_but_not_third_party() {
    // Given a dangling agents link, a dangling third-party link and a live third-party link
    let r = layout("prune");
    r.skill(".agents/skills/live");
    r.link(".claude/skills/gone", "../../agents/skills/gone");
    r.link(".claude/skills/thirdgone", "../../.agents/skills/thirdgone");
    r.link(".claude/skills/live", "../../.agents/skills/live");
    // When linking
    let actions = agents::link(r.path(), false).unwrap();
    // Then only the dangling agents link is pruned
    assert!(actions.iter().any(|a| matches!(
        a, LinkAction::Pruned { link } if link.ends_with(".claude/skills/gone")
    )));
    assert!(fs::symlink_metadata(r.path().join(".claude/skills/gone")).is_err());
    assert_eq!(
        r.read_link(".claude/skills/thirdgone"),
        Path::new("../../.agents/skills/thirdgone")
    );
    assert_eq!(
        r.read_link(".claude/skills/live"),
        Path::new("../../.agents/skills/live")
    );
    assert!(!actions.iter().any(|a| matches!(
        a, LinkAction::Pruned { link } if link.ends_with("thirdgone") || link.ends_with("live")
    )));
}

#[test]
fn link_refuses_to_replace_a_real_file() {
    // Given a real directory at a skill link path and at .claude/commands is untouched-able
    let r = layout("real");
    r.mkdir(".claude/skills/mine");
    r.link(".claude/skills/gone", "../../agents/skills/gone");
    // When linking
    let err = agents::link(r.path(), false).unwrap_err();
    // Then it refuses and nothing changed on disk
    assert!(
        err.to_string()
            .starts_with("refusing to replace real file "),
        "{err}"
    );
    assert!(r.path().join(".claude/skills/mine").is_dir());
    assert!(fs::symlink_metadata(r.path().join(".claude/skills/gone")).is_ok());
    assert!(fs::symlink_metadata(r.path().join(".claude/skills/generic")).is_err());
    assert!(fs::symlink_metadata(r.path().join(".claude/commands")).is_err());
}

#[test]
fn link_refuses_a_real_commands_directory() {
    // Given .claude/commands is a real directory
    let r = layout("realcmd");
    r.mkdir(".claude/commands");
    // When linking
    let err = agents::link(r.path(), false).unwrap_err();
    // Then it refuses before creating any skill link
    assert!(
        err.to_string()
            .starts_with("refusing to replace real file "),
        "{err}"
    );
    assert!(fs::symlink_metadata(r.path().join(".claude/skills/generic")).is_err());
}

#[test]
fn dry_run_changes_nothing() {
    // Given a repo needing create, replace and prune
    let r = layout("dry");
    r.link(".claude/skills/mine", "../../agents/skills/mine");
    r.link(".claude/skills/gone", "../../agents/skills/gone");
    // When linking with dry_run
    let actions = agents::link(r.path(), true).unwrap();
    // Then actions are reported but the disk is unchanged
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, LinkAction::Created { .. }))
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, LinkAction::Replaced { .. }))
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, LinkAction::Pruned { .. }))
    );
    assert_eq!(
        r.read_link(".claude/skills/mine"),
        Path::new("../../agents/skills/mine")
    );
    assert_eq!(
        r.read_link(".claude/skills/gone"),
        Path::new("../../agents/skills/gone")
    );
    assert!(fs::symlink_metadata(r.path().join(".claude/skills/generic")).is_err());
    assert!(fs::symlink_metadata(r.path().join(".claude/commands")).is_err());
}
