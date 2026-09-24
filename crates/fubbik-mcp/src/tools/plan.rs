use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};

use crate::api::ApiClient;

const NAMES: [&str; 13] = [
    "create_plan",
    "list_plans",
    "get_plan",
    "update_plan",
    "link_requirement",
    "unlink_requirement",
    "add_analyze_item",
    "update_analyze_item",
    "delete_analyze_item",
    "add_plan_task",
    "update_plan_task",
    "delete_plan_task",
    "link_plan_task_chunk",
];

pub(crate) fn handles(name: &str) -> bool {
    NAMES.contains(&name)
}

pub(crate) async fn call(api: &ApiClient, name: &str, arguments: &Value) -> Result<Value> {
    match name {
        "create_plan" => json_result(
            api.send(Method::POST, "/api/plans", arguments.clone())
                .await?,
        ),
        "list_plans" => {
            let mut query = Vec::new();
            push_string(&mut query, arguments, "spaceId", "spaceId");
            push_string(&mut query, arguments, "status", "status");
            push_string(&mut query, arguments, "requirementId", "requirementId");
            json_result(api.get("/api/plans", &query).await?)
        }
        "get_plan" => {
            let plan_id = required(arguments, "planId")?;
            json_result(api.get(&format!("/api/plans/{plan_id}"), &[]).await?)
        }
        "update_plan" => {
            let plan_id = required(arguments, "planId")?;
            json_result(
                api.send(
                    Method::PATCH,
                    &format!("/api/plans/{plan_id}"),
                    without(arguments, &["planId"]),
                )
                .await?,
            )
        }
        "link_requirement" => {
            let plan_id = required(arguments, "planId")?;
            required(arguments, "requirementId")?;
            api.send(
                Method::POST,
                &format!("/api/plans/{plan_id}/requirements"),
                without(arguments, &["planId"]),
            )
            .await?;
            Ok(text_result("Requirement linked"))
        }
        "unlink_requirement" => {
            let plan_id = required(arguments, "planId")?;
            let requirement_id = required(arguments, "requirementId")?;
            api.send(
                Method::DELETE,
                &format!("/api/plans/{plan_id}/requirements/{requirement_id}"),
                json!({}),
            )
            .await?;
            Ok(text_result("Requirement unlinked"))
        }
        "add_analyze_item" => {
            let plan_id = required(arguments, "planId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/analyze"),
                    without(arguments, &["planId"]),
                )
                .await?,
            )
        }
        "update_analyze_item" => {
            let plan_id = required(arguments, "planId")?;
            let item_id = required(arguments, "itemId")?;
            json_result(
                api.send(
                    Method::PATCH,
                    &format!("/api/plans/{plan_id}/analyze/{item_id}"),
                    without(arguments, &["planId", "itemId"]),
                )
                .await?,
            )
        }
        "delete_analyze_item" => {
            let plan_id = required(arguments, "planId")?;
            let item_id = required(arguments, "itemId")?;
            api.send(
                Method::DELETE,
                &format!("/api/plans/{plan_id}/analyze/{item_id}"),
                json!({}),
            )
            .await?;
            Ok(text_result("Analyze item deleted"))
        }
        "add_plan_task" => {
            let plan_id = required(arguments, "planId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/tasks"),
                    without(arguments, &["planId"]),
                )
                .await?,
            )
        }
        "update_plan_task" => {
            let plan_id = required(arguments, "planId")?;
            let task_id = required(arguments, "taskId")?;
            json_result(
                api.send(
                    Method::PATCH,
                    &format!("/api/plans/{plan_id}/tasks/{task_id}"),
                    without(arguments, &["planId", "taskId"]),
                )
                .await?,
            )
        }
        "delete_plan_task" => {
            let plan_id = required(arguments, "planId")?;
            let task_id = required(arguments, "taskId")?;
            api.send(
                Method::DELETE,
                &format!("/api/plans/{plan_id}/tasks/{task_id}"),
                json!({}),
            )
            .await?;
            Ok(text_result("Task deleted"))
        }
        "link_plan_task_chunk" => {
            let plan_id = required(arguments, "planId")?;
            let task_id = required(arguments, "taskId")?;
            json_result(
                api.send(
                    Method::POST,
                    &format!("/api/plans/{plan_id}/tasks/{task_id}/chunks"),
                    without(arguments, &["planId", "taskId"]),
                )
                .await?,
            )
        }
        _ => bail!("unknown plan tool: {name}"),
    }
}

pub(crate) fn definitions() -> Vec<Value> {
    let plan_id = || json!({"planId":{"type":"string"}});
    vec![
        tool(
            "create_plan",
            "Create a new implementation plan",
            object(
                json!({"title":{"type":"string"},"description":{"type":"string"},"spaceId":{"type":"string"},"requirementIds":{"type":"array","items":{"type":"string"}}}),
                &["title"],
            ),
        ),
        tool(
            "list_plans",
            "List plans with optional filters",
            object(
                json!({"spaceId":{"type":"string"},"status":{"type":"string"},"requirementId":{"type":"string"}}),
                &[],
            ),
        ),
        tool(
            "get_plan",
            "Get plan details including tasks and requirements",
            object(plan_id(), &["planId"]),
        ),
        tool(
            "update_plan",
            "Update plan title, description, or status",
            object(
                json!({"planId":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"status":{"type":"string","enum":plan_statuses()}}),
                &["planId"],
            ),
        ),
        tool(
            "link_requirement",
            "Link a requirement to a plan",
            object(
                json!({"planId":{"type":"string"},"requirementId":{"type":"string"}}),
                &["planId", "requirementId"],
            ),
        ),
        tool(
            "unlink_requirement",
            "Unlink a requirement from a plan",
            object(
                json!({"planId":{"type":"string"},"requirementId":{"type":"string"}}),
                &["planId", "requirementId"],
            ),
        ),
        tool(
            "add_analyze_item",
            "Add an item to the plan analyze phase",
            object(
                json!({"planId":{"type":"string"},"kind":{"type":"string","enum":["chunk","file","risk","assumption","question"]},"chunkId":{"type":"string"},"filePath":{"type":"string"},"text":{"type":"string"},"metadata":{"type":"object"}}),
                &["planId", "kind"],
            ),
        ),
        tool(
            "update_analyze_item",
            "Update an analyze item",
            object(
                json!({"planId":{"type":"string"},"itemId":{"type":"string"},"text":{"type":"string"},"metadata":{"type":"object"}}),
                &["planId", "itemId"],
            ),
        ),
        tool(
            "delete_analyze_item",
            "Delete an analyze item",
            object(
                json!({"planId":{"type":"string"},"itemId":{"type":"string"}}),
                &["planId", "itemId"],
            ),
        ),
        tool(
            "add_plan_task",
            "Add a task to a plan",
            object(
                json!({"planId":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"acceptanceCriteria":{"type":"array","items":{"type":"string"}},"chunks":{"type":"array","items":task_chunk()},"dependsOnTaskIds":{"type":"array","items":{"type":"string"}}}),
                &["planId", "title"],
            ),
        ),
        tool(
            "update_plan_task",
            "Update a task in a plan",
            object(
                json!({"planId":{"type":"string"},"taskId":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"acceptanceCriteria":{"type":"array","items":{"type":"string"}},"status":{"type":"string","enum":["pending","in_progress","done","skipped","blocked"]}}),
                &["planId", "taskId"],
            ),
        ),
        tool(
            "delete_plan_task",
            "Delete a task from a plan",
            object(
                json!({"planId":{"type":"string"},"taskId":{"type":"string"}}),
                &["planId", "taskId"],
            ),
        ),
        tool(
            "link_plan_task_chunk",
            "Link a chunk to a plan task",
            object(
                json!({"planId":{"type":"string"},"taskId":{"type":"string"},"chunkId":{"type":"string"},"relation":{"type":"string","enum":["context","created","modified"]}}),
                &["planId", "taskId", "chunkId", "relation"],
            ),
        ),
    ]
}

fn plan_statuses() -> Value {
    json!([
        "draft",
        "analyzing",
        "ready",
        "in_progress",
        "completed",
        "archived"
    ])
}
fn task_chunk() -> Value {
    object(
        json!({"chunkId":{"type":"string"},"relation":{"type":"string","enum":["context","created","modified"]}}),
        &["chunkId", "relation"],
    )
}
fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("{field} must be a string"))
}
fn without(value: &Value, fields: &[&str]) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            object.remove(*field);
        }
    }
    value
}
fn push_string(
    query: &mut Vec<(&'static str, String)>,
    value: &Value,
    field: &str,
    parameter: &'static str,
) {
    if let Some(value) = value.get(field).and_then(Value::as_str) {
        query.push((parameter, value.into()));
    }
}
fn text_result(text: &str) -> Value {
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
    async fn create_plan_forwards_the_plan_contract() {
        // Given an API accepting a plan with linked requirements
        let api = wiremock::MockServer::start().await;
        let body = json!({"title":"Rust cutover","description":"Remove runtime JS","requirementIds":["req-1"]});
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/plans"))
            .and(wiremock::matchers::body_json(body.clone()))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(json!({"id":"plan-1","title":"Rust cutover"})),
            )
            .mount(&api)
            .await;

        // When the create-plan tool runs
        let result = call(&ApiClient::new(api.uri()), "create_plan", &body)
            .await
            .unwrap();

        // Then it preserves the structured plan response
        assert_eq!(result["structuredContent"]["id"], "plan-1");
    }

    #[tokio::test]
    async fn plan_task_update_uses_an_unambiguous_tool_name_and_body() {
        // Given a plan task update endpoint
        let api = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("PATCH"))
            .and(wiremock::matchers::path("/api/plans/plan-1/tasks/task-1"))
            .and(wiremock::matchers::body_json(json!({"status":"done"})))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(json!({"id":"task-1","status":"done"})),
            )
            .mount(&api)
            .await;

        // When the explicitly plan-scoped task tool runs
        let result = call(
            &ApiClient::new(api.uri()),
            "update_plan_task",
            &json!({"planId":"plan-1","taskId":"task-1","status":"done"}),
        )
        .await
        .unwrap();

        // Then path identifiers are removed from the patch body
        assert_eq!(result["structuredContent"]["status"], "done");
    }
}
