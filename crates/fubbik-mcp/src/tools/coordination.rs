use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};

use crate::api::ApiClient;

const NAMES: [&str; 6] = [
    "join_board",
    "read_board",
    "claim_task",
    "update_board_task",
    "write_board_entry",
    "ack_board",
];

pub(crate) fn handles(name: &str) -> bool {
    NAMES.contains(&name)
}

pub(crate) async fn call(api: &ApiClient, name: &str, arguments: &Value) -> Result<Value> {
    match name {
        "join_board" => {
            let plan_id = required(arguments, "planId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/board/runs"),
                    without(arguments, &["planId"]),
                )
                .await?,
            )
        }
        "read_board" => {
            let plan_id = required(arguments, "planId")?;
            let mut query = Vec::new();
            push_string(&mut query, arguments, "runId", "runId");
            push_number(&mut query, arguments, "afterSequence", "afterSequence");
            push_number(&mut query, arguments, "limit", "limit");
            let board = api
                .get(&format!("/api/plans/{plan_id}/board"), &query)
                .await?;
            Ok(text_result(format_board(
                &board,
                arguments.get("runId").and_then(Value::as_str),
            )))
        }
        "claim_task" => {
            let plan_id = required(arguments, "planId")?;
            let task_id = required(arguments, "taskId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/board/tasks/{task_id}/claim"),
                    without(arguments, &["planId", "taskId"]),
                )
                .await?,
            )
        }
        "update_board_task" => {
            let plan_id = required(arguments, "planId")?;
            let task_id = required(arguments, "taskId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/board/tasks/{task_id}/transition"),
                    without(arguments, &["planId", "taskId"]),
                )
                .await?,
            )
        }
        "write_board_entry" => {
            let plan_id = required(arguments, "planId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/board/entries"),
                    without(arguments, &["planId"]),
                )
                .await?,
            )
        }
        "ack_board" => {
            let plan_id = required(arguments, "planId")?;
            let run_id = required(arguments, "runId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/board/runs/{run_id}/ack"),
                    without(arguments, &["planId", "runId"]),
                )
                .await?,
            )
        }
        _ => bail!("unknown coordination tool: {name}"),
    }
}

pub(crate) fn definitions() -> Vec<Value> {
    vec![
        tool(
            "join_board",
            "Join or reconnect to a persistent Plan board",
            object(
                json!({"planId":{"type":"string"},"handle":{"type":"string"},"parentRunId":{"type":"string"},"externalKey":{"type":"string"},"capabilities":{"type":"array","items":{"type":"string"}},"metadata":{"type":"object"}}),
                &["planId", "handle"],
            ),
        ),
        tool(
            "read_board",
            "Read persistent tasks, claims, agents, and journal changes",
            object(
                json!({"planId":{"type":"string"},"runId":{"type":"string"},"afterSequence":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":500}}),
                &["planId"],
            ),
        ),
        tool(
            "claim_task",
            "Claim, renew, or release a task lease",
            object(
                json!({"planId":{"type":"string"},"taskId":{"type":"string"},"runId":{"type":"string"},"action":{"type":"string","enum":["claim","renew","release"]},"leaseSeconds":{"type":"integer","minimum":60,"maximum":3600}}),
                &["planId", "taskId", "runId", "action"],
            ),
        ),
        tool(
            "update_board_task",
            "Transition a task held by this run",
            object(
                json!({"planId":{"type":"string"},"taskId":{"type":"string"},"runId":{"type":"string"},"status":{"type":"string","enum":["pending","in_progress","done","skipped","blocked"]},"note":{"type":"string"},"clientMutationId":{"type":"string"}}),
                &["planId", "taskId", "runId", "status", "clientMutationId"],
            ),
        ),
        tool(
            "write_board_entry",
            "Persist a board note or addressed message",
            object(
                json!({"planId":{"type":"string"},"runId":{"type":"string"},"kind":{"type":"string","enum":["note","question","answer","progress","decision","handoff","artifact","system"]},"body":{"type":"string"},"clientMutationId":{"type":"string"},"taskId":{"type":"string"},"recipientRunId":{"type":"string"},"replyToId":{"type":"string"},"metadata":{"type":"object"}}),
                &["planId", "runId", "kind", "body", "clientMutationId"],
            ),
        ),
        tool(
            "ack_board",
            "Persist this run's journal cursor and heartbeat",
            object(
                json!({"planId":{"type":"string"},"runId":{"type":"string"},"throughSequence":{"type":"integer","minimum":0},"status":{"type":"string","enum":["active","finished","abandoned"]}}),
                &["planId", "runId", "throughSequence"],
            ),
        ),
    ]
}

fn format_board(board: &Value, run_id: Option<&str>) -> String {
    let runs = array(board, "runs");
    let claims = array(board, "claims");
    let entries = array(board, "entries");
    let run_handle = |id: &str| {
        runs.iter()
            .find(|run| string(run, "id") == id)
            .map(|run| string(run, "handle"))
            .unwrap_or_else(|| id.to_string())
    };
    let mut lines = vec![
        format!("# {}", string(&board["plan"], "title")),
        format!(
            "Plan: {} ({})",
            string(&board["plan"], "id"),
            string(&board["plan"], "status")
        ),
        String::new(),
        "## Tasks".into(),
    ];
    if array(board, "tasks").is_empty() {
        lines.push("No tasks.".into());
    }
    for task in array(board, "tasks") {
        let claim = claims
            .iter()
            .find(|claim| string(claim, "taskId") == string(task, "id"));
        let lease = claim
            .map(|claim| {
                format!(
                    "{}, {} {}",
                    run_handle(&string(claim, "agentRunId")),
                    if claim["expired"].as_bool() == Some(true) {
                        "expired"
                    } else {
                        "until"
                    },
                    string(claim, "leaseExpiresAt")
                )
            })
            .unwrap_or_else(|| "unclaimed".into());
        lines.push(format!(
            "- [{}] {} ({}) — {lease}",
            string(task, "status"),
            string(task, "title"),
            string(task, "id")
        ));
    }
    lines.extend([String::new(), "## Agents".into()]);
    if runs.is_empty() {
        lines.push("No agent runs.".into());
    }
    for run in runs {
        let parent = run
            .get("parentRunId")
            .and_then(Value::as_str)
            .map(|id| format!(", parent {id}"))
            .unwrap_or_default();
        lines.push(format!(
            "- {} [{}] ({}{parent})",
            string(run, "handle"),
            string(run, "status"),
            string(run, "id")
        ));
    }
    let direct = entries
        .iter()
        .filter(|entry| {
            run_id.is_some_and(|id| entry.get("recipientRunId").and_then(Value::as_str) == Some(id))
        })
        .collect::<Vec<_>>();
    lines.extend([String::new(), "## Direct messages".into()]);
    if direct.is_empty() {
        lines.push("No new direct messages.".into());
    }
    for entry in direct {
        lines.push(format!(
            "- #{} {}: {}",
            scalar(entry, "sequence"),
            run_handle(&string(entry, "authorRunId")),
            string(entry, "body")
        ));
    }
    lines.extend([String::new(), "## Journal".into()]);
    if entries.is_empty() {
        lines.push("No new entries.".into());
    }
    for entry in entries {
        let scope = entry
            .get("taskId")
            .and_then(Value::as_str)
            .map(|id| format!(" task {id}"))
            .unwrap_or_default();
        let recipient = entry
            .get("recipientRunId")
            .and_then(Value::as_str)
            .map(|id| format!(" → {id}"))
            .unwrap_or_default();
        lines.push(format!(
            "- #{} [{}] {}{recipient}{scope}: {}",
            scalar(entry, "sequence"),
            string(entry, "kind"),
            string(entry, "authorRunId"),
            string(entry, "body")
        ));
    }
    lines.extend([
        String::new(),
        format!("Next cursor: {}", scalar(&board["cursor"], "nextSequence")),
    ]);
    if board["cursor"]["hasMore"].as_bool() == Some(true) {
        lines.push("More entries are available; read again from the next cursor.".into());
    }
    lines.join("\n")
}

fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("{key} must be a string"))
}
fn without(v: &Value, keys: &[&str]) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        for key in keys {
            o.remove(*key);
        }
    }
    v
}
fn push_string(q: &mut Vec<(&'static str, String)>, v: &Value, key: &str, param: &'static str) {
    if let Some(x) = v.get(key).and_then(Value::as_str) {
        q.push((param, x.into()));
    }
}
fn push_number(q: &mut Vec<(&'static str, String)>, v: &Value, key: &str, param: &'static str) {
    if let Some(x) = v.get(key).and_then(Value::as_i64) {
        q.push((param, x.to_string()));
    }
}
fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}
fn string(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
fn scalar(v: &Value, key: &str) -> String {
    v.get(key)
        .map(|x| {
            if let Some(s) = x.as_str() {
                s.into()
            } else {
                x.to_string()
            }
        })
        .unwrap_or_default()
}
fn text_result(text: String) -> Value {
    json!({"content":[{"type":"text","text":text}],"isError":false})
}
fn json_result(value: Value) -> Result<Value> {
    Ok(
        json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&value)?}],"structuredContent":value,"isError":false}),
    )
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({"name":name,"description":description,"inputSchema":input_schema})
}

#[cfg(test)]
mod tests {
    use super::call;
    use crate::api::ApiClient;
    use serde_json::json;

    #[tokio::test]
    async fn read_board_formats_tasks_agents_messages_and_cursor() {
        // Given a persistent board with a claimed task and direct message
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/plans/plan-1/board"))
            .and(wiremock::matchers::query_param("runId", "run-1"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "plan":{"id":"plan-1","title":"Migration","status":"in_progress"},
                "tasks":[{"id":"task-1","title":"Port MCP","status":"in_progress"}],
                "runs":[{"id":"run-1","handle":"codex","status":"active","parentRunId":null}],
                "claims":[{"taskId":"task-1","agentRunId":"run-1","expired":false,"leaseExpiresAt":"tomorrow"}],
                "entries":[{"sequence":4,"authorRunId":"run-1","recipientRunId":"run-1","taskId":"task-1","kind":"progress","body":"Halfway"}],
                "cursor":{"nextSequence":5,"hasMore":false}
            })))
            .mount(&api)
            .await;

        // When the board is read through the MCP tool
        let result = call(
            &ApiClient::new(api.uri()),
            "read_board",
            &json!({"planId":"plan-1","runId":"run-1"}),
        )
        .await
        .unwrap();

        // Then the agent receives the compact Markdown board
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("# Migration"));
        assert!(text.contains("codex, until tomorrow"));
        assert!(text.contains("## Direct messages"));
        assert!(text.contains("Next cursor: 5"));
    }

    #[tokio::test]
    async fn task_transition_removes_path_identifiers_from_the_body() {
        // Given a transition endpoint expecting only mutation fields
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path(
                "/api/plans/plan-1/board/tasks/task-1/transition",
            ))
            .and(wiremock::matchers::body_json(
                json!({"runId":"run-1","status":"done","clientMutationId":"mutation-1"}),
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(json!({"task":{"id":"task-1","status":"done"}})),
            )
            .mount(&api)
            .await;

        // When the board task tool transitions the task
        let result = call(&ApiClient::new(api.uri()), "update_board_task", &json!({"planId":"plan-1","taskId":"task-1","runId":"run-1","status":"done","clientMutationId":"mutation-1"})).await.unwrap();

        // Then the structured API response is preserved
        assert_eq!(result["structuredContent"]["task"]["status"], "done");
    }
}
