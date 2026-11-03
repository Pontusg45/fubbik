//! AI coding sessions that ran in a repo or one of its worktrees.
//!
//! - Claude Code: live sessions register in `~/.claude/sessions/<pid>.json`
//!   (with a busy/idle status); transcripts live in
//!   `~/.claude/projects/<cwd with every non-alphanumeric char as '-'>/<id>.jsonl`,
//!   carrying `ai-title` / `custom-title` lines.
//! - Codex: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`, whose first line is
//!   `session_meta` (id, cwd, git branch); titles in `~/.codex/session_index.jsonl`.
//!
//! Transcripts are append-only, so each is read incrementally from where the
//! last scan stopped. Only metadata lines are parsed; message content is not.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::Value;

/// Recent sessions kept per repo.
pub const RECENT: usize = 25;
/// A Codex transcript written to this recently counts as active.
const CODEX_ACTIVE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Agent {
    ClaudeCode,
    Codex,
}

impl Agent {
    pub fn label(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    /// The shell command that resumes a session.
    pub fn resume_command(self, id: &str) -> String {
        match self {
            Agent::ClaudeCode => format!("claude --resume {id}"),
            Agent::Codex => format!("codex resume {id}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Working on a turn.
    Busy,
    /// Open and waiting for input.
    Idle,
    /// Recently written to, state unknown (Codex).
    Active,
    Ended,
}

impl SessionState {
    pub fn label(self) -> &'static str {
        match self {
            SessionState::Busy => "Working",
            SessionState::Idle => "Waiting",
            SessionState::Active => "Recently active",
            SessionState::Ended => "Ended",
        }
    }

    pub fn is_live(self) -> bool {
        self != SessionState::Ended
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSession {
    pub agent: Agent,
    pub id: String,
    pub title: String,
    pub cwd: PathBuf,
    pub branch: Option<String>,
    pub state: SessionState,
    pub pid: Option<u32>,
    pub last_activity: SystemTime,
    /// The tmux pane it runs in, on the app's server or the user's.
    pub pane: Option<crate::tmux::Pane>,
    /// The last lines on that pane's screen (live sessions only).
    pub preview: Option<String>,
    /// Tokens used so far (Claude Code: input, cache and output).
    pub tokens: Option<u64>,
}

/// Claude Code's project folder name for a cwd.
pub fn claude_dir_name(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// `path` is `root` or inside it, compared by components.
pub fn is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct TranscriptMeta {
    offset: u64,
    cwd: Option<PathBuf>,
    branch: Option<String>,
    ai_title: Option<String>,
    custom_title: Option<String>,
    /// Tokens across the transcript's assistant messages.
    tokens: u64,
    /// The last message ids counted: a message's lines are next to each
    /// other, so a short window stops one being counted twice.
    counted: std::collections::VecDeque<String>,
}

#[derive(Debug, Clone)]
struct CodexMeta {
    id: String,
    cwd: PathBuf,
    branch: Option<String>,
    /// The start of the first real prompt: the title when the index has none.
    first_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LiveClaude {
    pub pid: u32,
    pub session_id: String,
    pub cwd: PathBuf,
    pub name: String,
    pub busy: bool,
}

/// Holds per-file read positions between scans. Shared across polls.
pub struct SessionIndex {
    claude_home: PathBuf,
    codex_home: PathBuf,
    transcripts: HashMap<PathBuf, TranscriptMeta>,
    codex: HashMap<PathBuf, Option<CodexMeta>>,
    codex_titles: HashMap<String, String>,
    codex_index_mtime: Option<SystemTime>,
    /// Where read positions are kept between launches (None in tests), so a
    /// restart reads only what was appended instead of every transcript.
    cache: Option<PathBuf>,
    dirty: bool,
    saved: Option<std::time::Instant>,
}

/// Saves at most this often.
const SAVE_EVERY: Duration = Duration::from_secs(20);

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn str_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// Folds one transcript line into `meta`. Only lines that look like metadata
/// are parsed as JSON.
fn absorb_line(meta: &mut TranscriptMeta, line: &str) {
    let wants_cwd = meta.cwd.is_none() && line.contains("\"cwd\"");
    let is_title = line.contains("\"ai-title\"") || line.contains("\"custom-title\"");
    let has_usage = line.contains("\"usage\"") && line.contains("\"assistant\"");
    if !wants_cwd && !is_title && !has_usage {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if wants_cwd && let Some(cwd) = str_field(&value, "cwd") {
        meta.cwd = Some(PathBuf::from(cwd));
        meta.branch = str_field(&value, "gitBranch");
    }
    if has_usage
        && let Some(message) = value.get("message")
        && let Some(usage) = message.get("usage")
    {
        let id = str_field(message, "id").unwrap_or_else(|| line.len().to_string());
        if !meta.counted.contains(&id) {
            meta.counted.push_back(id);
            if meta.counted.len() > COUNTED_WINDOW {
                meta.counted.pop_front();
            }
            meta.tokens += [
                "input_tokens",
                "cache_creation_input_tokens",
                "cache_read_input_tokens",
                "output_tokens",
            ]
            .iter()
            .filter_map(|k| usage.get(*k).and_then(Value::as_u64))
            .sum::<u64>();
        }
    }
    match value.get("type").and_then(Value::as_str) {
        Some("ai-title") => meta.ai_title = str_field(&value, "aiTitle").or(meta.ai_title.take()),
        Some("custom-title") => {
            meta.custom_title = str_field(&value, "customTitle").or(meta.custom_title.take())
        }
        _ => {}
    }
}

/// Message ids remembered per transcript (see `TranscriptMeta::counted`).
const COUNTED_WINDOW: usize = 64;

/// Reads complete lines appended since `meta.offset`.
fn read_transcript(path: &Path, meta: &mut TranscriptMeta) {
    let Ok(mut file) = std::fs::File::open(path) else {
        return;
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len < meta.offset {
        *meta = TranscriptMeta::default(); // rewritten: start over
    }
    if file.seek(SeekFrom::Start(meta.offset)).is_err() {
        return;
    }
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(n) if !line.ends_with('\n') => {
                let _ = n; // partial last line: read it next time
                break;
            }
            Ok(n) => {
                meta.offset += n as u64;
                absorb_line(meta, &line);
            }
        }
    }
}

pub fn parse_live_claude(text: &str) -> Option<LiveClaude> {
    let value: Value = serde_json::from_str(text).ok()?;
    Some(LiveClaude {
        pid: value.get("pid")?.as_u64()? as u32,
        session_id: str_field(&value, "sessionId")?,
        cwd: PathBuf::from(str_field(&value, "cwd")?),
        name: str_field(&value, "name").unwrap_or_default(),
        busy: value.get("status").and_then(Value::as_str) == Some("busy"),
    })
}

fn parse_codex_meta(first_line: &str) -> Option<CodexMeta> {
    let value: Value = serde_json::from_str(first_line).ok()?;
    if value.get("type")?.as_str()? != "session_meta" {
        return None;
    }
    let payload = value.get("payload")?;
    Some(CodexMeta {
        id: str_field(payload, "id")?,
        cwd: PathBuf::from(str_field(payload, "cwd")?),
        branch: payload.get("git").and_then(|git| str_field(git, "branch")),
        first_prompt: None,
    })
}

const TITLE_CHARS: usize = 80;

/// The user's text in a Codex line, skipping the injected `AGENTS.md` and
/// `<environment_context>` blocks. Older logs use `event_msg/user_message`.
fn codex_prompt(line: &str) -> Option<String> {
    if !line.contains("\"user") {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    let payload = value.get("payload")?;
    let texts: Vec<String> = match (value.get("type")?.as_str()?, payload.get("type")?.as_str()?) {
        ("event_msg", "user_message") => vec![str_field(payload, "message")?],
        ("response_item", "message") if payload.get("role")?.as_str()? == "user" => payload
            .get("content")?
            .as_array()?
            .iter()
            .filter(|c| c.get("type").and_then(Value::as_str) == Some("input_text"))
            .filter_map(|c| str_field(c, "text"))
            .collect(),
        _ => return None,
    };
    texts
        .into_iter()
        .map(|t| t.trim().to_string())
        .find(|t| !t.is_empty() && !t.starts_with('<') && !t.starts_with("# AGENTS.md"))
        .map(|t| {
            let first = t.lines().next().unwrap_or_default();
            let mut title: String = first.chars().take(TITLE_CHARS).collect();
            if first.chars().count() > TITLE_CHARS {
                title.push('…');
            }
            title
        })
}

/// The meta line, then lines up to the first prompt (within 2 MiB).
fn read_codex_meta(path: &Path) -> Option<CodexMeta> {
    let file = std::fs::File::open(path).ok()?;
    let mut lines = BufReader::new(file.take(2 << 20)).lines();
    let mut meta = parse_codex_meta(&lines.next()?.ok()?)?;
    meta.first_prompt = lines
        .map_while(Result::ok)
        .find_map(|line| codex_prompt(&line));
    Some(meta)
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn walk_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_jsonl(&path, out);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            out.push(path);
        }
    }
}

impl SessionIndex {
    pub fn new() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let mut index = Self::with_homes(crate::claude_hooks::claude_dir(), home.join(".codex"));
        index.cache = Some(crate::backend::data_dir().join("session-index.json"));
        index.load();
        index
    }

    fn load(&mut self) {
        let Some(text) = self
            .cache
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
        else {
            return;
        };
        if let Ok(saved) = serde_json::from_str::<Vec<(PathBuf, TranscriptMeta)>>(&text) {
            self.transcripts.extend(saved);
        }
    }

    fn save_if_due(&mut self) {
        let Some(path) = self.cache.clone() else {
            return;
        };
        if !self.dirty || self.saved.is_some_and(|at| at.elapsed() < SAVE_EVERY) {
            return;
        }
        // Only transcripts that still exist.
        let entries: Vec<(&PathBuf, &TranscriptMeta)> = self
            .transcripts
            .iter()
            .filter(|(p, _)| p.exists())
            .collect();
        if let Ok(text) = serde_json::to_string(&entries) {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
        self.dirty = false;
        self.saved = Some(std::time::Instant::now());
    }

    pub fn with_homes(claude_home: PathBuf, codex_home: PathBuf) -> Self {
        Self {
            claude_home,
            codex_home,
            transcripts: HashMap::new(),
            codex: HashMap::new(),
            codex_titles: HashMap::new(),
            codex_index_mtime: None,
            cache: None,
            dirty: false,
            saved: None,
        }
    }

    fn live_claude(&self) -> Vec<LiveClaude> {
        let Ok(entries) = std::fs::read_dir(self.claude_home.join("sessions")) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter_map(|text| parse_live_claude(&text))
            .filter(|live| pid_alive(live.pid))
            .collect()
    }

    fn claude_sessions(&mut self, roots: &[PathBuf]) -> Vec<AiSession> {
        let projects = self.claude_home.join("projects");
        let prefixes: Vec<String> = roots.iter().map(|r| claude_dir_name(r)).collect();
        let mut files: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(&projects)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                prefixes
                    .iter()
                    .any(|p| name == *p || name.starts_with(&format!("{p}-")))
            })
            .flat_map(|e| std::fs::read_dir(e.path()).into_iter().flatten().flatten())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .filter_map(|p| mtime(&p).map(|m| (p, m)))
            .collect();
        files.sort_by_key(|(_, m)| std::cmp::Reverse(*m));

        let live = self.live_claude();
        let mut sessions = Vec::new();
        for (path, modified) in files {
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let meta = self.transcripts.entry(path.clone()).or_default();
            let before = meta.offset;
            read_transcript(&path, meta);
            if meta.offset != before {
                self.dirty = true;
            }
            let Some(cwd) = meta.cwd.clone() else {
                continue;
            };
            if !roots.iter().any(|root| is_within(&cwd, root)) {
                continue; // a prefix collision: `app` vs `app-other`
            }
            let running = live.iter().find(|l| l.session_id == id);
            sessions.push(AiSession {
                agent: Agent::ClaudeCode,
                title: meta
                    .custom_title
                    .clone()
                    .or(meta.ai_title.clone())
                    .or(running.map(|l| l.name.clone()))
                    .unwrap_or_else(|| "Untitled session".into()),
                cwd,
                branch: meta.branch.clone(),
                state: match running {
                    Some(l) if l.busy => SessionState::Busy,
                    Some(_) => SessionState::Idle,
                    None => SessionState::Ended,
                },
                pid: running.map(|l| l.pid),
                last_activity: modified,
                pane: None,
                preview: None,
                tokens: Some(meta.tokens).filter(|t| *t > 0),
                id,
            });
            if sessions.len() >= RECENT {
                break;
            }
        }
        // A live session whose transcript is not written yet.
        for l in live
            .iter()
            .filter(|l| roots.iter().any(|r| is_within(&l.cwd, r)))
        {
            if !sessions.iter().any(|s| s.id == l.session_id) {
                sessions.push(AiSession {
                    agent: Agent::ClaudeCode,
                    id: l.session_id.clone(),
                    title: l.name.clone(),
                    cwd: l.cwd.clone(),
                    branch: None,
                    state: if l.busy {
                        SessionState::Busy
                    } else {
                        SessionState::Idle
                    },
                    pid: Some(l.pid),
                    last_activity: SystemTime::now(),
                    pane: None,
                    preview: None,
                    tokens: None,
                });
            }
        }
        sessions
    }

    fn refresh_codex_titles(&mut self) {
        let index = self.codex_home.join("session_index.jsonl");
        let modified = mtime(&index);
        if modified.is_none() || modified == self.codex_index_mtime {
            return;
        }
        self.codex_index_mtime = modified;
        let Ok(text) = std::fs::read_to_string(&index) else {
            return;
        };
        for line in text.lines() {
            if let Ok(value) = serde_json::from_str::<Value>(line)
                && let (Some(id), Some(name)) =
                    (str_field(&value, "id"), str_field(&value, "thread_name"))
            {
                self.codex_titles.insert(id, name); // later lines win
            }
        }
    }

    fn codex_sessions(&mut self, roots: &[PathBuf]) -> Vec<AiSession> {
        let mut files = Vec::new();
        walk_jsonl(&self.codex_home.join("sessions"), &mut files);
        if files.is_empty() {
            return Vec::new();
        }
        self.refresh_codex_titles();
        let now = SystemTime::now();
        let mut sessions = Vec::new();
        for path in files {
            let meta = self
                .codex
                .entry(path.clone())
                .or_insert_with(|| read_codex_meta(&path))
                .clone();
            let Some(meta) = meta else { continue };
            if !roots.iter().any(|root| is_within(&meta.cwd, root)) {
                continue;
            }
            let modified = mtime(&path).unwrap_or(SystemTime::UNIX_EPOCH);
            let recent = now
                .duration_since(modified)
                .is_ok_and(|age| age < CODEX_ACTIVE);
            sessions.push(AiSession {
                agent: Agent::Codex,
                title: self
                    .codex_titles
                    .get(&meta.id)
                    .cloned()
                    .or(meta.first_prompt)
                    .unwrap_or_else(|| "Untitled session".into()),
                id: meta.id,
                cwd: meta.cwd,
                branch: meta.branch,
                state: if recent {
                    SessionState::Active
                } else {
                    SessionState::Ended
                },
                pid: None,
                last_activity: modified,
                pane: None,
                preview: None,
                tokens: None,
            });
        }
        sessions
    }

    /// Sessions whose cwd is inside any of `roots` (the repo and its
    /// worktrees), live first, then newest first.
    pub fn scan(&mut self, roots: &[PathBuf]) -> Vec<AiSession> {
        let mut sessions = self.claude_sessions(roots);
        sessions.extend(self.codex_sessions(roots));
        sessions.sort_by_key(|s| (!s.state.is_live(), std::cmp::Reverse(s.last_activity)));
        sessions.truncate(RECENT);
        self.save_if_due();
        sessions
    }
}

impl Default for SessionIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// "just now", "5m ago", "3h ago", "2d ago".
pub fn ago(when: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(when).map(|d| d.as_secs()).unwrap_or(0);
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn transcript_line(cwd: &Path) -> String {
        format!(
            "{{\"type\":\"user\",\"cwd\":\"{}\",\"gitBranch\":\"feat\",\"sessionId\":\"s\"}}\n",
            cwd.display()
        )
    }

    #[test]
    fn claude_dir_name_matches_claude_code() {
        // Given
        let cwd = Path::new("/Users/me/.t3/worktrees/app/t3-66");

        // When
        let name = claude_dir_name(cwd);

        // Then
        assert_eq!(name, "-Users-me--t3-worktrees-app-t3-66");
    }

    #[test]
    fn finds_titled_session_and_skips_prefix_collision() {
        // Given
        let home = tempfile::tempdir().unwrap();
        let repo = PathBuf::from("/code/app");
        let other = PathBuf::from("/code/app-other");
        for (cwd, id, title) in [(&repo, "s1", "Fix login"), (&other, "s2", "Other repo")] {
            let dir = home
                .path()
                .join("claude/projects")
                .join(claude_dir_name(cwd));
            std::fs::create_dir_all(&dir).unwrap();
            let mut file = std::fs::File::create(dir.join(format!("{id}.jsonl"))).unwrap();
            file.write_all(transcript_line(cwd).as_bytes()).unwrap();
            writeln!(
                file,
                "{{\"type\":\"ai-title\",\"aiTitle\":\"{title}\",\"sessionId\":\"{id}\"}}"
            )
            .unwrap();
        }
        let mut index =
            SessionIndex::with_homes(home.path().join("claude"), home.path().join("codex"));

        // When
        let sessions = index.scan(std::slice::from_ref(&repo));

        // Then
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s1");
        assert_eq!(sessions[0].title, "Fix login");
        assert_eq!(sessions[0].branch.as_deref(), Some("feat"));
        assert_eq!(sessions[0].state, SessionState::Ended);
    }

    #[test]
    fn read_positions_survive_a_restart() {
        // Given: a transcript scanned once with a cache file
        let home = tempfile::tempdir().unwrap();
        let repo = PathBuf::from("/code/app");
        let dir = home
            .path()
            .join("claude/projects")
            .join(claude_dir_name(&repo));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s1.jsonl");
        std::fs::write(&path, transcript_line(&repo)).unwrap();
        let cache = home.path().join("data/session-index.json");
        let open = || {
            let mut index =
                SessionIndex::with_homes(home.path().join("claude"), home.path().join("codex"));
            index.cache = Some(cache.clone());
            index.load();
            index
        };
        open().scan(std::slice::from_ref(&repo));

        // When: a new index (the next launch) loads it
        let reloaded = open();

        // Then
        let meta = &reloaded.transcripts[&path];
        assert_eq!(meta.offset, transcript_line(&repo).len() as u64);
        assert_eq!(meta.cwd.as_deref(), Some(repo.as_path()));
    }

    #[test]
    fn reads_appended_titles_incrementally() {
        // Given
        let home = tempfile::tempdir().unwrap();
        let repo = PathBuf::from("/code/app");
        let dir = home
            .path()
            .join("claude/projects")
            .join(claude_dir_name(&repo));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s1.jsonl");
        std::fs::write(&path, transcript_line(&repo)).unwrap();
        let mut index =
            SessionIndex::with_homes(home.path().join("claude"), home.path().join("codex"));
        let first = index.scan(std::slice::from_ref(&repo));

        // When
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(
            file,
            "{{\"type\":\"custom-title\",\"customTitle\":\"Renamed\",\"sessionId\":\"s1\"}}"
        )
        .unwrap();
        let second = index.scan(&[repo]);

        // Then
        assert_eq!(first[0].title, "Untitled session");
        assert_eq!(second[0].title, "Renamed");
    }

    #[test]
    fn counts_tokens_once_per_assistant_message() {
        // Given: one message logged on two lines, and a second message
        let home = tempfile::tempdir().unwrap();
        let repo = PathBuf::from("/code/app");
        let dir = home
            .path()
            .join("claude/projects")
            .join(claude_dir_name(&repo));
        std::fs::create_dir_all(&dir).unwrap();
        let usage = |id: &str, n: u64| {
            format!(
                "{{\"type\":\"assistant\",\"message\":{{\"id\":\"{id}\",\"usage\":{{\"input_tokens\":{n},\"cache_read_input_tokens\":{n},\"output_tokens\":{n}}}}}}}\n"
            )
        };
        let text = transcript_line(&repo) + &usage("m1", 10) + &usage("m1", 10) + &usage("m2", 1);
        std::fs::write(dir.join("s1.jsonl"), text).unwrap();
        let mut index =
            SessionIndex::with_homes(home.path().join("claude"), home.path().join("codex"));

        // When
        let sessions = index.scan(&[repo]);

        // Then: m1 counted once (30), plus m2 (3)
        assert_eq!(sessions[0].tokens, Some(33));
    }

    #[test]
    fn live_registry_marks_session_busy() {
        // Given
        let home = tempfile::tempdir().unwrap();
        let repo = PathBuf::from("/code/app");
        let live_dir = home.path().join("claude/sessions");
        std::fs::create_dir_all(&live_dir).unwrap();
        let pid = std::process::id();
        std::fs::write(
            live_dir.join(format!("{pid}.json")),
            format!("{{\"pid\":{pid},\"sessionId\":\"live1\",\"cwd\":\"/code/app/sub\",\"name\":\"app-1\",\"status\":\"busy\"}}"),
        )
        .unwrap();
        let mut index =
            SessionIndex::with_homes(home.path().join("claude"), home.path().join("codex"));

        // When
        let sessions = index.scan(&[repo]);

        // Then
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, SessionState::Busy);
        assert_eq!(sessions[0].pid, Some(pid));
        assert_eq!(sessions[0].title, "app-1");
    }

    #[test]
    fn finds_codex_session_with_index_title() {
        // Given
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("codex/sessions/2026/10/31");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(
            day.join("rollout-1.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"c1\",\"cwd\":\"/code/app\",\"git\":{\"branch\":\"main\"}}}\n",
        )
        .unwrap();
        std::fs::write(
            home.path().join("codex/session_index.jsonl"),
            "{\"id\":\"c1\",\"thread_name\":\"Old\"}\n{\"id\":\"c1\",\"thread_name\":\"Plan the API\"}\n",
        )
        .unwrap();
        let mut index =
            SessionIndex::with_homes(home.path().join("claude"), home.path().join("codex"));

        // When
        let sessions = index.scan(&[PathBuf::from("/code/app")]);

        // Then
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent, Agent::Codex);
        assert_eq!(sessions[0].title, "Plan the API");
        assert_eq!(sessions[0].state, SessionState::Active);
    }

    #[test]
    fn codex_title_falls_back_to_first_real_prompt() {
        // Given
        let lines = [
            r##"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions"},{"type":"input_text","text":"<environment_context>x</environment_context>"}]}}"##,
            r##"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"add enums from the schema\nmore"}]}}"##,
            r##"{"type":"event_msg","payload":{"type":"user_message","message":"older format"}}"##,
        ];

        // When
        let prompts: Vec<_> = lines.iter().map(|l| codex_prompt(l)).collect();

        // Then
        assert_eq!(prompts[0], None);
        assert_eq!(prompts[1].as_deref(), Some("add enums from the schema"));
        assert_eq!(prompts[2].as_deref(), Some("older format"));
    }

    #[test]
    fn ago_buckets() {
        // Given
        let now = SystemTime::now();

        // When
        let labels = [0, 300, 7200, 172800].map(|s| ago(now - Duration::from_secs(s), now));

        // Then
        assert_eq!(labels, ["just now", "5m ago", "2h ago", "2d ago"]);
    }
}
