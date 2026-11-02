//! What the projects take on disk, and cleaning it: each repo's build
//! output (`target/`, `.turbo`) and installed packages (`node_modules`,
//! at the root and in workspace packages), measured with `du`. Cleaning
//! always rebuilds or reinstalls on the next build, so the app asks first.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Cargo's build output.
    Target,
    NodeModules,
    /// Turborepo's cache.
    Turbo,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Target => "target/ (Rust builds)",
            Kind::NodeModules => "node_modules",
            Kind::Turbo => ".turbo cache",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub kind: Kind,
    pub paths: Vec<PathBuf>,
    pub bytes: u64,
}

/// `du -sk` of paths, in bytes.
fn du(paths: &[PathBuf]) -> u64 {
    if paths.is_empty() {
        return 0;
    }
    let output = Command::new("du").arg("-sk").args(paths).output();
    output
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split_whitespace().next()?.parse::<u64>().ok())
                .sum::<u64>()
                * 1024
        })
        .unwrap_or(0)
}

/// `node_modules` at the root and one or two levels down (workspace
/// packages), never inside another `node_modules`.
fn node_modules(repo: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let root = repo.join("node_modules");
    if root.is_dir() {
        out.push(root);
    }
    for first in std::fs::read_dir(repo).into_iter().flatten().flatten() {
        let path = first.path();
        let name = first.file_name().to_string_lossy().into_owned();
        if !path.is_dir()
            || name.starts_with('.')
            || matches!(name.as_str(), "node_modules" | "target")
        {
            continue;
        }
        for second in std::fs::read_dir(&path).into_iter().flatten().flatten() {
            let candidate = second.path().join("node_modules");
            if second.path().is_dir() && candidate.is_dir() {
                out.push(candidate);
            }
        }
    }
    out
}

/// The repo's cleanable folders and their sizes (empty kinds left out).
pub fn scan(repo: &Path) -> Vec<Item> {
    let mut out = Vec::new();
    let target = repo.join("target");
    if target.is_dir() {
        out.push(Item {
            kind: Kind::Target,
            bytes: du(std::slice::from_ref(&target)),
            paths: vec![target],
        });
    }
    let modules = node_modules(repo);
    if !modules.is_empty() {
        out.push(Item {
            kind: Kind::NodeModules,
            bytes: du(&modules),
            paths: modules,
        });
    }
    let turbo = repo.join(".turbo");
    if turbo.is_dir() {
        out.push(Item {
            kind: Kind::Turbo,
            bytes: du(std::slice::from_ref(&turbo)),
            paths: vec![turbo],
        });
    }
    out
}

/// Removes an item. `target/` goes through `cargo clean` (it knows what is
/// its own); the rest is deleted.
pub fn clean(repo: &Path, item: &Item) -> Result<(), String> {
    match item.kind {
        Kind::Target => {
            let output = Command::new("cargo")
                .arg("clean")
                .current_dir(repo)
                .env_remove("CARGO_TARGET_DIR")
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }
            Ok(())
        }
        Kind::NodeModules | Kind::Turbo => {
            for path in &item.paths {
                // Only what the scan found, and only by its expected name.
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !path.starts_with(repo) || !matches!(name.as_str(), "node_modules" | ".turbo") {
                    return Err(format!("refusing to delete {}", path.display()));
                }
                std::fs::remove_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Ok(())
        }
    }
}

/// How to get it back.
pub fn restore_hint(repo: &Path, kind: Kind) -> &'static str {
    match kind {
        Kind::Target => "the next cargo build rebuilds it",
        Kind::Turbo => "turbo rebuilds its cache on the next run",
        Kind::NodeModules if repo.join("pnpm-lock.yaml").is_file() => {
            "run pnpm install to get it back"
        }
        Kind::NodeModules
            if repo.join("bun.lock").is_file() || repo.join("bun.lockb").is_file() =>
        {
            "run bun install to get it back"
        }
        Kind::NodeModules => "run npm install to get it back",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_and_cleans_node_modules_and_turbo() {
        // Given: root and package node_modules, a nested one, a .turbo
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        for p in [
            "node_modules/a",
            "apps/web/node_modules/b",
            "node_modules/a/node_modules/c",
            ".turbo",
        ] {
            std::fs::create_dir_all(repo.join(p)).unwrap();
        }
        std::fs::write(repo.join("node_modules/a/big"), vec![0u8; 50_000]).unwrap();
        std::fs::write(repo.join("pnpm-lock.yaml"), "").unwrap();

        // When
        let items = scan(repo);
        let modules = items
            .iter()
            .find(|i| i.kind == Kind::NodeModules)
            .unwrap()
            .clone();
        clean(repo, &modules).unwrap();

        // Then
        assert_eq!(
            modules.paths.len(),
            2,
            "root and apps/web, not the nested one: {:?}",
            modules.paths
        );
        assert!(modules.bytes >= 50_000);
        assert!(items.iter().any(|i| i.kind == Kind::Turbo));
        assert!(
            !repo.join("node_modules").exists() && !repo.join("apps/web/node_modules").exists()
        );
        assert!(repo.join("apps/web").exists(), "only node_modules goes");
        assert_eq!(
            restore_hint(repo, Kind::NodeModules),
            "run pnpm install to get it back"
        );
    }

    #[test]
    fn refuses_anything_it_did_not_find() {
        // Given: an item pointing outside the repo
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let item = Item {
            kind: Kind::NodeModules,
            paths: vec![other.path().join("node_modules")],
            bytes: 0,
        };

        // When
        let result = clean(dir.path(), &item);

        // Then
        assert!(result.unwrap_err().contains("refusing"));
    }
}
