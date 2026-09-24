use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};

mod api;
mod protocol;
#[path = "tools/mod.rs"]
mod tool_groups;

use api::ApiClient;
pub use protocol::run;

const PROTOCOL_VERSION: &str = "2025-06-18";

pub struct Server {
    api: ApiClient,
}

impl Server {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            api: ApiClient::new(base_url),
        }
    }

    pub async fn handle(&self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

        id.as_ref()?;
        let id = id.unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "fubbik", "version": env!("CARGO_PKG_VERSION") }
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => self.call_tool(&params).await,
            _ => {
                return Some(error_response(
                    id,
                    -32601,
                    format!("method not found: {method}"),
                ));
            }
        };

        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => error_response(id, -32602, error.to_string()),
        })
    }

    async fn call_tool(&self, params: &Value) -> Result<Value> {
        let name = required_string(params, "name")?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            bail!("arguments must be an object");
        }
        if matches!(
            name,
            "sync_claude_md"
                | "get_context"
                | "get_context_for_task"
                | "create_context_snapshot"
                | "get_context_snapshot"
        ) {
            return self.call_context_tool(name, &arguments).await;
        }
        if matches!(
            name,
            "list_requirements"
                | "create_requirement"
                | "update_requirement_status"
                | "suggest_requirements"
                | "create_requirements_batch"
        ) {
            return self.call_requirement_tool(name, &arguments).await;
        }
        if matches!(name, "add_task" | "list_tasks" | "complete_task") {
            return self.call_task_tool(name, &arguments).await;
        }
        if tool_groups::coordination::handles(name) {
            return tool_groups::coordination::call(&self.api, name, &arguments).await;
        }
        if tool_groups::plan::handles(name) {
            return tool_groups::plan::call(&self.api, name, &arguments).await;
        }
        if tool_groups::matrix::handles(name) {
            return tool_groups::matrix::call(&self.api, name, &arguments).await;
        }
        let value = match name {
            "search_chunks" => {
                let mut query = vec![(
                    "limit",
                    arguments
                        .get("limit")
                        .and_then(Value::as_u64)
                        .unwrap_or(10)
                        .to_string(),
                )];
                push_string_query(&mut query, &arguments, "query", "search");
                push_string_query(&mut query, &arguments, "spaceId", "spaceId");
                push_string_query(&mut query, &arguments, "tags", "tags");
                self.api.get("/api/chunks", &query).await?
            }
            "get_chunk" => {
                let id = required_string(&arguments, "id")?;
                self.api.get(&format!("/api/chunks/{id}"), &[]).await?
            }
            "create_chunk" => {
                let mut body = arguments.clone();
                if let Some(space_id) = body
                    .as_object_mut()
                    .and_then(|object| object.remove("spaceId"))
                {
                    body["spaceIds"] = json!([space_id]);
                }
                self.api.send(Method::POST, "/api/chunks", body).await?
            }
            "get_conventions" => {
                let mut query = vec![("limit", "100".into())];
                push_string_query(&mut query, &arguments, "spaceId", "spaceId");
                conventions(self.api.get("/api/chunks", &query).await?)
            }
            "get_requirements" => {
                let mut query = Vec::new();
                push_string_query(&mut query, &arguments, "spaceId", "spaceId");
                push_string_query(&mut query, &arguments, "status", "status");
                self.api.get("/api/requirements", &query).await?
            }
            "update_chunk" => {
                let id = required_string(&arguments, "id")?;
                let mut body = arguments.clone();
                body.as_object_mut()
                    .expect("arguments are an object")
                    .remove("id");
                self.api
                    .send(Method::PATCH, &format!("/api/chunks/{id}"), body)
                    .await?
            }
            "list_updates" => {
                required_string(&arguments, "tag")?;
                let mut query = Vec::new();
                push_string_query(&mut query, &arguments, "tag", "tag");
                push_string_query(&mut query, &arguments, "spaceId", "spaceId");
                self.api.get("/api/chunks/updates", &query).await?
            }
            "propose_chunk_update" => {
                let id = required_string(&arguments, "chunkId")?;
                let body = json!({
                    "changes": arguments.get("changes").cloned().unwrap_or_else(|| json!({})),
                    "reason": arguments.get("reason").cloned().unwrap_or(Value::Null)
                });
                self.api
                    .send(Method::POST, &format!("/api/chunks/{id}/proposals"), body)
                    .await?
            }
            "search_vocabulary" => {
                let space_id = required_string(&arguments, "spaceId")?;
                let value = self
                    .api
                    .get("/api/vocabulary", &[("spaceId", space_id.to_string())])
                    .await?;
                filter_vocabulary(value, arguments.get("category").and_then(Value::as_str))
            }
            _ => bail!("unknown tool: {name}"),
        };

        Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value)? }],
            "structuredContent": value,
            "isError": false
        }))
    }

    async fn call_context_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let text = match name {
            "sync_claude_md" => {
                let mut query = Vec::new();
                push_string_query(&mut query, arguments, "tag", "tag");
                push_string_query(&mut query, arguments, "spaceId", "spaceId");
                let data = self.api.get("/api/chunks/export/claude-md", &query).await?;
                let chunks = data.get("chunks").and_then(Value::as_u64).unwrap_or(0);
                if chunks == 0 {
                    let tag = arguments
                        .get("tag")
                        .and_then(Value::as_str)
                        .unwrap_or("claude-context");
                    format!(
                        "No chunks tagged \"{tag}\" found. Tag some chunks with \"{tag}\" to generate CLAUDE.md content."
                    )
                } else {
                    format!(
                        "Generated CLAUDE.md content ({chunks} chunks):\n\n{}",
                        data.get("content").and_then(Value::as_str).unwrap_or("")
                    )
                }
            }
            "get_context" => self.get_context(arguments).await?,
            "get_context_for_task" => self.get_context_for_task(arguments).await?,
            "create_context_snapshot" => {
                let data = self
                    .api
                    .send(Method::POST, "/api/context/snapshot", arguments.clone())
                    .await?;
                format!(
                    "Snapshot created.\nsnapshotId: {}\nchunks: {}\ntokens: {}\ncreatedAt: {}\n\nUse get_context_snapshot with this ID to retrieve the frozen content.",
                    json_scalar(&data, "snapshotId"),
                    json_scalar(&data, "chunkCount"),
                    json_scalar(&data, "tokenCount"),
                    json_scalar(&data, "createdAt")
                )
            }
            "get_context_snapshot" => {
                let snapshot_id = required_string(arguments, "snapshotId")?;
                let data = self
                    .api
                    .get(&format!("/api/context/snapshot/{snapshot_id}"), &[])
                    .await?;
                format_snapshot(&data)
            }
            _ => bail!("unknown context tool: {name}"),
        };
        Ok(text_result(text))
    }

    async fn call_requirement_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let text = match name {
            "list_requirements" => {
                let mut query = Vec::new();
                push_string_query(&mut query, arguments, "status", "status");
                push_string_query(&mut query, arguments, "priority", "priority");
                push_string_query(&mut query, arguments, "spaceId", "spaceId");
                push_string_query(&mut query, arguments, "search", "search");
                let data = self.api.get("/api/requirements", &query).await?;
                let requirements = data
                    .get("requirements")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|requirement| {
                        json!({
                            "id": requirement.get("id").cloned().unwrap_or(Value::Null),
                            "title": requirement.get("title").cloned().unwrap_or(Value::Null),
                            "status": requirement.get("status").cloned().unwrap_or(Value::Null),
                            "priority": requirement.get("priority").cloned().unwrap_or(Value::Null),
                            "description": requirement.get("description").cloned().unwrap_or(Value::Null)
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::to_string_pretty(&json!({
                    "requirements": requirements,
                    "total": data.get("total").cloned().unwrap_or_else(|| json!(0))
                }))?
            }
            "create_requirement" => {
                required_string(arguments, "title")?;
                let steps = arguments
                    .get("steps")
                    .and_then(Value::as_array)
                    .filter(|steps| !steps.is_empty())
                    .context("steps must contain at least one Given/When/Then step")?;
                let mut body = arguments.clone();
                body["steps"] = Value::Array(steps.clone());
                let data = self
                    .api
                    .send(Method::POST, "/api/requirements", body)
                    .await?;
                let requirement = data.get("requirement").unwrap_or(&data);
                format!(
                    "Created requirement \"{}\" ({}). ID: {}",
                    json_scalar(requirement, "title"),
                    json_scalar(requirement, "status"),
                    json_scalar(requirement, "id")
                )
            }
            "update_requirement_status" => {
                let id = required_string(arguments, "requirementId")?;
                let status = required_string(arguments, "status")?;
                let data = self
                    .api
                    .send(
                        Method::PATCH,
                        &format!("/api/requirements/{id}/status"),
                        json!({"status": status}),
                    )
                    .await?;
                format!(
                    "Requirement \"{}\" status updated to {}. ID: {}",
                    json_scalar(&data, "title"),
                    json_scalar(&data, "status"),
                    json_scalar(&data, "id")
                )
            }
            "suggest_requirements" => {
                let mut query = Vec::new();
                push_string_query(&mut query, arguments, "focus", "focus");
                push_string_query(&mut query, arguments, "spaceId", "spaceId");
                let data = self
                    .api
                    .get("/api/requirements/suggest-context", &query)
                    .await?;
                format_suggestion_context(&data, optional_string(arguments, "focus"))
            }
            "create_requirements_batch" => {
                let requirements = arguments
                    .get("requirements")
                    .and_then(Value::as_array)
                    .filter(|requirements| !requirements.is_empty())
                    .context("requirements must contain at least one item")?;
                let mut body = arguments.clone();
                body["requirements"] = Value::Array(requirements.clone());
                let data = self
                    .api
                    .send(Method::POST, "/api/requirements/batch", body)
                    .await?;
                format_created_requirements(&data)
            }
            _ => bail!("unknown requirement tool: {name}"),
        };
        Ok(text_result(text))
    }

    async fn call_task_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let text = match name {
            "add_task" => {
                let title = required_string(arguments, "title")?;
                let task = self
                    .api
                    .send(Method::POST, "/api/tasks", arguments.clone())
                    .await?;
                format!(
                    "Task created: \"{title}\" (ID: {})",
                    json_scalar(&task, "id")
                )
            }
            "list_tasks" => {
                let tasks = self.api.get("/api/tasks", &[]).await?;
                let tasks = tasks.as_array().map(Vec::as_slice).unwrap_or_default();
                if tasks.is_empty() {
                    "No open tasks".into()
                } else {
                    let list = tasks
                        .iter()
                        .map(|task| {
                            format!(
                                "- [{}] {} ({})",
                                json_scalar(task, "status"),
                                json_scalar(task, "title"),
                                json_scalar(task, "id")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{} open task(s):\n{list}", tasks.len())
                }
            }
            "complete_task" => {
                let id = required_string(arguments, "taskId")?;
                self.api
                    .send(
                        Method::POST,
                        &format!("/api/tasks/{id}/complete"),
                        json!({"note": arguments.get("note").cloned().unwrap_or(Value::Null)}),
                    )
                    .await?;
                "Task completed".into()
            }
            _ => bail!("unknown task tool: {name}"),
        };
        Ok(text_result(text))
    }

    async fn get_context(&self, arguments: &Value) -> Result<String> {
        let plan_id = optional_string(arguments, "planId");
        let concept = optional_string(arguments, "concept");
        let file_path = optional_string(arguments, "filePath");
        let max_tokens = arguments
            .get("maxTokens")
            .and_then(Value::as_u64)
            .unwrap_or(8000)
            .to_string();
        let space_id = optional_string(arguments, "spaceId");
        let mut parts = Vec::new();

        if let Some(plan_id) = plan_id {
            let mut query = vec![
                ("planId", plan_id.to_string()),
                ("maxTokens", max_tokens.clone()),
                ("format", "structured-md".into()),
            ];
            push_optional_query(&mut query, "spaceId", space_id);
            parts.push(context_text(
                self.api.get("/api/context/for-plan", &query).await?,
            ));
            if let Some(file_path) = file_path {
                let mut query = vec![
                    ("paths", file_path.to_string()),
                    ("maxTokens", max_tokens),
                    ("format", "structured-md".into()),
                ];
                push_optional_query(&mut query, "spaceId", space_id);
                parts.push(context_text(
                    self.api.get("/api/context/for-files", &query).await?,
                ));
            }
        } else if let Some(concept) = concept {
            let mut query = vec![
                ("q", concept.to_string()),
                ("maxTokens", max_tokens),
                ("format", "structured-md".into()),
            ];
            push_optional_query(&mut query, "spaceId", space_id);
            parts.push(context_text(
                self.api.get("/api/context/about", &query).await?,
            ));
        } else if let Some(file_path) = file_path {
            let mut query = vec![
                ("paths", file_path.to_string()),
                ("maxTokens", max_tokens),
                ("format", "structured-md".into()),
            ];
            push_optional_query(&mut query, "spaceId", space_id);
            parts.push(context_text(
                self.api.get("/api/context/for-files", &query).await?,
            ));
        } else {
            return Ok("Provide at least one of: planId, concept, or filePath.".into());
        }

        Ok(parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n"))
    }

    async fn get_context_for_task(&self, arguments: &Value) -> Result<String> {
        let plan_id = required_string(arguments, "planId")?;
        let task_id = required_string(arguments, "taskId")?;
        let max_tokens = arguments
            .get("maxTokens")
            .and_then(Value::as_u64)
            .unwrap_or(4000)
            .to_string();
        let detail = self.api.get(&format!("/api/plans/{plan_id}"), &[]).await?;
        let task = detail
            .get("tasks")
            .and_then(Value::as_array)
            .and_then(|tasks| {
                tasks
                    .iter()
                    .find(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
            });
        let mut chunk_ids = Vec::new();
        if let Some(chunks) = task
            .and_then(|task| task.get("chunks"))
            .and_then(Value::as_array)
        {
            collect_chunk_ids(chunks, &mut chunk_ids);
        }
        if let Some(chunks) = detail
            .get("analyze")
            .and_then(|analyze| analyze.get("chunk"))
            .and_then(Value::as_array)
        {
            collect_chunk_ids(chunks, &mut chunk_ids);
        }
        chunk_ids.sort_unstable();
        chunk_ids.dedup();
        let context = context_text(
            self.api
                .get(
                    "/api/context/for-plan",
                    &[
                        ("planId", plan_id.to_string()),
                        ("maxTokens", max_tokens),
                        ("format", "structured-md".into()),
                    ],
                )
                .await?,
        );
        let header = if task.is_some() {
            format!(
                "# Context for Task: {task_id}\n\nRelevant chunk IDs: {}\n\n",
                if chunk_ids.is_empty() {
                    "none".into()
                } else {
                    chunk_ids.join(", ")
                }
            )
        } else {
            format!(
                "# Context for Plan: {plan_id}\n\nTask {task_id} not found — returning full plan context.\n\n"
            )
        };
        Ok(header + &context)
    }
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("{field} must be a string"))
}

fn optional_string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn push_optional_query(
    query: &mut Vec<(&'static str, String)>,
    parameter: &'static str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        query.push((parameter, value.to_string()));
    }
}

fn context_text(value: Value) -> String {
    match value {
        Value::String(text) => text,
        Value::Object(object) => object
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn collect_chunk_ids(chunks: &[Value], ids: &mut Vec<String>) {
    ids.extend(chunks.iter().filter_map(|chunk| {
        chunk
            .get("chunkId")
            .and_then(Value::as_str)
            .map(str::to_string)
    }));
}

fn json_scalar(value: &Value, field: &str) -> String {
    match value.get(field) {
        Some(Value::String(value)) => value.clone(),
        Some(value) if !value.is_null() => value.to_string(),
        _ => String::new(),
    }
}

fn format_snapshot(data: &Value) -> String {
    let chunks = data
        .get("chunks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut lines = vec![
        format!("# Context Snapshot: {}", json_scalar(data, "id")),
        format!(
            "> Created: {} | Tokens: {} | Chunks: {}",
            json_scalar(data, "createdAt"),
            json_scalar(data, "tokenCount"),
            chunks.len()
        ),
        String::new(),
    ];
    for chunk in chunks {
        lines.push(format!(
            "## {} [{}]",
            json_scalar(chunk, "title"),
            json_scalar(chunk, "type")
        ));
        if let Some(content) = optional_string(chunk, "content").filter(|value| !value.is_empty()) {
            lines.extend([String::new(), content.to_string()]);
        }
        if let Some(rationale) =
            optional_string(chunk, "rationale").filter(|value| !value.is_empty())
        {
            lines.extend([String::new(), format!("**Rationale:** {rationale}")]);
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string()
}

fn format_suggestion_context(data: &Value, focus: Option<&str>) -> String {
    let mut lines = vec!["# Knowledge Base Context for Requirement Suggestions".to_string()];
    if let Some(focus) = focus {
        lines.extend([String::new(), format!("**Focus area:** {focus}")]);
    }
    lines.extend([String::new(), "## Existing Requirements".into()]);
    for use_case in array_field(data, "useCases") {
        lines.extend([
            String::new(),
            format!("### {}", json_scalar(use_case, "name")),
        ]);
        for requirement in array_field(use_case, "requirements") {
            lines.push(format!(
                "- [{}] {}",
                json_scalar(requirement, "status"),
                json_scalar(requirement, "title")
            ));
        }
    }
    let ungrouped = array_field(data, "ungroupedRequirements");
    if !ungrouped.is_empty() {
        lines.extend([String::new(), "### Ungrouped".into()]);
        for requirement in ungrouped {
            lines.push(format!(
                "- [{}] {}",
                json_scalar(requirement, "status"),
                json_scalar(requirement, "title")
            ));
        }
    }

    let gaps = array_field(data, "coverageGaps");
    if !gaps.is_empty() {
        lines.extend([
            String::new(),
            "## Uncovered Chunks (no requirements linked)".into(),
            String::new(),
        ]);
        for gap in gaps {
            lines.push(format!(
                "- {} ({})",
                json_scalar(gap, "title"),
                json_scalar(gap, "id")
            ));
        }
    }

    let health = data.get("healthIssueCounts").unwrap_or(&Value::Null);
    lines.extend([
        String::new(),
        "## Knowledge Health".into(),
        String::new(),
        format!(
            "- Orphan chunks (no connections): {}",
            json_scalar(health, "orphan")
        ),
        format!(
            "- Stale chunks (>30 days old, neighbors updated): {}",
            json_scalar(health, "stale")
        ),
        format!(
            "- Thin chunks (<100 chars): {}",
            json_scalar(health, "thin")
        ),
    ]);

    let relevant = array_field(data, "relevantChunks");
    if !relevant.is_empty() {
        lines.extend([String::new(), "## Relevant Chunks".into()]);
        for chunk in relevant {
            lines.extend([
                String::new(),
                format!(
                    "### {} ({})",
                    json_scalar(chunk, "title"),
                    json_scalar(chunk, "id")
                ),
                json_scalar(chunk, "content"),
            ]);
        }
    }

    lines.extend([
        String::new(),
        "---".into(),
        String::new(),
        "Based on this context, suggest new requirements organized into use cases.".into(),
        "For each requirement, provide: title, Given/When/Then steps, priority, and which use case it belongs to.".into(),
        "When ready, call `create_requirements_batch` to create the approved requirements.".into(),
    ]);
    lines.join("\n")
}

fn format_created_requirements(data: &Value) -> String {
    let mut lines = vec![format!(
        "# Created {} Requirements",
        json_scalar(data, "created")
    )];
    let use_cases = array_field(data, "useCasesCreated");
    if !use_cases.is_empty() {
        lines.extend([
            String::new(),
            format!(
                "**Use cases auto-created:** {}",
                use_cases
                    .iter()
                    .map(|use_case| json_scalar(use_case, "name"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ]);
    }
    lines.extend([
        String::new(),
        "## Requirements Created".into(),
        String::new(),
    ]);
    for requirement in array_field(data, "requirements") {
        lines.push(format!(
            "- {} ({})",
            json_scalar(requirement, "title"),
            json_scalar(requirement, "id")
        ));
    }
    lines.join("\n")
}

fn array_field<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn text_result(text: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

fn push_string_query(
    query: &mut Vec<(&'static str, String)>,
    arguments: &Value,
    argument: &str,
    parameter: &'static str,
) {
    if let Some(value) = arguments.get(argument).and_then(Value::as_str) {
        query.push((parameter, value.to_string()));
    }
}

fn conventions(value: Value) -> Value {
    let tags = [
        "convention",
        "conventions",
        "pattern",
        "patterns",
        "standard",
        "standards",
        "guideline",
        "guidelines",
        "rule",
        "rules",
        "best-practice",
        "best-practices",
    ];
    let chunks = value
        .get("chunks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Value::Array(
        chunks
            .into_iter()
            .filter(|chunk| {
                chunk.get("rationale").is_some_and(|value| !value.is_null())
                    || chunk
                        .get("tags")
                        .and_then(Value::as_array)
                        .is_some_and(|chunk_tags| {
                            chunk_tags.iter().any(|tag| {
                                tag.get("name").and_then(Value::as_str).is_some_and(|name| {
                                    tags.contains(&name.to_lowercase().as_str())
                                })
                            })
                        })
            })
            .collect(),
    )
}

fn filter_vocabulary(mut value: Value, category: Option<&str>) -> Value {
    let Some(category) = category else {
        return value;
    };
    let Some(entries) = value.get_mut("entries").and_then(Value::as_array_mut) else {
        return value;
    };
    entries.retain(|entry| {
        entry
            .get("category")
            .and_then(Value::as_str)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(category))
    });
    value
}

pub(crate) fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

pub fn tools() -> Vec<Value> {
    let mut tools = vec![
        tool(
            "search_chunks",
            "Search the fubbik knowledge base for chunks",
            object(
                json!({
                    "query": {"type":"string"}, "spaceId": {"type":"string"},
                    "tags": {"type":"string"}, "limit": {"type":"integer", "minimum":1}
                }),
                &[],
            ),
        ),
        tool(
            "get_chunk",
            "Get full details for a chunk",
            object(json!({"id":{"type":"string"}}), &["id"]),
        ),
        tool(
            "create_chunk",
            "Create a knowledge chunk",
            object(
                json!({
                    "title":{"type":"string"}, "content":{"type":"string"}, "type":{"type":"string"},
                    "tags":{"type":"array","items":{"type":"string"}}, "spaceId":{"type":"string"},
                    "updateTag":{"type":"string"}
                }),
                &["title", "content"],
            ),
        ),
        tool(
            "get_conventions",
            "Get convention-related chunks",
            object(json!({"spaceId":{"type":"string"}}), &[]),
        ),
        tool(
            "get_requirements",
            "Get requirements for a space",
            object(
                json!({
                    "spaceId":{"type":"string"}, "status":{"type":"string"}
                }),
                &[],
            ),
        ),
        tool(
            "update_chunk",
            "Update a knowledge chunk",
            object(
                json!({
                    "id":{"type":"string"}, "title":{"type":"string"}, "content":{"type":"string"},
                    "type":{"type":"string"}, "rationale":{"type":"string"}, "updateTag":{"type":"string"}
                }),
                &["id"],
            ),
        ),
        tool(
            "list_updates",
            "List tagged knowledge updates",
            object(
                json!({
                    "tag":{"type":"string"}, "spaceId":{"type":"string"}
                }),
                &["tag"],
            ),
        ),
        tool(
            "propose_chunk_update",
            "Propose a chunk update for review",
            object(
                json!({
                    "chunkId":{"type":"string"}, "changes":{"type":"object"}, "reason":{"type":"string"}
                }),
                &["chunkId", "changes"],
            ),
        ),
        tool(
            "search_vocabulary",
            "Search controlled vocabulary",
            object(
                json!({
                    "spaceId":{"type":"string"}, "category":{"type":"string"}
                }),
                &["spaceId"],
            ),
        ),
        tool(
            "sync_claude_md",
            "Generate CLAUDE.md content from tagged chunks",
            object(
                json!({"tag":{"type":"string"}, "spaceId":{"type":"string"}}),
                &[],
            ),
        ),
        tool(
            "get_context",
            "Retrieve context for a plan, concept, or file",
            object(
                json!({
                    "planId":{"type":"string"}, "concept":{"type":"string"},
                    "filePath":{"type":"string"}, "maxTokens":{"type":"integer","minimum":1},
                    "spaceId":{"type":"string"}
                }),
                &[],
            ),
        ),
        tool(
            "get_context_for_task",
            "Get tightly scoped context for a task within a plan",
            object(
                json!({
                    "planId":{"type":"string"}, "taskId":{"type":"string"},
                    "maxTokens":{"type":"integer","minimum":1}
                }),
                &["planId", "taskId"],
            ),
        ),
        tool(
            "create_context_snapshot",
            "Freeze context into a persistent snapshot",
            object(
                json!({
                    "planId":{"type":"string"}, "taskId":{"type":"string"},
                    "filePaths":{"type":"array","items":{"type":"string"}},
                    "concept":{"type":"string"}, "maxTokens":{"type":"integer","minimum":1},
                    "spaceId":{"type":"string"}
                }),
                &[],
            ),
        ),
        tool(
            "get_context_snapshot",
            "Retrieve a frozen context snapshot",
            object(json!({"snapshotId":{"type":"string"}}), &["snapshotId"]),
        ),
        tool(
            "list_requirements",
            "List requirements with optional filters",
            object(
                json!({
                    "status":{"type":"string"}, "priority":{"type":"string"},
                    "spaceId":{"type":"string"}, "search":{"type":"string"}
                }),
                &[],
            ),
        ),
        tool(
            "create_requirement",
            "Create a requirement with Given/When/Then steps",
            object(
                json!({
                    "title":{"type":"string"}, "description":{"type":"string"},
                    "priority":{"type":"string","enum":["must","should","could","wont"]},
                    "steps":{"type":"array","minItems":1,"items": requirement_step_schema()},
                    "spaceId":{"type":"string"}
                }),
                &["title", "steps"],
            ),
        ),
        tool(
            "update_requirement_status",
            "Update a requirement status",
            object(
                json!({
                    "requirementId":{"type":"string"},
                    "status":{"type":"string","enum":["passing","failing","untested"]}
                }),
                &["requirementId", "status"],
            ),
        ),
        tool(
            "suggest_requirements",
            "Get knowledge context for suggesting requirements",
            object(
                json!({"focus":{"type":"string"}, "spaceId":{"type":"string"}}),
                &[],
            ),
        ),
        tool(
            "create_requirements_batch",
            "Create multiple requirements and resolve their use cases",
            object(
                json!({
                    "requirements":{
                        "type":"array", "minItems":1, "maxItems":50,
                        "items":{
                            "type":"object", "additionalProperties":false,
                            "properties":{
                                "title":{"type":"string"}, "description":{"type":"string"},
                                "steps":{"type":"array","minItems":1,"items":requirement_step_schema()},
                                "priority":{"type":"string","enum":["must","should","could","wont"]},
                                "useCaseId":{"type":"string"}, "useCaseName":{"type":"string"},
                                "parentUseCaseName":{"type":"string"}
                            },
                            "required":["title","steps"]
                        }
                    },
                    "spaceId":{"type":"string"}
                }),
                &["requirements"],
            ),
        ),
        tool(
            "add_task",
            "Add a quick task for tracking",
            object(
                json!({"title":{"type":"string"}, "description":{"type":"string"}}),
                &["title"],
            ),
        ),
        tool("list_tasks", "List open tasks", object(json!({}), &[])),
        tool(
            "complete_task",
            "Complete a task",
            object(
                json!({"taskId":{"type":"string"}, "note":{"type":"string"}}),
                &["taskId"],
            ),
        ),
    ];
    tools.extend(tool_groups::coordination::definitions());
    tools.extend(tool_groups::plan::definitions());
    tools.extend(tool_groups::matrix::definitions());
    tools
}

fn requirement_step_schema() -> Value {
    object(
        json!({
            "keyword":{"type":"string","enum":["given","when","then","and","but"]},
            "text":{"type":"string"}
        }),
        &["keyword", "text"],
    )
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

#[cfg(test)]
mod tests {
    use super::{Server, tools};
    use serde_json::json;

    #[tokio::test]
    async fn initialize_negotiates_a_tools_capable_server() {
        // Given a Rust MCP server
        let server = Server::new("http://localhost:3100");
        // When a client initializes the protocol
        let response = server
            .handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}))
            .await
            .unwrap();
        // Then the server advertises the MCP tool capability
        assert_eq!(response["result"]["serverInfo"]["name"], "fubbik");
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }

    #[test]
    fn migrated_tool_catalog_has_unique_names_and_object_schemas() {
        // Given the migrated core, context, and requirement tool catalog
        let catalog = tools();
        // When its names and schemas are inspected
        let mut names = catalog
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        // Then all fifty-three tools are unique and expose object schemas
        assert_eq!(count, 53);
        assert_eq!(names.len(), count);
        assert!(
            catalog
                .iter()
                .all(|tool| tool["inputSchema"]["type"] == "object")
        );
        let proposal = catalog
            .iter()
            .find(|tool| tool["name"] == "propose_chunk_update")
            .unwrap();
        assert_eq!(
            proposal["inputSchema"]["required"],
            json!(["chunkId", "changes"])
        );
        let vocabulary = catalog
            .iter()
            .find(|tool| tool["name"] == "search_vocabulary")
            .unwrap();
        assert_eq!(vocabulary["inputSchema"]["required"], json!(["spaceId"]));
        let create_requirement = catalog
            .iter()
            .find(|tool| tool["name"] == "create_requirement")
            .unwrap();
        assert_eq!(
            create_requirement["inputSchema"]["required"],
            json!(["title", "steps"])
        );
    }

    #[tokio::test]
    async fn search_chunks_calls_the_active_api() {
        // Given
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/chunks"))
            .and(wiremock::matchers::query_param("search", "rust"))
            .and(wiremock::matchers::query_param("limit", "5"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(json!({"chunks":[{"id":"chunk-1","title":"Rust"}],"total":1})),
            )
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":2, "method":"tools/call",
                "params":{"name":"search_chunks","arguments":{"query":"rust","limit":5}}
            }))
            .await
            .unwrap();

        // Then
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(
            response["result"]["structuredContent"]["chunks"][0]["id"],
            "chunk-1"
        );
    }

    #[tokio::test]
    async fn sync_claude_md_preserves_markdown_tool_output() {
        // Given
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/chunks/export/claude-md"))
            .and(wiremock::matchers::query_param("tag", "agent-context"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(json!({"content":"# Agent context","chunks":2})),
            )
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":3, "method":"tools/call",
                "params":{"name":"sync_claude_md","arguments":{"tag":"agent-context"}}
            }))
            .await
            .unwrap();

        // Then
        assert_eq!(
            response["result"]["content"][0]["text"],
            "Generated CLAUDE.md content (2 chunks):\n\n# Agent context"
        );
        assert!(response["result"].get("structuredContent").is_none());
    }

    #[tokio::test]
    async fn concept_context_uses_the_active_semantic_endpoint() {
        // Given
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/context/about"))
            .and(wiremock::matchers::query_param("q", "authentication"))
            .and(wiremock::matchers::query_param("maxTokens", "1200"))
            .and(wiremock::matchers::query_param("format", "structured-md"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(json!({"content":"# Authentication"})),
            )
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":4, "method":"tools/call",
                "params":{"name":"get_context","arguments":{"concept":"authentication","maxTokens":1200}}
            }))
            .await
            .unwrap();

        // Then
        assert_eq!(response["result"]["content"][0]["text"], "# Authentication");
    }

    #[tokio::test]
    async fn context_snapshot_creation_forwards_the_versioned_body() {
        // Given
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/context/snapshot"))
            .and(wiremock::matchers::body_json(json!({
                "planId":"plan-1", "taskId":"task-1", "maxTokens":3200
            })))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "snapshotId":"snapshot-1", "chunkCount":3, "tokenCount":900,
                "createdAt":"2026-09-24T10:00:00Z"
            })))
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":5, "method":"tools/call",
                "params":{"name":"create_context_snapshot","arguments":{
                    "planId":"plan-1", "taskId":"task-1", "maxTokens":3200
                }}
            }))
            .await
            .unwrap();

        // Then
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("snapshotId: snapshot-1"));
        assert!(text.contains("chunks: 3"));
        assert!(text.contains("tokens: 900"));
    }

    #[tokio::test]
    async fn frozen_context_is_rendered_as_markdown() {
        // Given
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/context/snapshot/snapshot-1"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "id":"snapshot-1", "createdAt":"2026-09-24T10:00:00Z", "tokenCount":42,
                "chunks":[{"title":"Authentication", "type":"decision", "content":"Use sessions", "rationale":"Auditable"}]
            })))
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":6, "method":"tools/call",
                "params":{"name":"get_context_snapshot","arguments":{"snapshotId":"snapshot-1"}}
            }))
            .await
            .unwrap();

        // Then
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("# Context Snapshot: snapshot-1"));
        assert!(text.contains("## Authentication [decision]"));
        assert!(text.contains("**Rationale:** Auditable"));
    }

    #[tokio::test]
    async fn requirement_status_uses_the_dedicated_status_endpoint() {
        // Given an API requirement that can transition to passing
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("PATCH"))
            .and(wiremock::matchers::path(
                "/api/requirements/requirement-1/status",
            ))
            .and(wiremock::matchers::body_json(json!({"status":"passing"})))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "id":"requirement-1", "title":"Login succeeds", "status":"passing"
            })))
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When the requirement status tool runs
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":7, "method":"tools/call",
                "params":{"name":"update_requirement_status","arguments":{
                    "requirementId":"requirement-1", "status":"passing"
                }}
            }))
            .await
            .unwrap();

        // Then it reports the updated requirement
        assert_eq!(
            response["result"]["content"][0]["text"],
            "Requirement \"Login succeeds\" status updated to passing. ID: requirement-1"
        );
    }

    #[tokio::test]
    async fn suggestion_context_is_rendered_from_the_backend_contract() {
        // Given suggestion context using the endpoint's healthIssueCounts contract
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/api/requirements/suggest-context",
            ))
            .and(wiremock::matchers::query_param("focus", "auth"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "useCases":[{"id":"uc-1","name":"Authentication","description":null,
                    "requirements":[{"id":"r-1","title":"Require MFA","status":"untested","priority":"must"}]}],
                "ungroupedRequirements":[{"id":"r-2","title":"Audit login","status":"failing","priority":null}],
                "coverageGaps":[{"id":"c-1","title":"Session expiry"}],
                "healthIssueCounts":{"orphan":2,"stale":1,"thin":3},
                "relevantChunks":[{"id":"c-2","title":"Auth policy","content":"Use MFA","type":"decision"}]
            })))
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When the suggestion tool requests a focused report
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":8, "method":"tools/call",
                "params":{"name":"suggest_requirements","arguments":{"focus":"auth"}}
            }))
            .await
            .unwrap();

        // Then the Markdown includes grouped, ungrouped, coverage, health, and chunk context
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("### Authentication"));
        assert!(text.contains("### Ungrouped"));
        assert!(text.contains("- Session expiry (c-1)"));
        assert!(text.contains("Orphan chunks (no connections): 2"));
        assert!(text.contains("### Auth policy (c-2)"));
    }

    #[tokio::test]
    async fn batch_creation_is_summarized_for_the_agent() {
        // Given an API that creates requirements and a use case in one batch
        let api = wiremock::MockServer::start().await;
        let request_body = json!({
            "requirements":[{
                "title":"Require MFA",
                "steps":[{"keyword":"given","text":"an account"}]
            }]
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/requirements/batch"))
            .and(wiremock::matchers::body_json(request_body.clone()))
            .respond_with(wiremock::ResponseTemplate::new(201).set_body_json(json!({
                "created":1,
                "requirements":[{"id":"r-1","title":"Require MFA","useCaseId":"uc-1"}],
                "useCasesCreated":[{"id":"uc-1","name":"Authentication","parentId":null}]
            })))
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When the batch creation tool runs
        let response = server
            .handle(json!({
                "jsonrpc":"2.0", "id":9, "method":"tools/call",
                "params":{"name":"create_requirements_batch","arguments":request_body}
            }))
            .await
            .unwrap();

        // Then it names both the created use case and requirement
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("# Created 1 Requirements"));
        assert!(text.contains("**Use cases auto-created:** Authentication"));
        assert!(text.contains("- Require MFA (r-1)"));
    }

    #[tokio::test]
    async fn quick_task_tools_use_the_compatibility_queue() {
        // Given the quick-task API supports create, list, and complete
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/tasks"))
            .and(wiremock::matchers::body_json(json!({
                "title":"Review migration", "description":"Check parity"
            })))
            .respond_with(wiremock::ResponseTemplate::new(201).set_body_json(json!({
                "id":"task-1", "title":"Review migration", "status":"in_progress"
            })))
            .mount(&api)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tasks"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!([{
                "id":"task-1", "title":"Review migration", "status":"in_progress"
            }])))
            .mount(&api)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/tasks/task-1/complete"))
            .and(wiremock::matchers::body_json(json!({"note":"Done"})))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "id":"task-1", "status":"completed"
            })))
            .mount(&api)
            .await;
        let server = Server::new(api.uri());

        // When an agent creates, lists, and completes the task
        let created = server
            .handle(json!({
                "jsonrpc":"2.0", "id":10, "method":"tools/call",
                "params":{"name":"add_task","arguments":{
                    "title":"Review migration", "description":"Check parity"
                }}
            }))
            .await
            .unwrap();
        let listed = server
            .handle(json!({
                "jsonrpc":"2.0", "id":11, "method":"tools/call",
                "params":{"name":"list_tasks","arguments":{}}
            }))
            .await
            .unwrap();
        let completed = server
            .handle(json!({
                "jsonrpc":"2.0", "id":12, "method":"tools/call",
                "params":{"name":"complete_task","arguments":{"taskId":"task-1","note":"Done"}}
            }))
            .await
            .unwrap();

        // Then every tool returns concise task-oriented output
        assert_eq!(
            created["result"]["content"][0]["text"],
            "Task created: \"Review migration\" (ID: task-1)"
        );
        assert_eq!(
            listed["result"]["content"][0]["text"],
            "1 open task(s):\n- [in_progress] Review migration (task-1)"
        );
        assert_eq!(completed["result"]["content"][0]["text"], "Task completed");
    }
}
