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
    fn core_tool_catalog_has_unique_names_and_object_schemas() {
        // Given the migrated core tool catalog
        let catalog = tools();
        // When its names and schemas are inspected
        let mut names = catalog
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        // Then all nine core tools are unique and expose object schemas
        assert_eq!(count, 9);
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
}
