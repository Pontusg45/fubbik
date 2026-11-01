//! The fubbik HTTP API, seen from one repo. Responses are parsed by pure
//! functions so the shapes are tested without a server. A 404 means the
//! route's feature group is switched off (`fubbik.toml`) and reads as empty.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub task_done: i64,
    pub task_total: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReqStats {
    pub total: i64,
    pub passing: i64,
    pub failing: i64,
    pub untested: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance: Vec<(String, bool)>,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Tasks this one waits on (from the plan's dependencies).
    pub depends_on: Vec<String>,
    /// What the desktop app recorded when it started the task.
    pub link: TaskLink,
    /// The task's `metadata`, raw JSON, so a write can merge into it.
    pub metadata_raw: Option<String>,
}

/// `metadata.fubbikDesktop`: where a task's work happens, so a merge into
/// main can mark it done.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskLink {
    pub branch: Option<String>,
    /// Main's commit when the branch was created; a merged branch tip that
    /// is still this commit means nothing was done yet.
    pub base_commit: Option<String>,
    pub worktree: Option<String>,
}

pub const LINK_KEY: &str = "fubbikDesktop";

/// A coordination-board journal entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardEntry {
    pub task_id: Option<String>,
    pub kind: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanDetail {
    pub id: String,
    pub title: String,
    pub status: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub id: String,
    pub handle: String,
    pub external_key: Option<String>,
    pub status: String,
    pub metadata: Value,
}

impl Run {
    /// The Claude session this run is, by `externalKey` or `metadata.sessionId`.
    pub fn session_id(&self) -> Option<&str> {
        self.external_key
            .as_deref()
            .or_else(|| self.metadata.get("sessionId").and_then(Value::as_str))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Claim {
    pub task_id: String,
    pub run_id: String,
    pub expired: bool,
    pub claimed_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Board {
    pub plan_id: String,
    pub plan_title: String,
    pub tasks: Vec<Task>,
    pub runs: Vec<Run>,
    pub claims: Vec<Claim>,
    pub entries: Vec<BoardEntry>,
}

impl Board {
    /// Tasks a run holds a live claim on.
    pub fn tasks_of(&self, run_id: &str) -> Vec<&Task> {
        self.claims
            .iter()
            .filter(|c| c.run_id == run_id && !c.expired)
            .filter_map(|c| self.tasks.iter().find(|t| t.id == c.task_id))
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaleFlag {
    pub id: String,
    pub chunk_id: String,
    pub chunk_title: String,
    pub reason: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    pub id: String,
    pub chunk_id: String,
    pub chunk_title: String,
    pub reason: Option<String>,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requirement {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatrixSummary {
    pub id: String,
    pub name: String,
    pub layer: String,
    pub violated: i64,
    pub verified: i64,
    pub specified: i64,
    pub unspecified: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Feature {
    pub id: String,
    pub name: String,
    pub status: String,
    pub delta_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkRef {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub stale: bool,
    /// `file-ref`, `applies-to`, … when it came from a file lookup.
    pub match_reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiHealth {
    pub status: String,
    pub db: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Knowledge {
    pub space: Option<Space>,
    pub chunks: i64,
    pub stale: i64,
    pub orphans: i64,
    pub thin: i64,
    pub stale_embeddings: i64,
    pub unenriched: i64,
    pub plans: Vec<PlanSummary>,
    /// Open plans (not completed or archived) with their tasks.
    pub plan_details: Vec<PlanDetail>,
    /// Coordination boards of in-progress plans.
    pub boards: Vec<Board>,
    pub requirements: ReqStats,
    /// Failing first, then untested.
    pub open_requirements: Vec<Requirement>,
    pub matrices: Vec<MatrixSummary>,
    pub stale_flags: Vec<StaleFlag>,
    /// Pending proposals across all spaces (the API cannot filter by space).
    pub proposals: Vec<Proposal>,
    pub proposals_pending: i64,
    pub features: Vec<Feature>,
    pub active_features: Vec<String>,
}

impl Knowledge {
    pub fn active_agents(&self) -> usize {
        self.boards
            .iter()
            .flat_map(|b| b.runs.iter())
            .filter(|r| r.status == "active")
            .count()
    }

    pub fn violated_cells(&self) -> i64 {
        self.matrices.iter().map(|m| m.violated).sum()
    }

    /// The board run (and its claimed tasks) for a Claude session.
    pub fn run_for_session(&self, session_id: &str) -> Option<(&Board, &Run)> {
        self.boards.iter().find_map(|board| {
            board
                .runs
                .iter()
                .find(|run| run.session_id() == Some(session_id))
                .map(|run| (board, run))
        })
    }
}

#[derive(Clone)]
pub struct FubbikClient {
    base: String,
    agent: ureq::Agent,
    /// For calls that run for minutes, like enrich-all.
    slow: ureq::Agent,
}

/// `NotFound` for a 404: the route's feature group is switched off.
enum Fetch {
    Body(String),
    NotFound,
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

fn read_body(mut response: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn describe(error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(404) => "not available (feature switched off, or not found)".into(),
        ureq::Error::StatusCode(409) => {
            "conflict: someone else holds it, or it already exists".into()
        }
        ureq::Error::StatusCode(code) => format!("the server answered {code}"),
        other => other.to_string(),
    }
}

fn s(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn opt(value: &Value, key: &str) -> Option<String> {
    value[key]
        .as_str()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

fn items<'a>(value: &'a Value, key: Option<&str>) -> impl Iterator<Item = &'a Value> {
    let list = match key {
        Some(key) => &value[key],
        None => value,
    };
    list.as_array().into_iter().flatten()
}

impl FubbikClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            agent: agent(Duration::from_secs(3)),
            slow: agent(Duration::from_secs(15 * 60)),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Fetch, String> {
        let mut request = self.agent.get(self.url(path));
        for (key, value) in query {
            request = request.query(*key, *value);
        }
        match request.call() {
            Ok(mut response) => response
                .body_mut()
                .read_to_string()
                .map(Fetch::Body)
                .map_err(|e| e.to_string()),
            Err(ureq::Error::StatusCode(404)) => Ok(Fetch::NotFound),
            Err(e) => Err(describe(e)),
        }
    }

    fn get_json(&self, path: &str, query: &[(&str, &str)]) -> Result<Option<Value>, String> {
        match self.get(path, query)? {
            Fetch::Body(body) if body.trim().is_empty() => Ok(None),
            Fetch::Body(body) => serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| e.to_string()),
            Fetch::NotFound => Ok(None),
        }
    }

    fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.agent
            .post(self.url(path))
            .send_json(body)
            .map_err(describe)
            .and_then(read_body)
    }

    fn patch(&self, path: &str, body: Value) -> Result<Value, String> {
        self.agent
            .patch(self.url(path))
            .send_json(body)
            .map_err(describe)
            .and_then(read_body)
    }

    fn put(&self, path: &str, body: Value) -> Result<Value, String> {
        self.agent
            .put(self.url(path))
            .send_json(body)
            .map_err(describe)
            .and_then(read_body)
    }

    // ── reads ──────────────────────────────────────────────────────────

    pub fn health(&self) -> Result<ApiHealth, String> {
        let value = self.get_json("/api/health", &[])?.unwrap_or(Value::Null);
        Ok(ApiHealth {
            status: s(&value, "status"),
            db: s(&value, "db"),
        })
    }

    pub fn capabilities(&self) -> Result<Vec<String>, String> {
        let value = self
            .get_json("/api/capabilities", &[])?
            .unwrap_or(Value::Null);
        Ok(items(&value, Some("features"))
            .filter_map(|f| f.as_str().map(str::to_string))
            .collect())
    }

    pub fn detect_space(
        &self,
        remote: Option<&str>,
        local_path: &Path,
    ) -> Result<Option<Space>, String> {
        let local = local_path.to_string_lossy();
        let mut query = vec![("localPath", local.as_ref())];
        if let Some(remote) = remote {
            query.push(("remoteUrl", remote));
        }
        match self.get("/api/spaces/detect", &query)? {
            Fetch::Body(body) => Ok(parse_space(&body)),
            Fetch::NotFound => Ok(None),
        }
    }

    pub fn knowledge(&self, space: Space) -> Result<Knowledge, String> {
        let id = space.id.clone();
        let q = [("spaceId", id.as_str())];
        let total = |query: &[(&str, &str)]| -> Result<i64, String> {
            Ok(self
                .get_json("/api/chunks", query)?
                .and_then(|v| v["total"].as_i64())
                .unwrap_or(0))
        };
        let chunks = total(&[("spaceId", &id), ("limit", "1")])?;
        let unenriched = total(&[("spaceId", &id), ("limit", "1"), ("enrichment", "missing")])?;
        let stale = match self.get("/api/chunks/stale/count", &q)? {
            Fetch::Body(body) => body.trim().parse().unwrap_or(0),
            Fetch::NotFound => 0,
        };
        let health = self
            .get_json("/api/health/knowledge", &q)?
            .unwrap_or(Value::Null);
        let plans = self
            .get_json("/api/plans", &q)?
            .map(|v| parse_plans(&v))
            .unwrap_or_default();

        let mut plan_details = Vec::new();
        for plan in plans
            .iter()
            .filter(|p| !matches!(p.status.as_str(), "completed" | "archived"))
            .take(8)
        {
            if let Some(detail) = self.get_json(&format!("/api/plans/{}", plan.id), &[])? {
                plan_details.push(parse_plan_detail(&detail));
            }
        }
        let mut boards = Vec::new();
        for plan in plans.iter().filter(|p| p.status == "in_progress") {
            if let Some(board) = self.get_json(&format!("/api/plans/{}/board", plan.id), &[])? {
                boards.push(parse_board(&board));
            }
        }

        let requirements = self
            .get_json("/api/requirements/stats", &q)?
            .map(|v| parse_req_stats(&v))
            .unwrap_or_default();
        let mut open_requirements = Vec::new();
        for status in ["failing", "untested"] {
            if let Some(list) = self.get_json(
                "/api/requirements",
                &[("spaceId", &id), ("status", status), ("limit", "20")],
            )? {
                open_requirements.extend(parse_requirements(&list));
            }
        }
        let mut matrices = Vec::new();
        if let Some(list) = self.get_json("/api/matrices", &q)? {
            for matrix in items(&list, None).take(10) {
                let matrix_id = s(matrix, "id");
                if let Some(view) =
                    self.get_json(&format!("/api/matrices/{matrix_id}/view"), &[])?
                {
                    matrices.push(parse_matrix(matrix, &view));
                }
            }
        }

        let stale_flags = self
            .get_json("/api/chunks/stale", &[("spaceId", &id), ("limit", "20")])?
            .map(|v| parse_stale_flags(&v))
            .unwrap_or_default();
        let proposals = self
            .get_json("/api/proposals", &[("status", "pending"), ("limit", "20")])?
            .map(|v| parse_proposals(&v))
            .unwrap_or_default();
        let proposals_pending = self
            .get_json("/api/proposals/count", &[])?
            .and_then(|v| v["pending"].as_i64())
            .unwrap_or(proposals.len() as i64);
        let features = self
            .get_json("/api/features", &q)?
            .map(|v| parse_features(&v))
            .unwrap_or_default();
        let active_features = self
            .get_json("/api/features/active", &[])?
            .map(|v| {
                items(&v, None)
                    .filter_map(|f| f.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Knowledge {
            space: Some(space),
            chunks,
            stale,
            orphans: health["orphans"]["count"].as_i64().unwrap_or(0),
            thin: health["thin"]["count"].as_i64().unwrap_or(0),
            stale_embeddings: health["staleEmbeddings"]["count"].as_i64().unwrap_or(0),
            unenriched,
            plans,
            plan_details,
            boards,
            requirements,
            open_requirements,
            matrices,
            stale_flags,
            proposals,
            proposals_pending,
            features,
            active_features,
        })
    }

    /// `structured-md` context for a plan, for an agent's first prompt.
    pub fn context_for_plan(&self, plan_id: &str) -> Result<String, String> {
        let value = self
            .get_json(
                "/api/context/for-plan",
                &[("planId", plan_id), ("maxTokens", "6000")],
            )?
            .unwrap_or(Value::Null);
        Ok(s(&value, "content"))
    }

    /// The `for-diff` lookup: chunks for a set of changed files, by section.
    pub fn context_for_files(
        &self,
        paths: &[String],
        space_id: Option<&str>,
    ) -> Result<Vec<(String, Vec<ChunkRef>)>, String> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let joined = paths.join(",");
        let mut query = vec![
            ("paths", joined.as_str()),
            ("maxTokens", "8000"),
            ("format", "structured-json"),
        ];
        if let Some(space) = space_id {
            query.push(("spaceId", space));
        }
        Ok(self
            .get_json("/api/context/for-files", &query)?
            .map(|v| parse_sections(&v))
            .unwrap_or_default())
    }

    /// The `check-files` lookup for one file: every chunk linked to it.
    pub fn chunks_for_file(
        &self,
        path: &str,
        space_id: Option<&str>,
    ) -> Result<Vec<ChunkRef>, String> {
        let mut query = vec![
            ("path", path),
            ("maxTokens", "8000"),
            ("format", "json-legacy"),
        ];
        if let Some(space) = space_id {
            query.push(("spaceId", space));
        }
        Ok(self
            .get_json("/api/context/for-file", &query)?
            .map(|v| parse_legacy_chunks(&v))
            .unwrap_or_default())
    }

    pub fn context_for_file_md(
        &self,
        path: &str,
        space_id: Option<&str>,
    ) -> Result<String, String> {
        let mut query = vec![("path", path), ("maxTokens", "4000")];
        if let Some(space) = space_id {
            query.push(("spaceId", space));
        }
        let value = self
            .get_json("/api/context/for-file", &query)?
            .unwrap_or(Value::Null);
        Ok(s(&value, "content"))
    }

    /// A space's chunks and connections, laid out (`GET /api/graph`).
    pub fn graph(&self, space_id: &str) -> Result<crate::graph_layout::Graph, String> {
        let json = self
            .get_json("/api/graph", &[("spaceId", space_id)])?
            .ok_or("the server has no /api/graph")?;
        let mut graph = crate::graph_layout::Graph::from_api(&json);
        // What needs attention (best effort: the graph shows without it).
        let ids = |list: &Value, key: &str| -> Vec<String> {
            list.as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c[key].as_str().map(str::to_string))
                .collect()
        };
        if let Ok(Some(flags)) = self.get_json(
            "/api/chunks/stale",
            &[("spaceId", space_id), ("limit", "1000")],
        ) {
            // `age` only means old (stale); a changed file or a diverged
            // duplicate means the chunk may be wrong (flagged).
            let by_reason = |age: bool| -> Vec<String> {
                flags
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|f| (f["reason"] == "age") == age)
                    .filter_map(|f| f["chunkId"].as_str().map(str::to_string))
                    .collect()
            };
            graph.mark(&by_reason(false), "flagged");
            graph.mark(&by_reason(true), "stale");
        }
        if let Ok(Some(health)) = self.get_json("/api/health/knowledge", &[("spaceId", space_id)]) {
            graph.mark(&ids(&health["stale"]["chunks"], "id"), "stale");
            graph.mark(&ids(&health["thin"]["chunks"], "id"), "thin");
        }
        Ok(graph)
    }

    /// A connection between two chunks, made by the user.
    pub fn connect(&self, source: &str, target: &str, relation: &str) -> Result<(), String> {
        self.post(
            "/api/connections",
            json!({ "sourceId": source, "targetId": target, "relation": relation, "origin": "human" }),
        )
        .map(|_| ())
    }

    pub fn search(&self, query: &str, space_id: Option<&str>) -> Result<Vec<ChunkRef>, String> {
        let mut params = vec![("search", query), ("limit", "8")];
        if let Some(space) = space_id {
            params.push(("spaceId", space));
        }
        let value = self
            .get_json("/api/chunks", &params)?
            .unwrap_or(Value::Null);
        Ok(items(&value, Some("chunks")).map(chunk_ref).collect())
    }

    /// `GET /api/chunks/export/claude-md`: the generated CLAUDE.md text.
    pub fn claude_md(
        &self,
        tag: &str,
        max_tokens: u32,
        space_id: Option<&str>,
    ) -> Result<String, String> {
        let tokens = max_tokens.to_string();
        let mut query = vec![("tag", tag), ("maxTokens", tokens.as_str())];
        if let Some(space) = space_id {
            query.push(("spaceId", space));
        }
        let value = self
            .get_json("/api/chunks/export/claude-md", &query)?
            .unwrap_or(Value::Null);
        Ok(s(&value, "content"))
    }

    // ── writes ─────────────────────────────────────────────────────────

    /// One plan's coordination board (for a task's timeline).
    pub fn board(&self, plan_id: &str) -> Result<Option<Board>, String> {
        Ok(self
            .get_json(&format!("/api/plans/{plan_id}/board"), &[("limit", "500")])?
            .map(|v| parse_board(&v)))
    }

    /// Moves a task between columns. Unlike a board transition, this needs no
    /// claim: it is the user deciding.
    pub fn set_task_status(
        &self,
        plan_id: &str,
        task_id: &str,
        status: &str,
    ) -> Result<(), String> {
        self.patch(
            &format!("/api/plans/{plan_id}/tasks/{task_id}"),
            json!({ "status": status }),
        )
        .map(|_| ())
    }

    pub fn set_acceptance(
        &self,
        plan_id: &str,
        task_id: &str,
        criteria: &[(String, bool)],
    ) -> Result<(), String> {
        let criteria: Vec<Value> = criteria
            .iter()
            .map(|(text, done)| json!({ "text": text, "done": done }))
            .collect();
        self.patch(
            &format!("/api/plans/{plan_id}/tasks/{task_id}"),
            json!({ "acceptanceCriteria": criteria }),
        )
        .map(|_| ())
    }

    /// Records where a task's work happens, merged into its existing metadata.
    pub fn set_task_link(&self, plan_id: &str, task: &Task, link: &TaskLink) -> Result<(), String> {
        let mut metadata: Value = task
            .metadata_raw
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_else(|| json!({}));
        metadata[LINK_KEY] = json!({
            "branch": link.branch,
            "baseCommit": link.base_commit,
            "worktree": link.worktree,
        });
        self.patch(
            &format!("/api/plans/{plan_id}/tasks/{}", task.id),
            json!({ "metadata": metadata }),
        )
        .map(|_| ())
    }

    pub fn quick_note(&self, space_id: &str, title: &str, content: &str) -> Result<(), String> {
        self.post(
            "/api/chunks",
            json!({ "title": title, "content": content, "type": "note", "spaceIds": [space_id] }),
        )
        .map(|_| ())
    }

    pub fn dismiss_stale(&self, flag_id: &str) -> Result<(), String> {
        self.agent
            .post(self.url(&format!("/api/chunks/{flag_id}/dismiss-staleness")))
            .send_empty()
            .map(|_| ())
            .map_err(describe)
    }

    pub fn review_proposal(&self, proposal_id: &str, approve: bool) -> Result<(), String> {
        let action = if approve { "approve" } else { "reject" };
        self.post(&format!("/api/proposals/{proposal_id}/{action}"), json!({}))
            .map(|_| ())
    }

    /// Runs synchronously on the server (up to 1000 chunks); can take minutes.
    pub fn enrich_all(&self) -> Result<i64, String> {
        let response = self
            .slow
            .post(self.url("/api/chunks/enrich-all"))
            .send_empty()
            .map_err(describe)?;
        Ok(read_body(response)?["enriched"].as_i64().unwrap_or(0))
    }

    pub fn set_active_features(&self, ids: &[String]) -> Result<(), String> {
        self.put("/api/features/active", json!({ "featureIds": ids }))
            .map(|_| ())
    }

    pub fn set_requirement_status(&self, id: &str, status: &str) -> Result<(), String> {
        self.patch(
            &format!("/api/requirements/{id}/status"),
            json!({ "status": status }),
        )
        .map(|_| ())
    }

    pub fn report_test(
        &self,
        matrix: &str,
        cell: &str,
        test_ref: &str,
        pass: bool,
        detail: &str,
    ) -> Result<(), String> {
        self.post(
            &format!("/api/matrices/{matrix}/cells/{cell}/test-results"),
            json!({ "testRef": test_ref, "status": if pass { "pass" } else { "fail" }, "detail": detail }),
        )
        .map(|_| ())
    }

    /// Joins a plan's board. Idempotent on `external_key` (a Claude session id).
    pub fn join_run(
        &self,
        plan_id: &str,
        handle: &str,
        external_key: &str,
        metadata: Value,
    ) -> Result<String, String> {
        let run = self.post(
            &format!("/api/plans/{plan_id}/board/runs"),
            json!({ "handle": handle, "externalKey": external_key, "capabilities": ["code"], "metadata": metadata }),
        )?;
        opt(&run, "id").ok_or_else(|| "the board returned no run id".into())
    }

    pub fn claim_task(&self, plan_id: &str, task_id: &str, run_id: &str) -> Result<(), String> {
        self.post(
            &format!("/api/plans/{plan_id}/board/tasks/{task_id}/claim"),
            json!({ "runId": run_id, "action": "claim", "leaseSeconds": 3600 }),
        )
        .map(|_| ())
    }

    pub fn transition_task(
        &self,
        plan_id: &str,
        task_id: &str,
        run_id: &str,
        status: &str,
        note: &str,
    ) -> Result<(), String> {
        self.post(
            &format!("/api/plans/{plan_id}/board/tasks/{task_id}/transition"),
            json!({ "runId": run_id, "status": status, "note": note, "clientMutationId": crate::agent_task::uuid_v4() }),
        )
        .map(|_| ())
    }
}

/// `/api/spaces/detect` answers 200 with an empty body when nothing matches.
fn parse_space(body: &str) -> Option<Space> {
    let value: Value = serde_json::from_str(body).ok()?;
    Some(Space {
        id: opt(&value, "id")?,
        name: s(&value, "name"),
    })
}

fn parse_plans(value: &Value) -> Vec<PlanSummary> {
    items(value, None)
        .filter_map(|row| {
            Some(PlanSummary {
                id: opt(row, "id")?,
                title: s(row, "title"),
                status: s(row, "status"),
                task_done: row["taskDone"].as_i64().unwrap_or(0),
                task_total: row["taskTotal"].as_i64().unwrap_or(0),
            })
        })
        .collect()
}

fn parse_task(task: &Value) -> Option<Task> {
    Some(Task {
        id: opt(task, "id")?,
        title: s(task, "title"),
        description: opt(task, "description"),
        acceptance: items(task, Some("acceptanceCriteria"))
            .filter_map(|c| match c {
                Value::String(text) => Some((text.clone(), false)),
                Value::Object(_) => Some((s(c, "text"), c["done"].as_bool().unwrap_or(false))),
                _ => None,
            })
            .collect(),
        status: s(task, "status"),
        created_at: opt(task, "createdAt"),
        updated_at: opt(task, "updatedAt"),
        depends_on: Vec::new(),
        link: {
            let link = &task["metadata"][LINK_KEY];
            TaskLink {
                branch: opt(link, "branch"),
                base_commit: opt(link, "baseCommit"),
                worktree: opt(link, "worktree"),
            }
        },
        metadata_raw: task
            .get("metadata")
            .filter(|m| m.is_object())
            .map(|m| m.to_string()),
    })
}

fn parse_plan_detail(value: &Value) -> PlanDetail {
    let plan = &value["plan"];
    PlanDetail {
        id: s(plan, "id"),
        title: s(plan, "title"),
        status: s(plan, "status"),
        tasks: {
            let mut tasks: Vec<Task> = items(value, Some("tasks")).filter_map(parse_task).collect();
            for dep in items(value, Some("dependencies")) {
                let (task, on) = (s(dep, "taskId"), s(dep, "dependsOnTaskId"));
                if let Some(t) = tasks.iter_mut().find(|t| t.id == task) {
                    t.depends_on.push(on);
                }
            }
            tasks
        },
    }
}

fn parse_board(value: &Value) -> Board {
    Board {
        plan_id: s(&value["plan"], "id"),
        plan_title: s(&value["plan"], "title"),
        tasks: items(value, Some("tasks")).filter_map(parse_task).collect(),
        runs: items(value, Some("runs"))
            .filter_map(|run| {
                Some(Run {
                    id: opt(run, "id")?,
                    handle: s(run, "handle"),
                    external_key: opt(run, "externalKey"),
                    status: s(run, "status"),
                    metadata: run["metadata"].clone(),
                })
            })
            .collect(),
        claims: items(value, Some("claims"))
            .map(|claim| Claim {
                task_id: s(claim, "taskId"),
                run_id: s(claim, "agentRunId"),
                expired: claim["expired"].as_bool().unwrap_or(false),
                claimed_at: opt(claim, "claimedAt"),
            })
            .collect(),
        entries: items(value, Some("entries"))
            .map(|e| BoardEntry {
                task_id: opt(e, "taskId"),
                kind: s(e, "kind"),
                body: s(e, "body"),
                created_at: s(e, "createdAt"),
            })
            .collect(),
    }
}

fn parse_req_stats(value: &Value) -> ReqStats {
    let n = |key: &str| value[key].as_i64().unwrap_or(0);
    ReqStats {
        total: n("total"),
        passing: n("passing"),
        failing: n("failing"),
        untested: n("untested"),
    }
}

fn parse_requirements(value: &Value) -> Vec<Requirement> {
    items(value, Some("requirements"))
        .filter_map(|r| {
            Some(Requirement {
                id: opt(r, "id")?,
                title: s(r, "title"),
                status: s(r, "status"),
                priority: opt(r, "priority"),
            })
        })
        .collect()
}

fn parse_matrix(matrix: &Value, view: &Value) -> MatrixSummary {
    let summary = &view["summary"];
    let n = |key: &str| summary[key].as_i64().unwrap_or(0);
    MatrixSummary {
        id: s(matrix, "id"),
        name: s(matrix, "name"),
        layer: s(matrix, "layer"),
        violated: n("violated"),
        verified: n("verified"),
        specified: n("specified"),
        unspecified: n("unspecified"),
    }
}

fn parse_stale_flags(value: &Value) -> Vec<StaleFlag> {
    items(value, None)
        .filter_map(|f| {
            Some(StaleFlag {
                id: opt(f, "id")?,
                chunk_id: s(f, "chunkId"),
                chunk_title: s(f, "chunkTitle"),
                reason: s(f, "reason"),
                detail: opt(f, "detail"),
            })
        })
        .collect()
}

fn parse_proposals(value: &Value) -> Vec<Proposal> {
    items(value, None)
        .filter_map(|p| {
            Some(Proposal {
                id: opt(p, "id")?,
                chunk_id: s(p, "chunkId"),
                chunk_title: s(p, "chunkTitle"),
                reason: opt(p, "reason"),
                fields: p["changes"]
                    .as_object()
                    .map(|changes| {
                        changes
                            .iter()
                            .filter(|(_, v)| !v.is_null())
                            .map(|(k, _)| k.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn parse_features(value: &Value) -> Vec<Feature> {
    items(value, None)
        .filter_map(|f| {
            Some(Feature {
                id: opt(f, "id")?,
                name: s(f, "name"),
                status: s(f, "status"),
                delta_count: f["deltaCount"].as_i64().unwrap_or(0),
            })
        })
        .collect()
}

fn chunk_ref(c: &Value) -> ChunkRef {
    ChunkRef {
        id: s(c, "id"),
        title: s(c, "title"),
        kind: s(c, "type"),
        stale: c["isStale"].as_bool().unwrap_or(false),
        match_reason: opt(c, "matchReason"),
    }
}

fn parse_sections(value: &Value) -> Vec<(String, Vec<ChunkRef>)> {
    items(value, Some("sections"))
        .map(|section| {
            (
                s(section, "title"),
                items(section, Some("chunks")).map(chunk_ref).collect(),
            )
        })
        .filter(|(_, chunks): &(String, Vec<ChunkRef>)| !chunks.is_empty())
        .collect()
}

fn parse_legacy_chunks(value: &Value) -> Vec<ChunkRef> {
    items(value, Some("chunks")).map(chunk_ref).collect()
}

/// A fake fubbik server for tests in any module.
#[cfg(test)]
pub(crate) mod test_server {
    /// A tiny HTTP server: path (no query) → (status, body). Each response
    /// closes its connection. Returns the base URL and the request log.
    pub(crate) fn fake_server(
        routes: Vec<(&'static str, u16, String)>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = header.to_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or_default().to_string();
                let target = parts.next().unwrap_or_default().to_string();
                let path = target.split('?').next().unwrap_or_default().to_string();
                seen.lock().unwrap().push(format!("{method} {target}"));
                let (status, text) = routes
                    .iter()
                    .find(|(p, ..)| *p == path)
                    .map(|(_, status, body)| (*status, body.clone()))
                    .unwrap_or((404, r#"{"message":"not found"}"#.into()));
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                    text.len()
                );
            }
        });
        (base, log)
    }
}

#[cfg(test)]
mod tests {
    use super::test_server::fake_server;
    use super::*;

    #[test]
    fn parses_plan_rows() {
        // Given
        let value = json!([
            {"id": "p1", "title": "Ship desktop", "status": "in_progress", "taskDone": 2, "taskTotal": 5},
            {"title": "no id"}
        ]);

        // When
        let plans = parse_plans(&value);

        // Then
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].title, "Ship desktop");
        assert_eq!((plans[0].task_done, plans[0].task_total), (2, 5));
    }

    #[test]
    fn parses_requirement_stats() {
        // Given
        let value = json!({"total": 9, "passing": 5, "failing": 1, "untested": 3});

        // When
        let stats = parse_req_stats(&value);

        // Then
        assert_eq!(
            stats,
            ReqStats {
                total: 9,
                passing: 5,
                failing: 1,
                untested: 3
            }
        );
    }

    #[test]
    fn empty_detect_body_is_none() {
        // Given
        let bodies = ["", r#"{"id":"s1","name":"fubbik"}"#];

        // When
        let parsed: Vec<_> = bodies.iter().map(|b| parse_space(b)).collect();

        // Then
        assert_eq!(parsed[0], None);
        assert_eq!(
            parsed[1],
            Some(Space {
                id: "s1".into(),
                name: "fubbik".into()
            })
        );
    }

    #[test]
    fn board_links_session_to_claimed_task() {
        // Given
        let value = json!({
            "plan": {"id": "p1", "title": "Desktop"},
            "tasks": [{"id": "t1", "title": "Prune worktrees", "status": "in_progress"}, {"id": "t2", "title": "Other", "status": "pending"}],
            "runs": [
                {"id": "r1", "handle": "claude-a", "externalKey": "sess-1", "status": "active", "metadata": {}},
                {"id": "r2", "handle": "claude-b", "externalKey": null, "status": "active", "metadata": {"sessionId": "sess-2"}}
            ],
            "claims": [
                {"taskId": "t1", "agentRunId": "r1", "expired": false},
                {"taskId": "t2", "agentRunId": "r2", "expired": true}
            ]
        });
        let knowledge = Knowledge {
            boards: vec![parse_board(&value)],
            ..Default::default()
        };

        // When
        let (board, run) = knowledge.run_for_session("sess-1").unwrap();
        let (board2, run2) = knowledge.run_for_session("sess-2").unwrap();

        // Then
        assert_eq!(run.handle, "claude-a");
        assert_eq!(board.tasks_of(&run.id)[0].title, "Prune worktrees");
        assert!(board2.tasks_of(&run2.id).is_empty()); // the claim expired
        assert_eq!(knowledge.active_agents(), 2);
    }

    #[test]
    fn plan_detail_carries_dependencies_timestamps_and_link() {
        // Given
        let value = json!({
            "plan": {"id": "p1", "title": "Desktop", "status": "ready"},
            "tasks": [
                {"id": "t1", "title": "A", "status": "done", "createdAt": "2026-10-30T10:00:00Z",
                 "metadata": {"other": 1, "fubbikDesktop": {"branch": "task/a", "baseCommit": "abc", "worktree": "/w/a"}}},
                {"id": "t2", "title": "B", "status": "pending", "metadata": {}}
            ],
            "dependencies": [{"id": "d1", "taskId": "t2", "dependsOnTaskId": "t1"}]
        });

        // When
        let plan = parse_plan_detail(&value);

        // Then
        assert_eq!(plan.tasks[1].depends_on, ["t1"]);
        assert_eq!(
            plan.tasks[0].created_at.as_deref(),
            Some("2026-10-30T10:00:00Z")
        );
        assert_eq!(plan.tasks[0].link.branch.as_deref(), Some("task/a"));
        assert!(
            plan.tasks[0]
                .metadata_raw
                .as_deref()
                .unwrap()
                .contains("\"other\":1")
        );
    }

    #[test]
    fn task_writes_patch_status_criteria_and_merged_metadata() {
        // Given
        let (base, log) = fake_server(vec![("/api/plans/p1/tasks/t1", 200, "{}".into())]);
        let client = FubbikClient::new(base);
        let task = Task {
            id: "t1".into(),
            metadata_raw: Some(r#"{"keep":true}"#.into()),
            ..Default::default()
        };

        // When
        client.set_task_status("p1", "t1", "in_progress").unwrap();
        client
            .set_acceptance("p1", "t1", &[("works".into(), true)])
            .unwrap();
        client
            .set_task_link(
                "p1",
                &task,
                &TaskLink {
                    branch: Some("task/x".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        // Then
        let log = log.lock().unwrap();
        assert_eq!(
            log.iter()
                .filter(|r| r.starts_with("PATCH /api/plans/p1/tasks/t1"))
                .count(),
            3
        );
    }

    #[test]
    fn parses_plan_detail_acceptance_in_both_shapes() {
        // Given
        let value = json!({
            "plan": {"id": "p1", "title": "Desktop", "status": "ready"},
            "tasks": [{"id": "t1", "title": "A", "status": "pending",
                       "acceptanceCriteria": ["plain", {"text": "object", "done": true}]}]
        });

        // When
        let plan = parse_plan_detail(&value);

        // Then
        assert_eq!(
            plan.tasks[0].acceptance,
            [("plain".to_string(), false), ("object".to_string(), true)]
        );
    }

    #[test]
    fn parses_proposal_changed_fields_and_sections() {
        // Given
        let proposals = json!([{"id": "pr1", "chunkId": "c1", "chunkTitle": "Auth", "changes": {"title": "New", "content": null}}]);
        let sections = json!({"sections": [
            {"title": "Conventions", "chunks": [{"id": "c1", "title": "Use AppError", "type": "note", "isStale": true}]},
            {"title": "Empty", "chunks": []}
        ]});

        // When
        let parsed = parse_proposals(&proposals);
        let grouped = parse_sections(&sections);

        // Then
        assert_eq!(parsed[0].fields, ["title"]);
        assert_eq!(grouped.len(), 1);
        assert!(grouped[0].1[0].stale);
    }

    #[test]
    fn knowledge_fetch_end_to_end_with_switched_off_groups() {
        // Given: plans and coordination on; requirements, matrices, overlays off (404)
        let (base, log) = fake_server(vec![
            ("/api/chunks", 200, json!({"chunks": [], "total": 12}).to_string()),
            ("/api/chunks/stale/count", 200, "3".into()),
            ("/api/health/knowledge", 200, json!({"orphans": {"count": 2}, "thin": {"count": 1}, "staleEmbeddings": {"count": 4}}).to_string()),
            ("/api/plans", 200, json!([{"id": "p1", "title": "Desktop", "status": "in_progress", "taskDone": 0, "taskTotal": 1}]).to_string()),
            ("/api/plans/p1", 200, json!({"plan": {"id": "p1", "title": "Desktop", "status": "in_progress"}, "tasks": [{"id": "t1", "title": "Prune", "status": "pending", "acceptanceCriteria": []}]}).to_string()),
            ("/api/plans/p1/board", 200, json!({"plan": {"id": "p1", "title": "Desktop"}, "tasks": [], "runs": [{"id": "r1", "handle": "h", "status": "active", "metadata": {}}], "claims": []}).to_string()),
            ("/api/chunks/stale", 200, json!([{"id": "f1", "chunkId": "c1", "chunkTitle": "Old", "reason": "age"}]).to_string()),
            ("/api/proposals", 200, "[]".into()),
            ("/api/proposals/count", 200, json!({"pending": 0}).to_string()),
        ]);
        let client = FubbikClient::new(base);

        // When
        let k = client
            .knowledge(Space {
                id: "s1".into(),
                name: "fubbik".into(),
            })
            .unwrap();

        // Then
        assert_eq!(
            (k.chunks, k.stale, k.orphans, k.thin, k.stale_embeddings),
            (12, 3, 2, 1, 4)
        );
        assert_eq!(k.plan_details[0].tasks[0].title, "Prune");
        assert_eq!(k.active_agents(), 1);
        assert_eq!(k.stale_flags[0].chunk_title, "Old");
        assert_eq!(k.requirements, ReqStats::default());
        assert!(k.matrices.is_empty() && k.features.is_empty());
        let log = log.lock().unwrap();
        assert!(
            log.iter()
                .any(|r| r.starts_with("GET /api/chunks?") && r.contains("enrichment=missing"))
        );
        assert!(
            log.iter()
                .any(|r| r.starts_with("GET /api/requirements?") && r.contains("status=failing"))
        );
    }

    #[test]
    fn writes_hit_the_documented_routes() {
        // Given
        let (base, log) = fake_server(vec![
            (
                "/api/plans/p1/board/runs",
                200,
                json!({"id": "r9"}).to_string(),
            ),
            (
                "/api/plans/p1/board/tasks/t1/claim",
                200,
                json!({"action": "claim"}).to_string(),
            ),
            ("/api/plans/p1/board/tasks/t1/transition", 200, "{}".into()),
            ("/api/chunks/f1/dismiss-staleness", 200, "{}".into()),
            ("/api/matrices/m1/cells/c1/test-results", 201, "{}".into()),
        ]);
        let client = FubbikClient::new(base);

        // When
        let run = client
            .join_run("p1", "claude-1", "sess", json!({}))
            .unwrap();
        client.claim_task("p1", "t1", &run).unwrap();
        client
            .transition_task("p1", "t1", &run, "in_progress", "go")
            .unwrap();
        client.dismiss_stale("f1").unwrap();
        client
            .report_test("m1", "c1", "cargo test", true, "exit code 0")
            .unwrap();
        let gated = client.set_active_features(&[]);

        // Then
        assert_eq!(run, "r9");
        assert!(gated.unwrap_err().contains("not available"));
        let log = log.lock().unwrap();
        assert_eq!(log.iter().filter(|r| r.starts_with("POST ")).count(), 5);
        assert!(log.iter().any(|r| r == "PUT /api/features/active"));
    }
}
