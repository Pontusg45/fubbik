//! tmux as the persistence layer for terminals and agents.
//!
//! The app's own terminals live on a separate server (`tmux -L fubbik`) with a
//! minimal config, so the user's sessions and `tmux.conf` are untouched.
//! Quitting the app detaches its clients; the sessions keep running and are
//! reattached on the next launch. Panes on the user's default server are only
//! read (to find agents) and attached to, never changed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The app's tmux server: `tmux -L fubbik`, or the name in
/// `FUBBIK_DESKTOP_TMUX_SERVER` (a second copy of the app, for testing, gets
/// its own server so it never touches the first one's tabs, runs or hooks).
pub fn server_name() -> String {
    std::env::var("FUBBIK_DESKTOP_TMUX_SERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "fubbik".into())
}
/// Session options that carry the tab's label and directory across restarts.
const LABEL_OPTION: &str = "@fubbik_label";

/// Which tmux server: the app's (`-L <name>`) or the user's default.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Server {
    Named(String),
    Default,
}

impl Server {
    pub fn fubbik() -> Self {
        Server::Named(server_name())
    }

    fn args(&self) -> Vec<String> {
        match self {
            Server::Named(name) => vec!["-L".into(), name.clone()],
            Server::Default => Vec::new(),
        }
    }

    pub fn is_fubbik(&self) -> bool {
        *self == Server::fubbik()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub dir: PathBuf,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub server: Server,
    /// `%12`: stable for the pane's life.
    pub id: String,
    pub session: String,
    /// The pane's process (its shell).
    pub pid: u32,
    pub path: PathBuf,
    /// What runs in the foreground, e.g. `zsh`, `claude`, `node`.
    pub command: String,
}

pub fn available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The fubbik server's config: no status bar (the app draws tabs), no escape
/// delay, deep history, and true color through to the embedded terminal.
pub const CONFIG: &str = "\
set -g status off
set -sg escape-time 0
set -g history-limit 50000
set -g default-terminal \"tmux-256color\"
set -as terminal-features \",xterm-256color:RGB\"
set -g set-titles on
set -g set-titles-string \"#{pane_title}\"
set -g focus-events on
";

pub fn config_path() -> PathBuf {
    crate::config::path().with_file_name("tmux.conf")
}

fn write_config() -> Result<PathBuf, String> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if std::fs::read_to_string(&path).ok().as_deref() != Some(CONFIG) {
        std::fs::write(&path, CONFIG).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

fn tmux(server: &Server, args: &[&str]) -> Result<String, String> {
    let output = Command::new("tmux")
        .args(server.args())
        .args(args)
        .output()
        .map_err(|e| format!("tmux: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs `tmux <server args> <args>`; stdout on success.
pub fn command(server: &Server, args: &[&str]) -> Result<String, String> {
    tmux(server, args)
}

/// The app's tmux config file, written if missing (pass with `-f` when a
/// command may start the server).
pub fn config_file() -> Result<String, String> {
    write_config().map(|p| p.to_string_lossy().into_owned())
}

/// A session name from a directory and a number: `fubbik-desktop-3`.
/// tmux reserves `.` and `:` in target names.
pub fn session_name(dir: &Path, n: usize) -> String {
    let base: String = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "shell".into())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{base}-{n}")
}

/// Starts a detached session in `dir`, typing `command` into its shell when
/// given (so the shell stays after the command ends).
pub fn create(
    server: &Server,
    name: &str,
    dir: &Path,
    label: &str,
    command: Option<&str>,
) -> Result<(), String> {
    let config = write_config()?;
    let dir_arg = dir.to_string_lossy().into_owned();
    // The config applies when the server starts; `-f` is ignored after that.
    let config_arg = config.to_string_lossy().into_owned();
    let mut args = vec![
        "-f",
        config_arg.as_str(),
        "new-session",
        "-d",
        "-s",
        name,
        "-c",
        dir_arg.as_str(),
    ];
    args.extend(["-x", "120", "-y", "32"]);
    let locale = locale_args();
    args.extend(locale.iter().map(String::as_str));
    tmux(server, &args)?;
    // The server may have just started: give it the event hooks.
    if server.is_fubbik() {
        let _ = crate::tmux_events::install(server, &crate::tmux_events::events_path());
    }
    tmux(server, &["set-option", "-t", name, LABEL_OPTION, label])?;
    if let Some(command) = command {
        tmux(server, &["send-keys", "-t", name, "-l", command])?;
        tmux(server, &["send-keys", "-t", name, "Enter"])?;
    }
    Ok(())
}

/// `-e LANG=…` for new-session: a server started without a UTF-8 locale
/// (from the Dock) keeps that environment, and its shells would draw `_`.
pub fn locale_args() -> Vec<String> {
    match std::env::var("LANG") {
        Ok(lang) if !lang.is_empty() => vec!["-e".into(), format!("LANG={lang}")],
        _ => Vec::new(),
    }
}

/// The argv that attaches to a session, for the embedded terminal's PTY.
pub fn attach_argv(server: &Server, name: &str) -> Vec<String> {
    // -u: the client draws UTF-8 whatever the locale says (else `_`).
    let mut argv = vec!["tmux".to_string(), "-u".to_string()];
    argv.extend(server.args());
    argv.extend(["attach-session".into(), "-t".into(), name.to_string()]);
    argv
}

pub fn exists(server: &Server, name: &str) -> bool {
    tmux(server, &["has-session", "-t", &format!("={name}")]).is_ok()
}

#[cfg(test)]
/// Where tmux puts a named server's socket: `$TMUX_TMPDIR/tmux-<uid>/<name>`.
fn socket_path(name: &str) -> Option<PathBuf> {
    let uid = Command::new("id").arg("-u").output().ok()?;
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    let base = std::env::var_os("TMUX_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/tmp".into());
    Some(base.join(format!("tmux-{uid}")).join(name))
}

#[cfg(test)]
/// Stops a whole server and removes its socket file (used by tests). The
/// server may already have exited with its last session.
pub fn kill_server(server: &Server) {
    let _ = tmux(server, &["kill-server"]);
    if let Server::Named(name) = server
        && let Some(socket) = socket_path(name)
    {
        let _ = std::fs::remove_file(socket);
    }
}

/// What a session's pane runs in the foreground (`zsh`, `node`, `claude`…).
pub fn foreground_command(server: &Server, name: &str) -> Option<String> {
    let out = tmux(
        server,
        &[
            "list-panes",
            "-t",
            &format!("={name}"),
            "-F",
            "#{pane_current_command}",
        ],
    )
    .ok()?;
    out.lines()
        .next()
        .map(str::to_string)
        .filter(|c| !c.is_empty())
}

/// Drops a session's history (its screen is cleared by the shell).
pub fn clear(server: &Server, name: &str) -> Result<(), String> {
    tmux(server, &["clear-history", "-t", &format!("={name}")]).map(|_| ())
}

pub fn kill(server: &Server, name: &str) -> Result<(), String> {
    tmux(server, &["kill-session", "-t", &format!("={name}")]).map(|_| ())
}

const SESSION_FORMAT: &str = "#{session_name}\t#{session_path}\t#{@fubbik_label}";

pub fn parse_sessions(text: &str) -> Vec<Session> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.to_string();
            let dir = PathBuf::from(parts.next()?);
            let label = parts
                .next()
                .filter(|l| !l.is_empty())
                .unwrap_or("Shell")
                .to_string();
            (!name.is_empty()).then_some(Session { name, dir, label })
        })
        .collect()
}

/// The server's sessions; empty when it is not running.
pub fn sessions(server: &Server) -> Vec<Session> {
    tmux(server, &["list-sessions", "-F", SESSION_FORMAT])
        .map(|text| parse_sessions(&text))
        .unwrap_or_default()
}

const PANE_FORMAT: &str =
    "#{pane_id}\t#{session_name}\t#{pane_pid}\t#{pane_current_path}\t#{pane_current_command}";

pub fn parse_panes(server: &Server, text: &str) -> Vec<Pane> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(Pane {
                server: server.clone(),
                id: parts.next()?.to_string(),
                session: parts.next()?.to_string(),
                pid: parts.next()?.parse().ok()?,
                path: PathBuf::from(parts.next()?),
                command: parts.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// One server's panes; empty when it is not running.
pub fn list_panes(server: &Server) -> Vec<Pane> {
    tmux(server, &["list-panes", "-a", "-F", PANE_FORMAT])
        .map(|text| parse_panes(server, &text))
        .unwrap_or_default()
}

/// Every pane on the app's server and the user's default server.
pub fn all_panes() -> Vec<Pane> {
    [Server::fubbik(), Server::Default]
        .iter()
        .flat_map(list_panes)
        .collect()
}

/// The last `lines` non-blank lines on a pane's screen.
pub fn capture(pane: &Pane, lines: usize) -> Option<String> {
    let text = tmux(&pane.server, &["capture-pane", "-p", "-J", "-t", &pane.id]).ok()?;
    Some(last_lines(&text, lines))
}

pub fn last_lines(text: &str, lines: usize) -> String {
    let kept: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    kept[kept.len().saturating_sub(lines)..].join("\n")
}

/// Types `text` into a pane and presses Enter.
pub fn send(pane: &Pane, text: &str) -> Result<(), String> {
    tmux(&pane.server, &["send-keys", "-t", &pane.id, "-l", text])?;
    tmux(&pane.server, &["send-keys", "-t", &pane.id, "Enter"]).map(|_| ())
}

/// Scrolls a session's history through copy mode (`-e`: leaves copy mode at
/// the bottom). Positive is up.
pub fn scroll(server: &Server, name: &str, lines: isize) -> Result<(), String> {
    let target = format!("={name}");
    if lines > 0 {
        tmux(server, &["copy-mode", "-e", "-t", &target])?;
    }
    let (direction, count) = if lines > 0 {
        ("scroll-up", lines)
    } else {
        ("scroll-down", -lines)
    };
    let count = count.to_string();
    tmux(
        server,
        &["send-keys", "-t", &target, "-X", "-N", &count, direction],
    )
    .map(|_| ())
}

/// The parent chain of a process, nearest first (via `ps`).
pub fn ancestors(pid: u32) -> Vec<u32> {
    let mut chain = Vec::new();
    let mut current = pid;
    for _ in 0..16 {
        let Some(parent) = Command::new("ps")
            .args(["-o", "ppid=", "-p", &current.to_string()])
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<u32>()
                    .ok()
            })
        else {
            break;
        };
        if parent <= 1 {
            break;
        }
        chain.push(parent);
        current = parent;
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private server per test, killed on drop.
    struct TestServer(Server);

    impl TestServer {
        fn new(tag: &str) -> Self {
            TestServer(Server::Named(format!(
                "fubbik-test-{tag}-{}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            kill_server(&self.0);
        }
    }

    #[test]
    fn session_names_are_tmux_safe() {
        // Given
        let dir = Path::new("/code/my.app:v2");

        // When
        let name = session_name(dir, 3);

        // Then
        assert_eq!(name, "my-app-v2-3");
    }

    #[test]
    fn parses_sessions_and_panes() {
        // Given
        let sessions = "app-1\t/code/app\tClaude\napp-2\t/code/app\t\n";
        let panes = "%3\tapp-1\t4242\t/code/app\tclaude\n";

        // When
        let parsed = parse_sessions(sessions);
        let parsed_panes = parse_panes(&Server::Default, panes);

        // Then
        assert_eq!(parsed[0].label, "Claude");
        assert_eq!(parsed[1].label, "Shell");
        assert_eq!(parsed_panes[0].pid, 4242);
        assert_eq!(parsed_panes[0].command, "claude");
    }

    #[test]
    fn last_lines_skips_blank_lines() {
        // Given
        let text = "one\n\n  \ntwo   \nthree\n\n\n";

        // When
        let tail = last_lines(text, 2);

        // Then
        assert_eq!(tail, "two\nthree");
    }

    #[test]
    fn create_list_send_capture_kill_on_a_private_server() {
        if !available() {
            return;
        }
        // Given
        let server = TestServer::new("life");
        let dir = tempfile::tempdir().unwrap();

        // When
        create(
            &server.0,
            "t-1",
            dir.path(),
            "Claude",
            Some("echo started-$((40+2))"),
        )
        .unwrap();
        let listed = sessions(&server.0);
        let pane = tmux(&server.0, &["list-panes", "-a", "-F", PANE_FORMAT])
            .map(|t| parse_panes(&server.0, &t))
            .unwrap()
            .remove(0);
        let mut screen = String::new();
        for _ in 0..100 {
            screen = capture(&pane, 20).unwrap_or_default();
            if screen.contains("started-42") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        send(&pane, "echo replied-$((1+1))").unwrap();
        for _ in 0..100 {
            screen = capture(&pane, 20).unwrap_or_default();
            if screen.contains("replied-2") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let existed = exists(&server.0, "t-1");
        kill(&server.0, "t-1").unwrap();

        // Then
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].label, "Claude");
        assert_eq!(
            listed[0].dir.canonicalize().unwrap(),
            dir.path().canonicalize().unwrap()
        );
        assert!(screen.contains("replied-2"), "screen was: {screen}");
        assert!(existed);
        assert!(!exists(&server.0, "t-1"));
    }

    #[test]
    fn ancestors_include_our_parent() {
        // Given
        let me = std::process::id();

        // When
        let chain = ancestors(me);

        // Then
        assert!(!chain.is_empty());
    }
}
