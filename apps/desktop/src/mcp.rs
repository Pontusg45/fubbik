//! An MCP server so other agents can read what the app shows: registered
//! repos, their processes, databases, worktrees, AI sessions and scripts,
//! the Today list, process logs, and the built-in server's status.
//!
//! - Streamable HTTP (JSON responses) at `http://127.0.0.1:3100/mcp`
//!   (`FUBBIK_DESKTOP_MCP_PORT` moves it), served while the app runs.
//! - `fubbik-desktop --mcp`: the same over stdio, forwarded to the running
//!   app (for clients that only start stdio servers).
//! - Read-only. The UI publishes a JSON view after every poll
//!   (`ui/mcp_publish.rs`); tools answer from it, so a call never waits
//!   on git or the network.

use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::{Value, json};

use crate::supervisor::Supervisor;

const PROTOCOL_VERSION: &str = "2025-06-18";

/// What the UI last published.
#[derive(Debug, Clone, Default)]
pub struct Published {
    /// One object per repo (see `ui/mcp_publish.rs`).
    pub repos: Vec<Value>,
    pub today: Vec<Value>,
    pub at: Option<SystemTime>,
}

fn published() -> &'static Mutex<Published> {
    static PUBLISHED: OnceLock<Mutex<Published>> = OnceLock::new();
    PUBLISHED.get_or_init(Default::default)
}

pub fn publish(repos: Vec<Value>, today: Vec<Value>) {
    *published().lock().unwrap() = Published {
        repos,
        today,
        at: Some(SystemTime::now()),
    };
}

fn supervisor_cell() -> &'static OnceLock<Supervisor> {
    static SUPERVISOR: OnceLock<Supervisor> = OnceLock::new();
    &SUPERVISOR
}

/// The app's process supervisor, for log reads.
pub fn set_supervisor(supervisor: Supervisor) {
    let _ = supervisor_cell().set(supervisor);
}

/// `127.0.0.1:3100`, or the port in `FUBBIK_DESKTOP_MCP_PORT`.
pub fn addr() -> String {
    let port = std::env::var("FUBBIK_DESKTOP_MCP_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3100);
    format!("127.0.0.1:{port}")
}

fn tools() -> Value {
    let repo = json!({
        "type": "string",
        "description": "Repo name (its folder name, as list_repos shows) or its path"
    });
    json!([
        {
            "name": "list_repos",
            "description": "The repos registered in the fubbik desktop app, each with its overall status, git branch and changes, how many processes are up, live AI sessions, worktrees and anything that needs attention.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_repo",
            "description": "Everything the app knows about one repo: git state, processes (status, port, pid, memory, started by the app or not), databases, worktrees (branch, changes, ahead/behind), AI sessions (agent, title, state, branch, tokens), worktree overlaps, scripts (just/npm/make), and its fubbik knowledge summary.",
            "inputSchema": { "type": "object", "properties": { "repo": repo }, "required": ["repo"] }
        },
        {
            "name": "get_today",
            "description": "The app's Today list across all repos: what needs the user (waiting agents, failing processes, overlapping worktrees, failing requirements, stale knowledge), what is in flight (tasks in progress) and what is next (startable tasks).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_sessions",
            "description": "AI agent sessions (Claude Code, Codex) across repos or in one repo, live and waiting first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo": repo,
                    "state": { "type": "string", "description": "Only sessions in this state, e.g. working, waiting, idle, ended" }
                }
            }
        },
        {
            "name": "get_process_logs",
            "description": "The last lines a process (or script) started by the app printed. Processes started outside the app have no captured log.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo": repo,
                    "process": { "type": "string", "description": "Process name from get_repo, or `script:<name>` for a script" },
                    "lines": { "type": "integer", "description": "How many lines from the end (default 200, at most 2000)" }
                },
                "required": ["repo", "process"]
            }
        },
        {
            "name": "get_builtin_server",
            "description": "The status of the app's built-in fubbik server (the API on 127.0.0.1:3000 and its own PostgreSQL), and where its data, logs and backups are.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn find_repo<'a>(repos: &'a [Value], wanted: &str) -> Result<&'a Value, String> {
    repos
        .iter()
        .find(|r| r["name"] == wanted || r["path"] == wanted)
        .ok_or_else(|| {
            let names: Vec<&str> = repos.iter().filter_map(|r| r["name"].as_str()).collect();
            format!("no repo \"{wanted}\"; registered: {}", names.join(", "))
        })
}

fn call_tool(name: &str, args: &Value) -> Result<Value, String> {
    let state = published().lock().unwrap().clone();
    call_tool_in(state, name, args)
}

/// A tool call against a published view.
fn call_tool_in(state: Published, name: &str, args: &Value) -> Result<Value, String> {
    if state.at.is_none() && name != "get_builtin_server" {
        return Err("the app has not finished its first poll yet; try again in a moment".into());
    }
    let arg = |key: &str| args.get(key).and_then(Value::as_str);
    match name {
        "list_repos" => Ok(Value::Array(
            state.repos.iter().map(|r| r["summary"].clone()).collect(),
        )),
        "get_repo" => {
            let repo = find_repo(&state.repos, arg("repo").ok_or("repo is required")?)?;
            let mut full = repo.clone();
            if let Some(object) = full.as_object_mut() {
                object.remove("summary");
            }
            Ok(full)
        }
        "get_today" => Ok(Value::Array(state.today)),
        "list_sessions" => {
            let repos: Vec<&Value> = match arg("repo") {
                Some(wanted) => vec![find_repo(&state.repos, wanted)?],
                None => state.repos.iter().collect(),
            };
            let sessions: Vec<Value> = repos
                .into_iter()
                .flat_map(|r| {
                    let name = r["name"].clone();
                    r["sessions"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(move |mut s| {
                            s["repo"] = name.clone();
                            s
                        })
                })
                .filter(|s| {
                    arg("state").is_none_or(|wanted| {
                        s["state"]
                            .as_str()
                            .is_some_and(|st| st.eq_ignore_ascii_case(wanted))
                    })
                })
                .collect();
            Ok(Value::Array(sessions))
        }
        "get_process_logs" => {
            let repo = find_repo(&state.repos, arg("repo").ok_or("repo is required")?)?;
            let process = arg("process").ok_or("process is required")?;
            let path = repo["path"].as_str().unwrap_or_default();
            let key = crate::model::process_key(std::path::Path::new(path), process);
            let supervisor = supervisor_cell()
                .get()
                .ok_or("the app's supervisor is not available")?;
            let lines = args
                .get("lines")
                .and_then(Value::as_u64)
                .unwrap_or(200)
                .min(2000) as usize;
            let all = supervisor.logs(&key);
            if all.is_empty() {
                return Err(format!(
                    "no captured output for \"{process}\" in {}: only processes and scripts started from the app are logged",
                    repo["name"].as_str().unwrap_or(path)
                ));
            }
            let skip = all.len().saturating_sub(lines);
            Ok(json!({ "process": process, "lines": all[skip..] }))
        }
        "get_builtin_server" => {
            let status = crate::backend::status();
            let root = crate::backend::data_dir();
            Ok(json!({
                "status": status.label(),
                "url": format!("http://{}", crate::backend::api_addr()),
                "data": root.join("pgdata"),
                "logs": { "server": root.join("backend.log"), "postgres": root.join("postgres.log") },
                "backups": root.join("backups"),
            }))
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

/// One JSON-RPC message in, at most one out (notifications get none).
pub fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned()?;
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "fubbik-desktop", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Read-only view of the fubbik desktop app: the user's registered repos, their running processes, AI agent sessions, worktrees and the Today list. For the knowledge base itself (chunks, plans, requirements) use the fubbik MCP server (`fubbik mcp`)."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            // Tool failures are results the agent can read, not protocol errors.
            Ok(match call_tool(name, &args) {
                Ok(value) => json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                    "structuredContent": if value.is_object() { value.clone() } else { json!({ "items": value }) },
                }),
                Err(message) => {
                    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
                }
            })
        }
        _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

/// Serves `POST /mcp` on its own thread for the life of the app.
pub fn start_http() {
    let addr = addr();
    let spawned = std::thread::Builder::new()
        .name("fubbik-mcp".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => return eprintln!("fubbik: MCP server: {error}"),
            };
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::bind(&addr).await {
                    Ok(listener) => listener,
                    // Another copy of the app (or something else) has the port.
                    Err(error) => return eprintln!("fubbik: MCP server: {addr}: {error}"),
                };
                let app =
                    axum::Router::new().route("/mcp", axum::routing::post(http_post).get(http_get));
                if let Err(error) = axum::serve(listener, app).await {
                    eprintln!("fubbik: MCP server: {error}");
                }
            });
        });
    if let Err(error) = spawned {
        eprintln!("fubbik: MCP server: {error}");
    }
}

/// A browser page on another site must not reach this (DNS rebinding):
/// requests with a non-local Origin are refused.
fn local_origin(headers: &axum::http::HeaderMap) -> bool {
    match headers
        .get(axum::http::header::ORIGIN)
        .and_then(|o| o.to_str().ok())
    {
        None => true,
        Some(origin) => {
            let host = origin
                .split("://")
                .nth(1)
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("");
            matches!(host, "localhost" | "127.0.0.1" | "[::1]")
        }
    }
}

async fn http_post(headers: axum::http::HeaderMap, body: String) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    if !local_origin(&headers) {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let Ok(message) = serde_json::from_str::<Value>(&body) else {
        let error = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } });
        return (StatusCode::BAD_REQUEST, axum::Json(error)).into_response();
    };
    // A batch is an array of messages.
    let reply = match &message {
        Value::Array(batch) => {
            let replies: Vec<Value> = batch.iter().filter_map(handle).collect();
            (!replies.is_empty()).then_some(Value::Array(replies))
        }
        single => handle(single),
    };
    match reply {
        Some(reply) => axum::Json(reply).into_response(),
        // Notifications and responses: accepted, nothing to say.
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// No server-initiated stream.
async fn http_get() -> axum::http::StatusCode {
    axum::http::StatusCode::METHOD_NOT_ALLOWED
}

/// `--mcp`: JSON-RPC over stdio, each message forwarded to the running app.
pub fn run_stdio_bridge() {
    use std::io::{BufRead, Write};
    let url = format!("http://{}/mcp", addr());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(_) => continue,
        };
        let reply = match ureq::post(&url).header("content-type", "application/json").send(&line) {
            Ok(response) if response.status() == 202 => None,
            Ok(mut response) => response.body_mut().read_json::<Value>().ok(),
            Err(error) => request.get("id").cloned().map(|id| {
                json!({ "jsonrpc": "2.0", "id": id, "error": {
                    "code": -32000,
                    "message": format!("the fubbik desktop app is not running (or its MCP server is not at {url}): {error}")
                }})
            }),
        };
        if let Some(reply) = reply {
            let _ = writeln!(stdout, "{reply}");
            let _ = stdout.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_the_handshake_and_lists_tools() {
        // Given
        let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} });
        let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });

        // When
        let (init, list, note) = (
            handle(&init).unwrap(),
            handle(&list).unwrap(),
            handle(&note),
        );

        // Then
        assert_eq!(init["result"]["serverInfo"]["name"], "fubbik-desktop");
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(
            names.contains(&"list_repos") && names.contains(&"get_today"),
            "{names:?}"
        );
        assert!(note.is_none(), "notifications get no reply");
    }

    #[test]
    fn tools_answer_from_what_the_ui_published() {
        // Given
        let state = Published {
            repos: vec![json!({
                "name": "app", "path": "/x/app",
                "summary": { "name": "app", "status": "Running" },
                "sessions": [
                    { "agent": "Claude Code", "title": "Fix it", "state": "waiting" },
                    { "agent": "Codex", "title": "Old", "state": "ended" }
                ]
            })],
            today: vec![json!({ "section": "needs_you", "title": "Fix it" })],
            at: Some(SystemTime::now()),
        };
        let call = |name: &str, args: Value| call_tool_in(state.clone(), name, &args);

        // When
        let repos = call("list_repos", json!({})).unwrap();
        let repo = call("get_repo", json!({ "repo": "app" })).unwrap();
        let waiting = call("list_sessions", json!({ "state": "waiting" })).unwrap();
        let today = call("get_today", json!({})).unwrap();
        let missing = call("get_repo", json!({ "repo": "nope" }));
        let unpolled = call_tool_in(Published::default(), "list_repos", &json!({}));

        // Then
        assert_eq!(repos[0]["status"], "Running");
        assert!(repo.get("summary").is_none());
        assert_eq!(waiting.as_array().unwrap().len(), 1);
        assert_eq!(waiting[0]["repo"], "app");
        assert_eq!(today[0]["title"], "Fix it");
        assert!(missing.unwrap_err().contains("registered: app"));
        assert!(unpolled.unwrap_err().contains("first poll"));
    }

    #[test]
    fn tool_errors_are_results_the_agent_can_read() {
        // Given
        let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": "nope", "arguments": {} } });

        // When
        let reply = handle(&request).unwrap();

        // Then
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"]["isError"], true);
    }

    #[test]
    fn only_local_origins_are_let_in() {
        // Given
        let with = |origin: &str| {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(axum::http::header::ORIGIN, origin.parse().unwrap());
            headers
        };

        // When / Then
        assert!(local_origin(&axum::http::HeaderMap::new()));
        assert!(local_origin(&with("http://localhost:3001")));
        assert!(local_origin(&with("http://127.0.0.1:3000")));
        assert!(!local_origin(&with("https://evil.example")));
    }
}
