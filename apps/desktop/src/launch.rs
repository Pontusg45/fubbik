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

/// The locale variables that decide the character set, by precedence.
const LOCALE_VARS: [&str; 3] = ["LC_ALL", "LC_CTYPE", "LANG"];

/// Whether the effective locale (the first of LC_ALL, LC_CTYPE, LANG that is
/// set) is UTF-8.
fn locale_is_utf8(get: impl Fn(&str) -> Option<String>) -> bool {
    LOCALE_VARS
        .iter()
        .find_map(|var| get(var).filter(|v| !v.is_empty()))
        .is_some_and(|v| {
            let v = v.to_ascii_lowercase();
            v.contains("utf-8") || v.contains("utf8")
        })
}

/// Apps launched from the Dock get no LANG. Without a UTF-8 locale tmux
/// draws every non-ASCII character as `_`, and shells and tools fall back to
/// ASCII; everything the app starts inherits this.
pub fn adopt_utf8_locale() {
    if locale_is_utf8(|var| std::env::var(var).ok()) {
        return;
    }
    // SAFETY: called once at startup, before any other thread exists.
    unsafe {
        // A non-UTF-8 LC_ALL or LC_CTYPE would override LANG.
        std::env::remove_var("LC_ALL");
        std::env::remove_var("LC_CTYPE");
        std::env::set_var("LANG", "en_US.UTF-8");
    }
}

#[cfg(test)]
mod locale_tests {
    use super::locale_is_utf8;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn no_locale_is_not_utf8() {
        // Given: the Dock's environment
        let vars = [];

        // When
        let utf8 = locale_is_utf8(env(&vars));

        // Then
        assert!(!utf8);
    }

    #[test]
    fn the_first_set_variable_decides() {
        // Given
        let lang = [("LANG", "en_US.UTF-8")];
        let c_overrides = [("LC_ALL", "C"), ("LANG", "en_US.UTF-8")];
        let empty_skipped = [("LC_ALL", ""), ("LC_CTYPE", "sv_SE.utf8")];

        // When
        let results =
            [lang.as_slice(), &c_overrides, &empty_skipped].map(|v| locale_is_utf8(env(v)));

        // Then
        assert_eq!(results, [true, false, true]);
    }
}
