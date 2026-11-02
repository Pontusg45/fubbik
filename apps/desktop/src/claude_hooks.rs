//! Claude Code hooks that tell the app what a session does as it happens:
//! started, a prompt sent (working), stopped (waiting for you), a
//! notification (a permission prompt, idle), ended. Each hook pipes Claude's
//! JSON to the app's `/hook` endpoint with `curl`, times out after a second
//! and never fails, so Claude is never slowed or blocked by the app.
//!
//! Installed into `~/.claude/settings.json` only when asked: merged next to
//! the hooks already there, marked so uninstalling removes exactly these,
//! and the file is backed up first.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// In every command the app installs; how it finds its own entries.
const MARKER: &str = "# fubbik-desktop";

/// The events reported.
pub const EVENTS: [&str; 5] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "SessionEnd",
];

pub fn settings_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".claude/settings.json")
}

/// The hook command for an endpoint (`http://127.0.0.1:3100/hook`).
pub fn command(url: &str) -> String {
    format!(
        "curl -s -m 1 -X POST -H 'content-type: application/json' --data-binary @- '{url}' >/dev/null 2>&1 || true {MARKER}"
    )
}

fn is_ours(entry: &Value) -> bool {
    entry["hooks"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|h| h["command"].as_str().is_some_and(|c| c.contains(MARKER)))
}

/// Settings with the app's hooks added (replacing older ones of its own).
pub fn install(mut settings: Value, url: &str) -> Value {
    settings = uninstall(settings);
    if !settings.is_object() {
        settings = json!({});
    }
    let hooks = settings
        .as_object_mut()
        .expect("an object")
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    for event in EVENTS {
        let list = hooks
            .as_object_mut()
            .expect("an object")
            .entry(event)
            .or_insert_with(|| json!([]));
        if let Some(list) = list.as_array_mut() {
            list.push(json!({ "hooks": [{ "type": "command", "command": command(url) }] }));
        }
    }
    settings
}

/// Settings without the app's hooks; others untouched, empty lists dropped.
pub fn uninstall(mut settings: Value) -> Value {
    if let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) {
        for list in hooks.values_mut() {
            if let Some(entries) = list.as_array_mut() {
                entries.retain(|e| !is_ours(e));
            }
        }
        hooks.retain(|_, list| list.as_array().is_none_or(|l| !l.is_empty()));
    }
    settings
}

/// Whether the app's hooks are in, for every event.
pub fn installed(settings: &Value) -> bool {
    EVENTS.iter().all(|event| {
        settings["hooks"][event]
            .as_array()
            .is_some_and(|list| list.iter().any(is_ours))
    })
}

fn read(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| format!("{} is not valid JSON: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

fn write(path: &Path, settings: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if path.exists() {
        std::fs::copy(path, path.with_extension("json.fubbik-backup"))
            .map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())? + "\n";
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn install_file(path: &Path, url: &str) -> Result<(), String> {
    write(path, &install(read(path)?, url))
}

pub fn uninstall_file(path: &Path) -> Result<(), String> {
    write(path, &uninstall(read(path)?))
}

pub fn installed_file(path: &Path) -> bool {
    read(path).is_ok_and(|s| installed(&s))
}

/// A hook's report: which event, for which session, in which folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEvent {
    pub event: String,
    pub session: String,
    pub cwd: PathBuf,
    pub message: Option<String>,
}

pub fn parse(body: &Value) -> Option<HookEvent> {
    Some(HookEvent {
        event: body["hook_event_name"].as_str()?.to_string(),
        session: body["session_id"].as_str().unwrap_or_default().to_string(),
        cwd: PathBuf::from(body["cwd"].as_str().unwrap_or_default()),
        message: body["message"].as_str().map(str::to_string),
    })
}

/// The change-feed kind and text for a hook event.
pub fn feed_entry(event: &HookEvent) -> Option<(&'static str, String)> {
    let folder = event
        .cwd
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match event.event.as_str() {
        "SessionStart" => Some(("agent_started", format!("Claude Code started in {folder}"))),
        "UserPromptSubmit" => Some((
            "agent_working",
            format!("Claude Code is working in {folder}"),
        )),
        "Stop" => Some((
            "agent_waiting",
            format!("Claude Code finished its turn in {folder} and waits for you"),
        )),
        "Notification" => Some((
            "agent_waiting",
            format!(
                "Claude Code in {folder}: {}",
                event.message.as_deref().unwrap_or("needs you")
            ),
        )),
        "SessionEnd" => Some(("agent_ended", format!("Claude Code ended in {folder}"))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_next_to_existing_hooks_and_uninstalls_only_its_own() {
        // Given: settings with the user's own hooks and other keys
        let settings = json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "guard.sh" }] }],
                "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }]
            }
        });

        // When
        let once = install(settings.clone(), "http://127.0.0.1:3100/hook");
        let twice = install(once.clone(), "http://127.0.0.1:3199/hook");
        let removed = uninstall(twice.clone());

        // Then
        assert!(installed(&once));
        assert_eq!(once["model"], "opus");
        assert_eq!(
            once["hooks"]["Stop"].as_array().unwrap().len(),
            2,
            "the user's Stop hook stays"
        );
        assert_eq!(
            twice["hooks"]["Stop"].as_array().unwrap().len(),
            2,
            "installing again replaces, not adds"
        );
        assert!(twice.to_string().contains("3199") && !twice.to_string().contains("3100"));
        assert_eq!(removed, settings, "uninstall leaves exactly what was there");
        assert!(!installed(&removed));
    }

    #[test]
    fn files_are_backed_up_and_round_trip() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"hooks":{"PreToolUse":[]},"x":1}"#).unwrap();

        // When
        install_file(&path, "http://127.0.0.1:3100/hook").unwrap();
        let on = installed_file(&path);
        uninstall_file(&path).unwrap();

        // Then
        assert!(on);
        assert!(!installed_file(&path));
        assert!(path.with_extension("json.fubbik-backup").exists());
        let after: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after["x"], 1);
    }

    #[test]
    fn hook_bodies_become_feed_entries() {
        // Given
        let body = json!({ "hook_event_name": "Notification", "session_id": "s1", "cwd": "/u/projects/fubbik", "message": "Claude needs your permission to use Bash" });

        // When
        let event = parse(&body).unwrap();
        let (kind, text) = feed_entry(&event).unwrap();

        // Then
        assert_eq!(event.session, "s1");
        assert_eq!(kind, "agent_waiting");
        assert_eq!(
            text,
            "Claude Code in fubbik: Claude needs your permission to use Bash"
        );
        assert!(parse(&json!({})).is_none());
        assert!(command("http://x/hook").ends_with("|| true # fubbik-desktop"));
    }
}
