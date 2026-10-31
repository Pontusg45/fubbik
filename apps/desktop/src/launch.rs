//! Handing off to other apps: editor, terminal, browser, notifications.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::supervisor::shell;

fn on_path(program: &str) -> bool {
    Command::new(shell())
        .args(["-lc", &format!("command -v {program}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn spawn_detached(command: &str, cwd: &Path) -> Result<(), String> {
    Command::new(shell())
        .args(["-lc", command])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// `$VISUAL`, `$EDITOR`, then `zed`, then `code`.
pub fn open_in_editor(repo: &Path) -> Result<(), String> {
    let editor = ["VISUAL", "EDITOR"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.is_empty()))
        .or_else(|| {
            ["zed", "code"]
                .iter()
                .find(|p| on_path(p))
                .map(|p| p.to_string())
        })
        .ok_or("no editor found: set $EDITOR, or install zed or code")?;
    spawn_detached(&format!("{editor} {}", quote(repo)), repo)
}

pub fn open_in_terminal(repo: &Path) -> Result<(), String> {
    spawn_detached(&format!("open -a Terminal {}", quote(repo)), repo)
}

/// Opens a Terminal window in `cwd` running `command`.
pub fn run_in_terminal(cwd: &Path, command: &str) -> Result<(), String> {
    let script_line = format!("cd {} && {command}", quote(cwd));
    let escaped = script_line.replace('\\', "\\\\").replace('"', "\\\"");
    let status = Command::new("osascript")
        .args([
            "-e",
            "tell application \"Terminal\" to activate",
            "-e",
            &format!("tell application \"Terminal\" to do script \"{escaped}\""),
        ])
        .stdout(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Terminal did not accept the command (check Automation permission)".into())
    }
}

/// A macOS notification through `osascript`.
pub fn notify(title: &str, body: &str) {
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        escape(body),
        escape(title)
    );
    let _ = Command::new("osascript")
        .args(["-e", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// GUI apps launched from Finder get a minimal PATH. Take the login shell's
/// so `pnpm`, `just`, `docker` and `lsof` resolve as they do in a terminal.
pub fn adopt_login_path() {
    let Ok(output) = Command::new(shell())
        .args(["-lc", "printf %s \"$PATH\""])
        .output()
    else {
        return;
    };
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() && !path.is_empty() {
        // SAFETY: called once at startup, before any other thread exists.
        unsafe { std::env::set_var("PATH", path) };
    }
}
