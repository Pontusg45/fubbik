//! What changed, for agents that keep track of the projects: an in-memory
//! feed of events with increasing sequence numbers, read with
//! `get_changes_since(cursor)`.
//!
//! Events come from three places:
//! - comparing each published view with the previous one (commits, branch
//!   switches, processes up or down, agents starting, waiting or ending,
//!   worktrees added or removed, pull requests opened, merged or needing you);
//! - the app pushing them as they happen (a terminal tab's bell);
//! - slow background checks: dependencies that start or stop disagreeing by
//!   a major version (every 10 min), and tools Homebrew upgraded (every 30).
//!
//! Events are also appended to `~/.config/fubbik/events.jsonl` and read back
//! at start, so a tracker that checks once a day still sees what happened
//! before a restart. Sequence numbers continue after the last saved one, or
//! start at the app's start time in milliseconds × 1000, whichever is later.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Events kept; older ones are dropped (and a reader is told).
const KEEP: usize = 2000;
const DEPS_EVERY: Duration = Duration::from_secs(600);
const TOOLS_EVERY: Duration = Duration::from_secs(1800);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub seq: u64,
    /// Unix seconds.
    pub at: u64,
    pub repo: Option<String>,
    pub kind: String,
    pub text: String,
}

impl Event {
    pub fn to_json(&self) -> Value {
        json!({ "seq": self.seq, "at": self.at, "repo": self.repo, "kind": self.kind, "text": self.text })
    }
}

struct Feed {
    next: u64,
    events: VecDeque<Event>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Where the feed is kept between runs: `~/.config/fubbik/events.jsonl`
/// (beside the tmux config). None in tests.
fn events_file() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(crate::tmux::config_path().parent()?.join("events.jsonl"))
}

impl Event {
    fn from_json(value: &Value) -> Option<Event> {
        Some(Event {
            seq: value["seq"].as_u64()?,
            at: value["at"].as_u64()?,
            repo: value["repo"].as_str().map(str::to_string),
            kind: value["kind"].as_str()?.to_string(),
            text: value["text"].as_str()?.to_string(),
        })
    }
}

/// The last [`KEEP`] events in a JSON-lines file (unreadable lines skipped).
pub fn load(path: &std::path::Path) -> Vec<Event> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let events: Vec<Event> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|v| Event::from_json(&v))
        .collect();
    let skip = events.len().saturating_sub(KEEP);
    events.into_iter().skip(skip).collect()
}

/// Appends one event; when the file holds twice [`KEEP`], it is rewritten
/// with the newest `KEEP`.
pub fn append(path: &std::path::Path, event: &Event, kept: &VecDeque<Event>) {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let lines = std::fs::read_to_string(path)
        .map(|t| t.lines().count())
        .unwrap_or(0);
    if lines >= 2 * KEEP {
        let text: String = kept
            .iter()
            .map(|e| e.to_json().to_string() + "\n")
            .collect();
        let _ = std::fs::write(path, text);
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", event.to_json());
    }
}

fn feed() -> &'static Mutex<Feed> {
    static FEED: OnceLock<Mutex<Feed>> = OnceLock::new();
    FEED.get_or_init(|| {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        // Yesterday's events stay readable; new ones continue after them.
        let events: VecDeque<Event> = events_file().map(|p| load(&p)).unwrap_or_default().into();
        let next = (millis * 1000).max(events.back().map_or(0, |e| e.seq + 1));
        Mutex::new(Feed { next, events })
    })
}

/// Sessions that report through Claude Code hooks, and when they last did:
/// their agent events come from the hooks, so the poll does not repeat them.
fn hooked_sessions() -> &'static Mutex<std::collections::HashMap<String, std::time::Instant>> {
    static HOOKED: OnceLock<Mutex<std::collections::HashMap<String, std::time::Instant>>> =
        OnceLock::new();
    HOOKED.get_or_init(Default::default)
}

pub fn hooked(session: &str) {
    if !session.is_empty() {
        hooked_sessions()
            .lock()
            .unwrap()
            .insert(session.to_string(), std::time::Instant::now());
    }
}

fn is_hooked(session: &str) -> bool {
    hooked_sessions()
        .lock()
        .unwrap()
        .get(session)
        .is_some_and(|at| at.elapsed() < Duration::from_secs(6 * 3600))
}

/// Adds an event now.
pub fn record(repo: Option<&str>, kind: &str, text: impl Into<String>) {
    let mut feed = feed().lock().unwrap();
    let event = Event {
        seq: feed.next,
        at: now_secs(),
        repo: repo.map(str::to_string),
        kind: kind.to_string(),
        text: text.into(),
    };
    feed.next += 1;
    feed.events.push_back(event.clone());
    while feed.events.len() > KEEP {
        feed.events.pop_front();
    }
    if let Some(path) = events_file() {
        append(&path, &event, &feed.events);
    }
    // Watches see it after the feed's lock is released (an alert records
    // an event of its own).
    drop(feed);
    crate::tracker::on_event(&event);
}

/// Events after `cursor` (all kept ones when None), optionally for one
/// repo; at most `limit`. Returns the events, the cursor to pass next, and
/// whether events after `cursor` were already dropped.
pub fn since(cursor: Option<u64>, repo: Option<&str>, limit: usize) -> (Vec<Event>, u64, bool) {
    let feed = feed().lock().unwrap();
    let dropped = match (cursor, feed.events.front()) {
        (Some(c), Some(first)) => c + 1 < first.seq,
        _ => false,
    };
    let events: Vec<Event> = feed
        .events
        .iter()
        .filter(|e| cursor.is_none_or(|c| e.seq > c))
        .filter(|e| repo.is_none_or(|r| e.repo.as_deref() == Some(r)))
        .take(limit)
        .cloned()
        .collect();
    // Past a page: continue after its last event; else after everything.
    let next = match events.last() {
        Some(last) if events.len() == limit => last.seq,
        _ => feed.next.saturating_sub(1).max(cursor.unwrap_or(0)),
    };
    (events, next, dropped)
}

fn by<'a>(list: &'a Value, key: &str) -> BTreeMap<String, &'a Value> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| Some((item[key].as_str()?.to_string(), item)))
        .collect()
}

/// The events between two published views of the repos.
pub fn diff(old: &[Value], new: &[Value]) -> Vec<(String, &'static str, String)> {
    let mut out = Vec::new();
    let old_by: BTreeMap<&str, &Value> = old
        .iter()
        .filter_map(|r| Some((r["name"].as_str()?, r)))
        .collect();
    for repo in new {
        let Some(name) = repo["name"].as_str() else {
            continue;
        };
        let Some(prev) = old_by.get(name) else {
            // First sight of a repo (or of its first poll): a baseline.
            continue;
        };
        if prev["git"].is_null() || repo["git"].is_null() {
            continue;
        }
        let mut push = |kind: &'static str, text: String| out.push((name.to_string(), kind, text));

        // Git: a branch switch, or new commits on the same branch.
        let (branch, prev_branch) = (&repo["git"]["branch"], &prev["git"]["branch"]);
        if branch != prev_branch && !prev_branch.is_null() {
            push(
                "branch_switched",
                format!(
                    "{} → {}",
                    prev_branch.as_str().unwrap_or("?"),
                    branch.as_str().unwrap_or("?")
                ),
            );
        } else if repo["git"]["head"] != prev["git"]["head"] && !prev["git"]["head"].is_null() {
            push(
                "commit",
                format!(
                    "{}: {}",
                    branch.as_str().unwrap_or("?"),
                    repo["git"]["last_commit"].as_str().unwrap_or("new commit")
                ),
            );
        }

        // Processes up or down.
        let prev_procs = by(&prev["processes"], "name");
        for (pname, process) in by(&repo["processes"], "name") {
            let Some(before) = prev_procs.get(&pname) else {
                continue;
            };
            if process["up"] != before["up"] {
                let status = process["status"].as_str().unwrap_or("?");
                if process["up"] == true {
                    push("process_up", format!("{pname} is up ({status})"));
                } else {
                    push("process_down", format!("{pname} went down ({status})"));
                }
            }
        }

        // Agents: started, waiting for the user, ended.
        let prev_sessions = by(&prev["sessions"], "id");
        for (id, session) in by(&repo["sessions"], "id") {
            if is_hooked(&id) {
                continue;
            }
            let title = session["title"].as_str().unwrap_or("?");
            let agent = session["agent"].as_str().unwrap_or("agent");
            match prev_sessions.get(&id) {
                None if session["live"] == true => {
                    push("agent_started", format!("{agent}: {title}"))
                }
                None => {}
                Some(before) => {
                    let (state, was) = (&session["state"], &before["state"]);
                    if state == was {
                        continue;
                    }
                    if state == "Waiting" {
                        push("agent_waiting", format!("{agent} waits for you: {title}"));
                    } else if before["live"] == true && session["live"] == false {
                        push("agent_ended", format!("{agent} ended: {title}"));
                    }
                }
            }
        }

        // Worktrees added or removed.
        let (now_paths, was_paths): (BTreeSet<String>, BTreeSet<String>) = (
            by(&repo["worktrees"], "path").into_keys().collect(),
            by(&prev["worktrees"], "path").into_keys().collect(),
        );
        if !was_paths.is_empty() {
            for path in now_paths.difference(&was_paths) {
                push("worktree_added", path.clone());
            }
            for path in was_paths.difference(&now_paths) {
                push("worktree_removed", path.clone());
            }
        }

        // Pull requests: opened, merged, needing the user.
        let prev_prs = by(&prev["pull_requests"], "branch");
        if !prev["pull_requests"].is_null() {
            for (branch, pr) in by(&repo["pull_requests"], "branch") {
                let summary = pr["summary"].as_str().unwrap_or("PR");
                match prev_prs.get(&branch) {
                    None if pr["state"] == "OPEN" => {
                        push("pr_opened", format!("{branch}: {summary}"))
                    }
                    None => {}
                    Some(before) => {
                        if pr["state"] == "MERGED" && before["state"] != "MERGED" {
                            push("pr_merged", format!("{branch}: {summary}"));
                        } else if !pr["needs_you"].is_null()
                            && pr["needs_you"] != before["needs_you"]
                        {
                            push(
                                "pr_needs_you",
                                format!("{branch}: {}", pr["needs_you"].as_str().unwrap_or("?")),
                            );
                        }
                    }
                }
            }
        }
    }
    out
}

/// Diffs and records; called with every publish.
pub fn on_publish(old: &[Value], new: &[Value]) {
    for (repo, kind, text) in diff(old, new) {
        record(Some(&repo), kind, text);
    }
    // Duration watches (a process down N minutes, an agent waiting).
    crate::tracker::on_state(new);
}

/// Dependencies differing by a major, by name, with each repo's requirement.
fn breaking_deps(paths: &[(String, std::path::PathBuf)]) -> BTreeMap<String, String> {
    let scans: Vec<_> = paths.iter().map(|(_, p)| crate::deps::scan(p)).collect();
    crate::deps::compare(&scans, 2)
        .into_iter()
        .filter(|r| r.align == crate::deps::Align::Breaking)
        .map(|r| {
            let specs: Vec<String> = paths
                .iter()
                .enumerate()
                .filter(|(i, _)| !r.per_repo[*i].is_empty())
                .map(|(i, (name, _))| format!("{name} {}", r.specs(i)))
                .collect();
            (format!("{} ({})", r.name, r.eco.label()), specs.join(", "))
        })
        .collect()
}

/// The background checks (dependencies and Homebrew upgrades); `repos`
/// gives the registered repos' names and paths at each check.
pub fn start_watchers(repos: fn() -> Vec<(String, std::path::PathBuf)>) {
    std::thread::Builder::new()
        .name("mcp-deps-watch".into())
        .spawn(move || {
            let mut before: Option<BTreeMap<String, String>> = None;
            loop {
                let paths = repos();
                if paths.len() >= 2 {
                    let now = breaking_deps(&paths);
                    if let Some(before) = &before {
                        for (name, specs) in &now {
                            if !before.contains_key(name) {
                                record(None, "dependency_misaligned", format!("{name}: {specs}"));
                            }
                        }
                        for name in before.keys().filter(|n| !now.contains_key(*n)) {
                            record(
                                None,
                                "dependency_aligned",
                                format!("{name} now agrees across repos"),
                            );
                        }
                    }
                    before = Some(now);
                }
                std::thread::sleep(DEPS_EVERY);
            }
        })
        .ok();
    std::thread::Builder::new()
        .name("mcp-tools-watch".into())
        .spawn(|| {
            let mut before: Option<BTreeMap<&'static str, Option<String>>> = None;
            loop {
                let now: BTreeMap<&'static str, Option<String>> = crate::tools::report()
                    .tools
                    .into_iter()
                    .map(|t| (t.name, t.version))
                    .collect();
                if let Some(before) = &before {
                    for (name, version) in &now {
                        let was = before.get(name).cloned().flatten();
                        if was != *version {
                            let text = match (&was, version) {
                                (Some(a), Some(b)) => format!("{name} {a} → {b}"),
                                (None, Some(b)) => format!("{name} {b} installed"),
                                (Some(a), None) => format!("{name} {a} is gone"),
                                (None, None) => continue,
                            };
                            record(None, "tool_changed", text);
                        }
                    }
                }
                before = Some(now);
                std::thread::sleep(TOOLS_EVERY);
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(head: &str, process_up: bool, session_state: &str, pr_state: &str) -> Value {
        json!({
            "name": "app",
            "git": { "branch": "main", "head": head, "last_commit": "fix: things (1 minute ago)" },
            "processes": [{ "name": "api", "up": process_up, "status": if process_up { "Running" } else { "Stopped" } }],
            "sessions": [{ "id": "s1", "agent": "Claude Code", "title": "refactor", "state": session_state, "live": session_state != "Ended" }],
            "worktrees": [{ "path": "/x/app" }],
            "pull_requests": [{ "branch": "feat", "state": pr_state, "summary": "PR #3", "needs_you": null }],
        })
    }

    #[test]
    fn diffs_commits_processes_agents_and_prs() {
        // Given: before and after a poll
        let old = vec![repo("aaa", true, "Working", "OPEN")];
        let new = vec![repo("bbb", false, "Waiting", "MERGED")];

        // When
        let events = diff(&old, &new);

        // Then
        let kinds: Vec<&str> = events.iter().map(|(_, k, _)| *k).collect();
        assert_eq!(
            kinds,
            ["commit", "process_down", "agent_waiting", "pr_merged"]
        );
        assert_eq!(events[0].2, "main: fix: things (1 minute ago)");
        assert!(
            diff(&new, &new).is_empty(),
            "nothing changed, nothing to say"
        );
        assert!(diff(&[], &new).is_empty(), "a first view is a baseline");
    }

    #[test]
    fn the_file_keeps_events_across_restarts_and_is_capped() {
        // Given: an events file written past twice the cap
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut kept = VecDeque::new();
        for seq in 0..(2 * KEEP as u64 + 5) {
            let event = Event {
                seq,
                at: 1,
                repo: Some("app".into()),
                kind: "commit".into(),
                text: format!("c{seq}"),
            };
            kept.push_back(event.clone());
            while kept.len() > KEEP {
                kept.pop_front();
            }
            append(&path, &event, &kept);
        }
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, b"not json\n"))
            .unwrap();

        // When
        let loaded = load(&path);
        let lines = std::fs::read_to_string(&path).unwrap().lines().count();

        // Then
        assert_eq!(loaded.len(), KEEP);
        assert_eq!(loaded.last().unwrap().text, format!("c{}", 2 * KEEP + 4));
        assert!(lines < 2 * KEEP + 10, "rewritten once it doubled: {lines}");
    }

    #[test]
    fn reads_the_feed_by_cursor_and_repo() {
        // Given: three events (other tests may add to the shared feed)
        let (_, start, _) = since(None, None, KEEP);
        record(Some("a"), "commit", "one");
        record(Some("b"), "commit", "two");
        record(Some("a"), "bell", "three");

        // When
        let (all, cursor, dropped) = since(Some(start), None, 100);
        let (only_a, _, _) = since(Some(start), Some("a"), 100);
        let (page, page_cursor, _) = since(Some(start), None, 1);
        let (after, _, _) = since(Some(cursor), None, 100);

        // Then
        let texts: Vec<&str> = all.iter().map(|e| e.text.as_str()).collect();
        assert!(texts.ends_with(&["one", "two", "three"]), "{texts:?}");
        assert!(!dropped);
        assert_eq!(
            only_a
                .iter()
                .filter(|e| e.text == "one" || e.text == "three")
                .count(),
            2
        );
        assert_eq!(page.len(), 1);
        assert_eq!(page_cursor, page[0].seq);
        assert!(after.is_empty());
    }
}
