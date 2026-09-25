use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::HooksCommand;
use crate::output::{self, OutputMode};

const HOOK_SCRIPT: &str = "#!/bin/sh\n# Installed by fubbik — chunk-aware pre-commit hook\nfubbik check-files --staged 2>&1 || true\n";

pub fn run(command: HooksCommand, mode: OutputMode) -> Result<()> {
    let path = hook_path()?;
    match command {
        HooksCommand::Install { force } => {
            install(&path, force)?;
            report(mode, "installed", &path)
        }
        HooksCommand::Uninstall => {
            uninstall(&path)?;
            report(mode, "removed", &path)
        }
    }
}

fn hook_path() -> Result<PathBuf> {
    let output = Command::new("git")
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "hooks/pre-commit",
        ])
        .output()
        .context("failed to locate the Git hooks directory")?;
    if !output.status.success() {
        bail!("not inside a Git repository");
    }
    Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
}

fn install(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!("a pre-commit hook already exists; use --force to overwrite it");
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, HOOK_SCRIPT).with_context(|| format!("failed to write {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to make {} executable", path.display()))?;
    Ok(())
}

fn uninstall(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("no pre-commit hook found");
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if !content.contains("Installed by fubbik") {
        bail!("pre-commit hook was not installed by fubbik; refusing to remove it");
    }
    fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
}

fn report(mode: OutputMode, action: &str, path: &Path) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&serde_json::json!({"action": action, "path": path}));
    }
    if mode == OutputMode::Quiet {
        println!("{}", path.display());
    } else {
        println!("{action} fubbik pre-commit hook at {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_an_executable_hook_and_requires_force_to_replace_it() {
        // Given an empty temporary hooks directory
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pre-commit");
        // When the hook is installed and a second install is attempted
        install(&path, false).unwrap();
        let duplicate = install(&path, false);
        // Then the script is executable and replacement requires force
        assert_eq!(fs::read_to_string(&path).unwrap(), HOOK_SCRIPT);
        assert_ne!(fs::metadata(&path).unwrap().permissions().mode() & 0o111, 0);
        assert!(duplicate.unwrap_err().to_string().contains("--force"));
    }

    #[test]
    fn uninstall_only_removes_hooks_owned_by_fubbik() {
        // Given a foreign hook and a fubbik hook
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pre-commit");
        fs::write(&path, "#!/bin/sh\necho foreign\n").unwrap();
        // When removal is attempted before and after a forced fubbik install
        let foreign = uninstall(&path);
        install(&path, true).unwrap();
        uninstall(&path).unwrap();
        // Then the foreign hook was protected and the owned hook was removed
        assert!(foreign.unwrap_err().to_string().contains("refusing"));
        assert!(!path.exists());
    }
}
