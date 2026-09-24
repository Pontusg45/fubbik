use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2025-06-18";

pub async fn run(base_url: &str) -> Result<()> {
    let server = Server::new(base_url);
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_message(
                    &mut stdout,
                    &error_response(Value::Null, -32700, format!("parse error: {error}")),
                )
                .await?;
                continue;
            }
        };

        if let Some(response) = server.handle(request).await {
            write_message(&mut stdout, &response).await?;
        }
    }
    Ok(())
}

async fn write_message(writer: &mut tokio::io::Stdout, message: &Value) -> Result<()> {
    writer
        .write_all(serde_json::to_string(message)?.as_bytes())
        .await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

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

struct ApiClient {
    base: String,
    http: reqwest::Client,
}

impl ApiClient {
    fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        let response = self
            .http
            .get(format!("{}{path}", self.base))
            .query(query)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;
        decode(response, path).await
    }

    async fn send(&self, method: Method, path: &str, body: Value) -> Result<Value> {
        let response = self
            .http
            .request(method, format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;
        decode(response, path).await
    }
}

async fn decode(response: reqwest::Response, path: &str) -> Result<Value> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("API {status} for {path}: {body}");
    }
    Ok(response.json().await?)
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

fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

pub fn tools() -> Vec<Value> {
    vec![
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
    ]
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
        // Given the migrated core and context tool catalog
        let catalog = tools();
        // When its names and schemas are inspected
        let mut names = catalog
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        // Then all fourteen tools are unique and expose object schemas
        assert_eq!(count, 14);
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
}
