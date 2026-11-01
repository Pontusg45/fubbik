//! Live events from the app's tmux server. Global hooks append one line per
//! event to a file (`kind<TAB>session<TAB>pane`), and the app reads new
//! lines as they arrive. Bells and exits then show at once, without waiting
//! for the next poll.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::tmux::{self, Server};

/// Bigger than this, the file is emptied on the next read.
const MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// A program rang the terminal bell (`\a`): it wants attention.
    Bell,
    /// A pane's command exited (the pane stays, with `remain-on-exit`).
    Died,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub session: String,
    pub pane: String,
}

/// The events file, beside the app's tmux config.
pub fn events_path() -> PathBuf {
    tmux::config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("tmux-events.log")
}

/// One `kind<TAB>session<TAB>pane` line.
pub fn parse(line: &str) -> Option<Event> {
    let mut parts = line.trim_end().splitn(3, '\t');
    let kind = match parts.next()? {
        "bell" => EventKind::Bell,
        "died" => EventKind::Died,
        _ => return None,
    };
    let session = parts.next().filter(|s| !s.is_empty())?.to_string();
    let pane = parts.next().unwrap_or_default().to_string();
    Some(Event {
        kind,
        session,
        pane,
    })
}

/// The `run-shell` hook command that appends `kind` events to `path`.
fn hook(kind: &str, path: &Path) -> String {
    let path = path.to_string_lossy().replace('\'', "");
    format!(
        "run-shell -b \"printf '{kind}\\t%s\\t%s\\n' '#{{session_name}}' '#{{pane_id}}' >> '{path}'\""
    )
}

/// Sets the hooks on a running server (idempotent: `set-hook -g` replaces).
/// A bell in any window counts, not only in windows no client shows.
pub fn install(server: &Server, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bell = hook("bell", path);
    let died = hook("died", path);
    tmux::command(
        server,
        &[
            "set-option",
            "-g",
            "bell-action",
            "any",
            ";",
            "set-hook",
            "-g",
            "alert-bell",
            &bell,
            ";",
            "set-hook",
            "-g",
            "pane-died",
            &died,
        ],
    )
    .map(|_| ())
}

/// Reads what was appended to the events file since `offset`; returns the
/// events and the new offset. A shrunk file (emptied) starts over; a big one
/// is emptied.
pub fn read_new(path: &Path, offset: u64) -> (Vec<Event>, u64) {
    let Ok(mut file) = std::fs::File::open(path) else {
        return (Vec::new(), 0);
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = if len < offset { 0 } else { offset };
    if len == start || file.seek(SeekFrom::Start(start)).is_err() {
        return (Vec::new(), len);
    }
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    // Only whole lines; a partial last line waits for the next read.
    let complete = text.rfind('\n').map_or(0, |i| i + 1);
    let events = text[..complete].lines().filter_map(parse).collect();
    let mut next = start + complete as u64;
    if next >= MAX_BYTES && complete == text.len() && std::fs::write(path, "").is_ok() {
        next = 0;
    }
    (events, next)
}

/// The current end of the events file (where reading starts at launch, so
/// old events are not replayed).
pub fn end_of(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn parses_event_lines() {
        // Given
        let lines = [
            "bell\tfk-term-1\t%3",
            "died\tfk-run-api\t%7\n",
            "noise",
            "bell\t\t%1",
        ];

        // When
        let events: Vec<Option<Event>> = lines.iter().map(|l| parse(l)).collect();

        // Then
        assert_eq!(
            events,
            [
                Some(Event {
                    kind: EventKind::Bell,
                    session: "fk-term-1".into(),
                    pane: "%3".into()
                }),
                Some(Event {
                    kind: EventKind::Died,
                    session: "fk-run-api".into(),
                    pane: "%7".into()
                }),
                None,
                None,
            ]
        );
    }

    #[test]
    fn reads_only_new_whole_lines() {
        // Given: two events and half a third
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.log");
        std::fs::write(&path, "bell\ta\t%1\ndied\tb\t%2\nbell\tc").unwrap();

        // When
        let (first, offset) = read_new(&path, 0);
        let (none, same) = read_new(&path, offset);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, b"\t%3\n"))
            .unwrap();
        let (rest, _) = read_new(&path, same);

        // Then
        assert_eq!(first.len(), 2);
        assert!(none.is_empty());
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].session, "c");
    }

    /// The hooks on a real (private) tmux server: a bell and an exit each
    /// land in the file.
    #[test]
    fn hooks_report_bells_and_exits() {
        if !tmux::available() {
            return;
        }
        // Given
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.log");
        let server = Server::Named(format!("fubbik-test-events-{}", std::process::id()));
        tmux::command(
            &server,
            &["-f", "/dev/null", "new-session", "-d", "-s", "keep"],
        )
        .unwrap();
        install(&server, &path).unwrap();

        // When: a bell, then a command that exits
        tmux::command(
            &server,
            &["send-keys", "-t", "keep", "printf '\\a'", "Enter"],
        )
        .unwrap();
        tmux::command(
            &server,
            &[
                "new-session",
                "-d",
                "-s",
                "run",
                "sleep 0.2; exit 3",
                ";",
                "set-option",
                "-w",
                "-t",
                "run",
                "remain-on-exit",
                "on",
            ],
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut events = Vec::new();
        let mut offset = 0;
        while events.len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            let (new, next) = read_new(&path, offset);
            events.extend(new);
            offset = next;
        }
        tmux::kill_server(&server);

        // Then
        assert!(
            events
                .iter()
                .any(|e| e.kind == EventKind::Bell && e.session == "keep"),
            "{events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| e.kind == EventKind::Died && e.session == "run"),
            "{events:?}"
        );
    }
}
