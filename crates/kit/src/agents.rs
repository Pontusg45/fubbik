//! `fk agents link`: expose `agents/` material to Claude Code through
//! relative symlinks under `.claude/`.
//!
//! Only entries directly inside `.claude/skills/` and the single
//! `.claude/commands` link are ever managed. Everything else under `.claude/`
//! (third-party skill links, worktrees, settings) is left alone.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinkAction {
    Created { link: PathBuf, target: PathBuf },
    AlreadyLinked { link: PathBuf },
    Replaced { link: PathBuf, old_target: PathBuf },
    Pruned { link: PathBuf },
}

/// Only dangling links whose text starts with this are pruned.
const AGENTS_PREFIX: &str = "../../agents/";

/// Skill source directories, relative to the repo root, each paired with the
/// relative link target used from `.claude/skills/`.
const SKILL_SOURCES: [(&str, &str); 2] = [
    ("agents/skills", "../../agents/skills"),
    ("agents/project/skills", "../../agents/project/skills"),
];

/// Creates a relative symlink. Unix only: `link` refuses on other targets
/// before reaching here, but this stub keeps the rest of `fk` compiling.
#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "symlinks require a Unix-like OS",
    ))
}

/// Link `agents/` into `.claude/`. With `dry_run`, report without changing disk.
/// Unix only: on other targets it returns an error and changes nothing.
pub fn link(repo_root: &Path, dry_run: bool) -> Result<Vec<LinkAction>> {
    if cfg!(not(unix)) {
        bail!("fk agents link requires a Unix-like OS");
    }
    let skills_dir = repo_root.join(".claude/skills");

    // (link path, relative target)
    let mut desired: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (src, target_prefix) in SKILL_SOURCES {
        let src_dir = repo_root.join(src);
        let Ok(entries) = fs::read_dir(&src_dir) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("SKILL.md").is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        for name in names {
            let link = skills_dir.join(&name);
            if desired.iter().any(|(l, _)| *l == link) {
                bail!("skill {name} exists in more than one agents/ skill directory");
            }
            desired.push((link, Path::new(target_prefix).join(&name)));
        }
    }
    if repo_root.join("agents/commands").is_dir() {
        desired.push((
            repo_root.join(".claude/commands"),
            PathBuf::from("../agents/commands"),
        ));
    }

    // Plan everything first so a refusal changes nothing.
    let mut actions = Vec::new();
    let mut writes: Vec<(PathBuf, PathBuf, bool)> = Vec::new(); // link, target, replace
    for (link, target) in &desired {
        match fs::symlink_metadata(link) {
            Err(_) => {
                actions.push(LinkAction::Created {
                    link: link.clone(),
                    target: target.clone(),
                });
                writes.push((link.clone(), target.clone(), false));
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                let current = fs::read_link(link)?;
                if current == *target {
                    actions.push(LinkAction::AlreadyLinked { link: link.clone() });
                } else {
                    actions.push(LinkAction::Replaced {
                        link: link.clone(),
                        old_target: current,
                    });
                    writes.push((link.clone(), target.clone(), true));
                }
            }
            Ok(_) => bail!("refusing to replace real file {}", link.display()),
        }
    }

    let mut prunes = Vec::new();
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if desired.iter().any(|(l, _)| *l == path) {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.file_type().is_symlink() {
                continue;
            }
            let text = fs::read_link(&path)?;
            if text.starts_with(AGENTS_PREFIX) && !path.exists() {
                actions.push(LinkAction::Pruned { link: path.clone() });
                prunes.push(path);
            }
        }
    }

    if !dry_run {
        for path in &prunes {
            fs::remove_file(path).with_context(|| format!("failed to prune {}", path.display()))?;
        }
        for (link, target, replace) in &writes {
            if let Some(parent) = link.parent() {
                fs::create_dir_all(parent)?;
            }
            if *replace {
                fs::remove_file(link)?;
            }
            symlink(target, link).with_context(|| format!("failed to link {}", link.display()))?;
        }
    }
    Ok(actions)
}

pub fn describe(root: &Path, action: &LinkAction, dry_run: bool) -> String {
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let w = |done: &'static str, would: &'static str| if dry_run { would } else { done };
    match action {
        LinkAction::Created { link, target } => {
            format!(
                "{} {} -> {}",
                w("created", "would create"),
                rel(link),
                target.display()
            )
        }
        LinkAction::AlreadyLinked { link } => format!("{} already linked", rel(link)),
        LinkAction::Replaced { link, old_target } => format!(
            "{} {} (was -> {})",
            w("replaced", "would replace"),
            rel(link),
            old_target.display()
        ),
        LinkAction::Pruned { link } => format!("{} {}", w("pruned", "would prune"), rel(link)),
    }
}
