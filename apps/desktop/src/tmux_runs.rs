//! Processes and scripts run in tmux sessions on the app's server
//! (`tmux -L fubbik`), so they can be attached from any terminal
//! (`tmux -L fubbik attach -t fk-run-<repo>-<name>`) and outlive the app.
//!
//! The session is the source of truth: a restarted app finds its runs by
//! the `@fubbik_run` option (the supervisor key). The pane stays after the
//! command ends (`remain-on-exit`), so its exit code and screen stay
//! readable; output is also piped to `<runs dir>/<session>.log` for the
//! Logs tab.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::tmux::{self, Server};

/// Run sessions start with this; terminal tabs leave them alone.
pub const PREFIX: &str = "fk-run-";
const KEY_OPTION: &str = "@fubbik_run";
const STOP_GRACE: Duration = Duration::from_secs(2);
/// Bytes read from the end of a log.
const LOG_TAIL: u64 = 512 * 1024;

/// `fk-run-<repo folder>-<name>` for a key `<repo path>::<name>`; tmux
/// reserves `.` and `:` in target names.
pub fn session_name(key: &str) -> String {
    let (repo, name) = key.rsplit_once("::").unwrap_or(("", key));
    let repo = Path::new(repo)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let raw = if repo.is_empty() {
        format!("{PREFIX}{name}")
    } else {
        format!("{PREFIX}{repo}-{name}")
    };
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// The attach command for a run's session, to paste into any terminal.
pub fn attach_command(session: &str) -> String {
    format!("tmux -L {} attach -t {session}", tmux::SERVER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneState {
    /// 0 for a dead pane: tmux 3.8 reports no pid once the process exits.
    pub pid: u32,
    pub dead: bool,
    pub exit: Option<i32>,
}

/// `pid<TAB>dead<TAB>status`. A dead pane may have an empty pid (tmux 3.8);
/// a live one must have one.
fn parse_pane(line: &str) -> Option<PaneState> {
    let mut parts = line.split('\t');
    let pid = parts.next()?;
    let dead = parts.next() == Some("1");
    let exit = parts.next().and_then(|s| s.parse().ok());
    let pid = match pid.parse() {
        Ok(pid) => pid,
        Err(_) if dead => 0,
        Err(_) => return None,
    };
    Some(PaneState { pid, dead, exit })
}

/// Where runs live: the app's tmux server and a folder for their logs.
#[derive(Debug, Clone)]
pub struct Runs {
    pub server: Server,
    pub logs: PathBuf,
}

impl Runs {
    pub fn log_path(&self, key: &str) -> PathBuf {
        self.logs.join(format!("{}.log", session_name(key)))
    }

    /// The pane of a run's session, if the session exists.
    pub fn state(&self, key: &str) -> Option<PaneState> {
        // `display-message -t =session` resolves no pane; list-panes does.
        let out = tmux::command(
            &self.server,
            &[
                "list-panes",
                "-t",
                &format!("={}", session_name(key)),
                "-F",
                "#{pane_pid}\t#{pane_dead}\t#{pane_dead_status}",
            ],
        )
        .ok()?;
        parse_pane(out.lines().next()?)
    }

    pub fn alive(&self, key: &str) -> bool {
        self.state(key).is_some_and(|s| !s.dead)
    }

    /// Starts `command` in a new detached session (replacing a finished
    /// one); the session name on success.
    pub fn start(
        &self,
        key: &str,
        command: &str,
        cwd: &Path,
        label: &str,
    ) -> Result<String, String> {
        let name = session_name(key);
        match self.state(key) {
            Some(state) if !state.dead => {
                return Err(format!("already running in tmux session {name}"));
            }
            Some(_) => {
                let _ = tmux::command(&self.server, &["kill-session", "-t", &format!("={name}")]);
            }
            None => {}
        }
        std::fs::create_dir_all(&self.logs).map_err(|e| e.to_string())?;
        let log = self.log_path(key);
        let header = format!("── $ {command}  (tmux session {name}) ──\n");
        append(&log, &header)?;
        let config = tmux::config_file()?;
        let cwd_arg = cwd.to_string_lossy().into_owned();
        let shell = crate::supervisor::shell();
        // A moment's pause so pipe-pane is attached before the first output.
        let script = format!("sleep 0.2; {command}");
        tmux::command(
            &self.server,
            &[
                "-f",
                &config,
                "new-session",
                "-d",
                "-s",
                &name,
                "-c",
                &cwd_arg,
                "-x",
                "200",
                "-y",
                "50",
                &shell,
                "-lc",
                &script,
            ],
        )?;
        tmux::command(&self.server, &["set-option", "-t", &name, KEY_OPTION, key])?;
        tmux::command(
            &self.server,
            &["set-option", "-t", &name, "@fubbik_label", label],
        )?;
        tmux::command(
            &self.server,
            &["set-option", "-w", "-t", &name, "remain-on-exit", "on"],
        )?;
        let quoted = log.to_string_lossy().replace('\'', r"'\''");
        tmux::command(
            &self.server,
            &[
                "pipe-pane",
                "-t",
                &name,
                "-o",
                &format!("cat >> '{quoted}'"),
            ],
        )?;
        Ok(name)
    }

    /// Stops the run: its process group gets SIGTERM, then SIGKILL after a
    /// grace period, then the session goes.
    pub fn stop(&self, key: &str) {
        let name = session_name(key);
        if let Some(state) = self.state(key)
            && !state.dead
        {
            signal_group(state.pid, "TERM");
            let deadline = Instant::now() + STOP_GRACE;
            while Instant::now() < deadline && self.alive(key) {
                std::thread::sleep(Duration::from_millis(50));
            }
            signal_group(state.pid, "KILL");
            let _ = append(&self.log_path(key), "── stopped ──\n");
        }
        let _ = tmux::command(&self.server, &["kill-session", "-t", &format!("={name}")]);
    }

    /// Every run's pane, by key, in one tmux call.
    pub fn all_states(&self) -> std::collections::HashMap<String, PaneState> {
        let format =
            format!("#{{{KEY_OPTION}}}\t#{{pane_pid}}\t#{{pane_dead}}\t#{{pane_dead_status}}");
        let Ok(out) = tmux::command(&self.server, &["list-panes", "-a", "-F", &format]) else {
            return Default::default();
        };
        out.lines()
            .filter_map(|line| {
                let (key, rest) = line.split_once('\t')?;
                (!key.is_empty()).then_some(())?;
                Some((key.to_string(), parse_pane(rest)?))
            })
            .collect()
    }

    /// Supervisor keys of every run session on the server.
    #[cfg(test)]
    pub fn keys(&self) -> Vec<String> {
        tmux::command(
            &self.server,
            &["list-sessions", "-F", &format!("#{{{KEY_OPTION}}}")],
        )
        .map(|out| {
            out.lines()
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
    }

    /// The log's last `lines` lines, escape codes stripped.
    pub fn read_log(&self, key: &str, lines: usize) -> Vec<String> {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(self.log_path(key)) else {
            return Vec::new();
        };
        let len = file.metadata().map_or(0, |m| m.len());
        let from = len.saturating_sub(LOG_TAIL);
        let mut bytes = Vec::new();
        if file.seek(SeekFrom::Start(from)).is_err() || file.read_to_end(&mut bytes).is_err() {
            return Vec::new();
        }
        let text = strip_ansi(&String::from_utf8_lossy(&bytes));
        let mut all: Vec<String> = text
            .split('\n')
            // A carriage return redraws the line: keep what was drawn last.
            .map(|l| {
                l.rsplit('\r')
                    .find(|part| !part.is_empty())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        if from > 0 && !all.is_empty() {
            all.remove(0);
        }
        while all.last().is_some_and(|l| l.is_empty()) {
            all.pop();
        }
        let skip = all.len().saturating_sub(lines);
        all.split_off(skip)
    }

    pub fn clear_log(&self, key: &str) {
        let _ = std::fs::write(self.log_path(key), "");
    }
}

fn append(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .map_err(|e| e.to_string())
}

fn signal_group(pgid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([&format!("-{signal}"), "--", &format!("-{pgid}")])
        .stderr(Stdio::null())
        .status();
}

/// Removes terminal escape sequences (colors, cursor moves, titles).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            if c == '\u{7}' || (c.is_control() && c != '\n' && c != '\r' && c != '\t') {
                continue;
            }
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters, then a final byte @–~.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: until BEL or ESC \.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            // Charset selection and the like take one more character.
            Some('(' | ')' | '#' | '%') => {
                chars.next();
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_names_are_tmux_safe() {
        // Given
        let keys = [
            "/Users/me/projects/fubbik::api",
            "/x/my.repo::script:db-start",
        ];

        // When
        let names: Vec<String> = keys.iter().map(|k| session_name(k)).collect();

        // Then
        assert_eq!(
            names,
            ["fk-run-fubbik-api", "fk-run-my-repo-script-db-start"]
        );
    }

    #[test]
    fn escape_codes_and_redraws_are_cleaned_up() {
        // Given
        let raw = "\u{1b}[32mok\u{1b}[0m done\n\u{1b}]0;title\u{7}50%\r100%\n";

        // When
        let clean = strip_ansi(raw);
        let last: Vec<String> = clean
            .split('\n')
            .map(|l| {
                l.rsplit('\r')
                    .find(|p| !p.is_empty())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        // Then
        assert_eq!(last[..2], ["ok done".to_string(), "100%".to_string()]);
    }

    #[test]
    fn a_dead_pane_without_a_pid_still_parses() {
        // Given: tmux 3.8 leaves pane_pid empty once the process has exited
        let dead = "\t1\t3";
        let live_without_pid = "\t0\t";

        // When
        let parsed = parse_pane(dead);

        // Then
        assert_eq!(
            parsed.map(|p| (p.pid, p.dead, p.exit)),
            Some((0, true, Some(3)))
        );
        assert!(parse_pane(live_without_pid).is_none());
    }

    /// Start, log, exit code, restart and stop in a real tmux session (on a
    /// private server).
    #[test]
    fn a_run_lives_in_tmux_and_logs_its_output() {
        if !tmux::available() {
            return;
        }
        // Given
        let logs = tempfile::tempdir().unwrap();
        let runs = Runs {
            server: Server::Named(format!("fubbik-test-runs-{}", std::process::id())),
            logs: logs.path().to_path_buf(),
        };
        let wait = |what: &str, done: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !done() {
                assert!(Instant::now() < deadline, "timed out waiting for {what}");
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        let dir = tempfile::tempdir().unwrap();

        // When: a script that prints and exits 3, then a long-running one
        let name = runs
            .start(
                "/x/app::script:hello",
                "echo hello-run; exit 3",
                dir.path(),
                "hello",
            )
            .unwrap();
        wait("the exit", &|| {
            runs.state("/x/app::script:hello").is_some_and(|s| s.dead)
        });
        let finished = runs.state("/x/app::script:hello").unwrap();
        let log = runs.read_log("/x/app::script:hello", 50);
        let keys = runs.keys();
        runs.start("/x/app::api", "sleep 30", dir.path(), "api")
            .unwrap();
        wait("the api", &|| runs.alive("/x/app::api"));
        let again = runs.start("/x/app::api", "sleep 30", dir.path(), "api");
        runs.stop("/x/app::api");

        // Then
        assert_eq!(name, "fk-run-app-script-hello");
        assert_eq!(finished.exit, Some(3));
        assert!(log.iter().any(|l| l == "hello-run"), "{log:?}");
        assert_eq!(keys, ["/x/app::script:hello"]);
        assert!(again.unwrap_err().contains("already running"));
        assert!(
            runs.state("/x/app::api").is_none(),
            "session gone after stop"
        );
        crate::tmux::kill_server(&runs.server);
    }
}
