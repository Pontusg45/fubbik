//! Notes and watches for an agent (or the user) keeping track of projects.
//!
//! - **Notes:** notes, todos and decisions, per repo or global. Open todos show
//!   on Today under "Next up" until done.
//! - **Watches:** rules the app checks while it runs, so the tracker does not
//!   have to: an event in the change feed (by kind, repo, text), a process
//!   down for N minutes, an agent waiting for N minutes. A watch that fires
//!   raises an alert: a "Needs you" line on Today until dismissed, a toast,
//!   and a `watch_fired` event in the feed.
//!
//! Kept in `~/.config/fubbik/tracker.json`.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::mcp_events::Event;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub repo: Option<String>,
    /// `note`, `todo` or `decision`.
    pub kind: String,
    pub text: String,
    pub at: u64,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Rule {
    /// A change-feed event: its kind (`process_down`, `dependency_misaligned`,
    /// `tool_changed`…) and/or text containing a phrase.
    Event {
        kind: Option<String>,
        contains: Option<String>,
    },
    /// A process (by name) not up for this long.
    ProcessDown { process: String, minutes: u64 },
    /// Any agent waiting for the user this long.
    AgentWaiting { minutes: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch {
    pub id: String,
    pub repo: Option<String>,
    pub rule: Rule,
    /// What it is for, in the words of whoever set it.
    pub label: String,
    pub created: u64,
}

/// A watch that fired, until dismissed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub watch: String,
    pub repo: Option<String>,
    pub text: String,
    pub at: u64,
    #[serde(default)]
    pub dismissed: bool,
    /// What fired it (a session or process), so one episode alerts once.
    #[serde(default)]
    pub key: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub watches: Vec<Watch>,
    #[serde(default)]
    pub alerts: Vec<Alert>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A short unique id (`n-1a2b3c`).
fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{:x}", (nanos ^ (n << 48)) & 0xff_ffff_ffff)
}

impl Store {
    pub fn add_note(
        &mut self,
        repo: Option<String>,
        kind: &str,
        text: &str,
    ) -> Result<Note, String> {
        if text.trim().is_empty() {
            return Err("a note needs text".into());
        }
        if !matches!(kind, "note" | "todo" | "decision") {
            return Err(format!(
                "kind must be note, todo or decision, not \"{kind}\""
            ));
        }
        let note = Note {
            id: new_id("n"),
            repo,
            kind: kind.into(),
            text: text.trim().into(),
            at: now(),
            done: false,
        };
        self.notes.push(note.clone());
        Ok(note)
    }

    pub fn complete_note(&mut self, id: &str) -> Result<(), String> {
        let note = self
            .notes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| format!("no note {id}"))?;
        note.done = true;
        Ok(())
    }

    pub fn add_watch(
        &mut self,
        repo: Option<String>,
        rule: Rule,
        label: &str,
    ) -> Result<Watch, String> {
        match &rule {
            Rule::Event {
                kind: None,
                contains: None,
            } => {
                return Err("an event watch needs an event kind, a phrase, or both".into());
            }
            Rule::ProcessDown { process, .. } if process.trim().is_empty() => {
                return Err("a process_down watch needs the process name".into());
            }
            _ => {}
        }
        let watch = Watch {
            id: new_id("w"),
            repo,
            label: if label.trim().is_empty() {
                describe(&rule)
            } else {
                label.trim().into()
            },
            rule,
            created: now(),
        };
        self.watches.push(watch.clone());
        Ok(watch)
    }

    pub fn remove_watch(&mut self, id: &str) -> Result<(), String> {
        let before = self.watches.len();
        self.watches.retain(|w| w.id != id);
        if self.watches.len() == before {
            return Err(format!("no watch {id}"));
        }
        Ok(())
    }

    pub fn dismiss(&mut self, id: &str) -> Result<(), String> {
        let alert = self
            .alerts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("no alert {id}"))?;
        alert.dismissed = true;
        Ok(())
    }

    /// An alert, unless the same watch already has an open one for `key`.
    fn fire(
        &mut self,
        watch: &Watch,
        repo: Option<&str>,
        key: &str,
        text: String,
    ) -> Option<Alert> {
        if self
            .alerts
            .iter()
            .any(|a| !a.dismissed && a.watch == watch.id && a.key == key)
        {
            return None;
        }
        let alert = Alert {
            id: new_id("a"),
            watch: watch.id.clone(),
            repo: repo.map(str::to_string),
            text,
            at: now(),
            dismissed: false,
            key: key.to_string(),
        };
        self.alerts.push(alert.clone());
        // Old dismissed alerts go.
        let dismissed = self.alerts.iter().filter(|a| a.dismissed).count();
        if dismissed > 200 {
            let mut drop = dismissed - 200;
            self.alerts.retain(|a| {
                if a.dismissed && drop > 0 {
                    drop -= 1;
                    false
                } else {
                    true
                }
            });
        }
        Some(alert)
    }

    /// Event watches matching a feed event.
    pub fn on_event(&mut self, event: &Event) -> Vec<Alert> {
        if event.kind == "watch_fired" {
            return Vec::new();
        }
        let watches = self.watches.clone();
        watches
            .iter()
            .filter(|w| w.repo.is_none() || w.repo == event.repo)
            .filter(|w| match &w.rule {
                Rule::Event { kind, contains } => {
                    kind.as_ref().is_none_or(|k| *k == event.kind)
                        && contains
                            .as_ref()
                            .is_none_or(|c| event.text.to_lowercase().contains(&c.to_lowercase()))
                }
                _ => false,
            })
            .filter_map(|w| {
                let repo = event.repo.as_deref();
                let text = format!(
                    "{}: {}{}",
                    w.label,
                    repo.map(|r| format!("{r} — ")).unwrap_or_default(),
                    event.text
                );
                // Each event is its own episode.
                self.fire(w, repo, &event.seq.to_string(), text)
            })
            .collect()
    }

    /// Duration watches against the published repos (`list_repos` shape with
    /// processes and sessions). `episodes` remembers, between calls, when a
    /// process went down and which waits already alerted, so one episode
    /// alerts once (even after its alert is dismissed).
    pub fn on_state(&mut self, repos: &[Value], episodes: &mut Episodes, at: u64) -> Vec<Alert> {
        let watches = self.watches.clone();
        let mut out = Vec::new();
        let mut live: HashSet<String> = HashSet::new();
        for repo in repos {
            let Some(name) = repo["name"].as_str() else {
                continue;
            };
            for watch in watches
                .iter()
                .filter(|w| w.repo.as_deref().is_none_or(|r| r == name))
            {
                match &watch.rule {
                    Rule::ProcessDown { process, minutes } => {
                        let Some(p) = repo["processes"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|p| p["name"] == process.as_str())
                        else {
                            continue;
                        };
                        if p["up"] == true {
                            continue;
                        }
                        let key = format!("{}|{name}|{process}", watch.id);
                        live.insert(key.clone());
                        let since = *episodes.since.entry(key.clone()).or_insert(at);
                        if at.saturating_sub(since) >= minutes * 60
                            && episodes.alerted.insert(key.clone())
                        {
                            out.extend(self.fire(
                                watch,
                                Some(name),
                                &key,
                                format!(
                                    "{}: {name} — {process} has been down {minutes}+ min",
                                    watch.label
                                ),
                            ));
                        }
                    }
                    Rule::AgentWaiting { minutes } => {
                        for session in repo["sessions"].as_array().into_iter().flatten() {
                            if session["state"] != "Waiting" {
                                continue;
                            }
                            let id = session["id"].as_str().unwrap_or_default();
                            let key = format!("{}|{id}", watch.id);
                            live.insert(key.clone());
                            let idle = session["idle_for_seconds"].as_u64().unwrap_or(0);
                            if idle >= minutes * 60 && episodes.alerted.insert(key.clone()) {
                                out.extend(self.fire(
                                    watch,
                                    Some(name),
                                    &key,
                                    format!(
                                        "{}: {name} — {} has waited {} min: {}",
                                        watch.label,
                                        session["agent"].as_str().unwrap_or("an agent"),
                                        idle / 60,
                                        session["title"].as_str().unwrap_or("")
                                    ),
                                ));
                            }
                        }
                    }
                    Rule::Event { .. } => {}
                }
            }
        }
        // A process back up, or an agent no longer waiting, ends its episode.
        episodes.since.retain(|k, _| live.contains(k));
        episodes.alerted.retain(|k| live.contains(k));
        out
    }
}

/// Episodes of the duration watches, between checks.
#[derive(Debug, Default)]
pub struct Episodes {
    /// When each watched process was first seen down.
    pub since: HashMap<String, u64>,
    /// Episodes that already alerted.
    pub alerted: HashSet<String>,
}

/// "process_down api 5 min", for a watch without a label.
pub fn describe(rule: &Rule) -> String {
    match rule {
        Rule::Event { kind, contains } => match (kind, contains) {
            (Some(k), Some(c)) => format!("{k} mentioning \"{c}\""),
            (Some(k), None) => k.clone(),
            (None, Some(c)) => format!("anything mentioning \"{c}\""),
            (None, None) => "any event".into(),
        },
        Rule::ProcessDown { process, minutes } => format!("{process} down {minutes}+ min"),
        Rule::AgentWaiting { minutes } => format!("an agent waiting {minutes}+ min"),
    }
}

// ── the app's store ────────────────────────────────────────────────────

fn store_file() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(crate::tmux::config_path().parent()?.join("tracker.json"))
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        let loaded = store_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
}

fn save(store: &Store) {
    if let Some(path) = store_file() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(store) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
    }
}

/// Changes the store, saves it, and records the alerts it fired in the feed
/// (after the store's lock is released: the feed calls back into here).
fn with_store<T>(change: impl FnOnce(&mut Store) -> (T, Vec<Alert>)) -> T {
    let (value, alerts) = {
        let mut store = store().lock().unwrap();
        let before = store.clone();
        let result = change(&mut store);
        if *store != before {
            save(&store);
        }
        result
    };
    for alert in alerts {
        crate::mcp_events::record(alert.repo.as_deref(), "watch_fired", alert.text);
    }
    value
}

/// A snapshot for the UI.
pub fn snapshot() -> Store {
    store().lock().unwrap().clone()
}

pub fn on_event(event: &Event) {
    if event.kind == "watch_fired" {
        return;
    }
    with_store(|s| ((), s.on_event(event)));
}

pub fn on_state(repos: &[Value]) {
    static EPISODES: OnceLock<Mutex<Episodes>> = OnceLock::new();
    let mut episodes = EPISODES.get_or_init(Default::default).lock().unwrap();
    with_store(|s| ((), s.on_state(repos, &mut episodes, now())));
}

pub fn complete_note(id: &str) -> Result<(), String> {
    with_store(|s| (s.complete_note(id), Vec::new()))
}

pub fn dismiss(id: &str) -> Result<(), String> {
    with_store(|s| (s.dismiss(id), Vec::new()))
}

// ── MCP tools ──────────────────────────────────────────────────────────

pub fn mcp_definitions() -> Vec<Value> {
    let repo = json!({ "type": "string", "description": "Repo name (as list_repos shows); leave out for all repos" });
    vec![
        json!({
            "name": "add_note",
            "description": "Saves a note, todo or decision in the app (per repo or global). Open todos show on the app's Today list under \"Next up\" until done. For knowledge worth keeping with the code, write a fubbik chunk instead.",
            "inputSchema": { "type": "object", "properties": {
                "text": { "type": "string" },
                "repo": repo.clone(),
                "kind": { "type": "string", "enum": ["note", "todo", "decision"], "description": "Default note" }
            }, "required": ["text"] }
        }),
        json!({
            "name": "list_notes",
            "description": "Notes, todos and decisions, newest first.",
            "inputSchema": { "type": "object", "properties": {
                "repo": repo.clone(),
                "kind": { "type": "string", "enum": ["note", "todo", "decision"] },
                "include_done": { "type": "boolean", "description": "Also finished todos (default false)" }
            } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "complete_note",
            "description": "Marks a todo (or any note) done.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] }
        }),
        json!({
            "name": "add_watch",
            "description": "Asks the app to watch for something while it runs, so you don't have to poll: `event` fires on change-feed events (see get_changes_since) by `event_kind` (e.g. process_down, agent_waiting, pr_needs_you, dependency_misaligned, tool_changed, commit) and/or text `contains`; `process_down` fires when a process has been down `minutes`; `agent_waiting` when an agent has waited `minutes`. A firing watch raises an alert: a \"Needs you\" line on Today, a toast, and a watch_fired event.",
            "inputSchema": { "type": "object", "properties": {
                "type": { "type": "string", "enum": ["event", "process_down", "agent_waiting"] },
                "repo": repo.clone(),
                "event_kind": { "type": "string" },
                "contains": { "type": "string", "description": "Case-insensitive phrase in the event text, e.g. \"vitest\"" },
                "process": { "type": "string", "description": "For process_down: the process name (get_repo)" },
                "minutes": { "type": "integer", "description": "For process_down and agent_waiting (default 5)" },
                "label": { "type": "string", "description": "What it is for (shown in alerts)" }
            }, "required": ["type"] }
        }),
        json!({
            "name": "list_watches",
            "description": "The watches the app keeps, with their ids.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "remove_watch",
            "description": "Stops a watch.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] }
        }),
        json!({
            "name": "list_alerts",
            "description": "Alerts raised by watches, open ones (not dismissed) by default.",
            "inputSchema": { "type": "object", "properties": {
                "include_dismissed": { "type": "boolean" }
            } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "dismiss_alert",
            "description": "Dismisses an alert (it leaves Today).",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] }
        }),
    ]
}

/// Answers the tools defined here; None for any other name.
pub fn mcp_call(name: &str, args: &Value) -> Option<Result<Value, String>> {
    call_in(&mut |change| with_store(|s| change(s)), name, args)
}

fn to_json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_default()
}

/// One change to the store: its answer, and the alerts it fired.
type StoreChange<'a> = dyn FnMut(&mut Store) -> (Result<Value, String>, Vec<Alert>) + 'a;
/// Runs a change against the store (the app's, or a test's).
type Change<'a> = &'a mut dyn FnMut(&mut StoreChange<'_>) -> Result<Value, String>;

fn call_in(apply: Change<'_>, name: &str, args: &Value) -> Option<Result<Value, String>> {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).map(str::to_string);
    let result = match name {
        "add_note" => apply(&mut |s| {
            let text = arg("text").unwrap_or_default();
            let kind = arg("kind").unwrap_or_else(|| "note".into());
            (
                s.add_note(arg("repo"), &kind, &text).map(|n| to_json(&n)),
                Vec::new(),
            )
        }),
        "list_notes" => apply(&mut |s| {
            let include_done = args["include_done"].as_bool().unwrap_or(false);
            let mut notes: Vec<&Note> = s
                .notes
                .iter()
                .filter(|n| include_done || !n.done)
                .filter(|n| arg("repo").is_none_or(|r| n.repo.as_deref() == Some(r.as_str())))
                .filter(|n| arg("kind").is_none_or(|k| n.kind == k))
                .collect();
            notes.sort_by_key(|n| std::cmp::Reverse(n.at));
            (
                Ok(Value::Array(notes.into_iter().map(to_json).collect())),
                Vec::new(),
            )
        }),
        "complete_note" => apply(&mut |s| {
            let id = arg("id").unwrap_or_default();
            (
                s.complete_note(&id).map(|_| json!({ "done": id })),
                Vec::new(),
            )
        }),
        "add_watch" => apply(&mut |s| {
            let minutes = args["minutes"].as_u64().unwrap_or(5).max(1);
            let rule = match arg("type").as_deref() {
                Some("event") => Rule::Event {
                    kind: arg("event_kind"),
                    contains: arg("contains"),
                },
                Some("process_down") => Rule::ProcessDown {
                    process: arg("process").unwrap_or_default(),
                    minutes,
                },
                Some("agent_waiting") => Rule::AgentWaiting { minutes },
                other => {
                    return (
                        Err(format!(
                            "type must be event, process_down or agent_waiting, not {other:?}"
                        )),
                        Vec::new(),
                    );
                }
            };
            let label = arg("label").unwrap_or_default();
            (
                s.add_watch(arg("repo"), rule, &label).map(|w| to_json(&w)),
                Vec::new(),
            )
        }),
        "list_watches" => apply(&mut |s| {
            (
                Ok(Value::Array(s.watches.iter().map(to_json).collect())),
                Vec::new(),
            )
        }),
        "remove_watch" => apply(&mut |s| {
            let id = arg("id").unwrap_or_default();
            (
                s.remove_watch(&id).map(|_| json!({ "removed": id })),
                Vec::new(),
            )
        }),
        "list_alerts" => apply(&mut |s| {
            let all = args["include_dismissed"].as_bool().unwrap_or(false);
            let alerts: Vec<Value> = s
                .alerts
                .iter()
                .rev()
                .filter(|a| all || !a.dismissed)
                .map(to_json)
                .collect();
            (Ok(Value::Array(alerts)), Vec::new())
        }),
        "dismiss_alert" => apply(&mut |s| {
            let id = arg("id").unwrap_or_default();
            (
                s.dismiss(&id).map(|_| json!({ "dismissed": id })),
                Vec::new(),
            )
        }),
        _ => return None,
    };
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: u64, repo: &str, kind: &str, text: &str) -> Event {
        Event {
            seq,
            at: 0,
            repo: Some(repo.into()),
            kind: kind.into(),
            text: text.into(),
        }
    }

    #[test]
    fn event_watches_fire_once_per_event_and_respect_repo() {
        // Given
        let mut store = Store::default();
        store
            .add_watch(
                None,
                Rule::Event {
                    kind: Some("dependency_misaligned".into()),
                    contains: Some("VITEST".into()),
                },
                "vitest drift",
            )
            .unwrap();
        store
            .add_watch(
                Some("api".into()),
                Rule::Event {
                    kind: Some("commit".into()),
                    contains: None,
                },
                "",
            )
            .unwrap();

        // When
        let vitest = store.on_event(&event(
            1,
            "x",
            "dependency_misaligned",
            "vitest (npm): a 3, b 5",
        ));
        let again = store.on_event(&event(
            1,
            "x",
            "dependency_misaligned",
            "vitest (npm): a 3, b 5",
        ));
        let other_repo = store.on_event(&event(2, "web", "commit", "main: fix"));
        let right_repo = store.on_event(&event(3, "api", "commit", "main: fix"));
        let loop_guard = store.on_event(&event(4, "api", "watch_fired", "commit"));

        // Then
        assert_eq!(vitest.len(), 1);
        assert!(
            vitest[0].text.starts_with("vitest drift: x — vitest"),
            "{}",
            vitest[0].text
        );
        assert!(again.is_empty(), "the same event alerts once");
        assert!(other_repo.is_empty());
        assert_eq!(right_repo.len(), 1);
        assert_eq!(
            right_repo[0].text, "commit: api — main: fix",
            "unlabelled watches describe themselves"
        );
        assert!(loop_guard.is_empty());
    }

    #[test]
    fn duration_watches_wait_then_alert_once_per_episode() {
        // Given: api down, an agent waiting 12 minutes
        let mut store = Store::default();
        store
            .add_watch(
                Some("app".into()),
                Rule::ProcessDown {
                    process: "api".into(),
                    minutes: 5,
                },
                "api",
            )
            .unwrap();
        store
            .add_watch(None, Rule::AgentWaiting { minutes: 10 }, "stuck")
            .unwrap();
        let repos = |up: bool| {
            vec![json!({
                "name": "app",
                "processes": [{ "name": "api", "up": up }],
                "sessions": [{ "id": "s1", "agent": "Claude Code", "title": "t", "state": "Waiting", "idle_for_seconds": 720 }],
            })]
        };
        let mut down = Episodes::default();

        // When
        let first = store.on_state(&repos(false), &mut down, 1000);
        let four_min = store.on_state(&repos(false), &mut down, 1000 + 240);
        let six_min = store.on_state(&repos(false), &mut down, 1000 + 360);
        let still = store.on_state(&repos(false), &mut down, 1000 + 600);
        store.on_state(&repos(true), &mut down, 1000 + 700);
        let new_episode = store.on_state(&repos(false), &mut down, 2000);

        // Then
        let kinds = |alerts: &[Alert]| alerts.iter().map(|a| a.text.clone()).collect::<Vec<_>>();
        assert_eq!(
            kinds(&first).len(),
            1,
            "the agent alert: {:?}",
            kinds(&first)
        );
        assert!(first[0].text.contains("waited 12 min"));
        assert!(four_min.is_empty());
        assert_eq!(kinds(&six_min), ["api: app — api has been down 5+ min"]);
        assert!(still.is_empty(), "one alert per episode");
        assert!(
            new_episode.is_empty(),
            "back up, then down again: the clock restarts"
        );
        assert_eq!(down.since.len(), 1);
        // A new episode alerts again, once the last alert was dismissed (an
        // open one already says it).
        store.dismiss(&six_min[0].id).unwrap();
        let later = store.on_state(&repos(false), &mut down, 2000 + 300);
        assert_eq!(later.len(), 1, "a new episode alerts again");
    }

    #[test]
    fn notes_and_tools_validate_and_list() {
        // Given
        let mut store = Store::default();
        let mut apply = |change: &mut StoreChange<'_>| change(&mut store).0;

        // When
        let added = call_in(
            &mut apply,
            "add_note",
            &json!({ "text": "bump vitest", "kind": "todo", "repo": "app" }),
        )
        .unwrap()
        .unwrap();
        let empty = call_in(&mut apply, "add_note", &json!({ "text": " " })).unwrap();
        let bad_kind = call_in(
            &mut apply,
            "add_note",
            &json!({ "text": "x", "kind": "idea" }),
        )
        .unwrap();
        let id = added["id"].as_str().unwrap().to_string();
        call_in(&mut apply, "complete_note", &json!({ "id": id }))
            .unwrap()
            .unwrap();
        let open = call_in(&mut apply, "list_notes", &json!({}))
            .unwrap()
            .unwrap();
        let all = call_in(&mut apply, "list_notes", &json!({ "include_done": true }))
            .unwrap()
            .unwrap();
        let bad_watch = call_in(&mut apply, "add_watch", &json!({ "type": "event" })).unwrap();
        let watch = call_in(
            &mut apply,
            "add_watch",
            &json!({ "type": "process_down", "process": "api", "minutes": 3 }),
        )
        .unwrap()
        .unwrap();

        // Then
        assert!(empty.is_err() && bad_kind.is_err() && bad_watch.is_err());
        assert_eq!(open, json!([]));
        assert_eq!(all[0]["done"], true);
        assert_eq!(watch["label"], "api down 3+ min");
        assert!(call_in(&mut apply, "nope", &json!({})).is_none());
    }
}
