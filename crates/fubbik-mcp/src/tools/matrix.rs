use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};

use crate::api::ApiClient;

const NAMES: [&str; 12] = [
    "list_matrices",
    "get_matrix_view",
    "create_matrix",
    "add_dimension",
    "add_rule",
    "toggle_cell",
    "link_cell_requirement",
    "link_cell_code",
    "record_test_result",
    "get_rule_history",
    "get_behaviors_for_file",
    "get_matrix_gaps",
];

pub(crate) fn handles(name: &str) -> bool {
    NAMES.contains(&name)
}

pub(crate) async fn call(api: &ApiClient, name: &str, a: &Value) -> Result<Value> {
    let value = match name {
        "list_matrices" => {
            let mut q = Vec::new();
            push_string(&mut q, a, "spaceId", "spaceId");
            push_string(&mut q, a, "layer", "layer");
            api.get("/api/matrices", &q).await?
        }
        "get_matrix_view" => {
            api.get(
                &format!("/api/matrices/{}/view", required(a, "matrixId")?),
                &[],
            )
            .await?
        }
        "create_matrix" => api.send(Method::POST, "/api/matrices", a.clone()).await?,
        "add_dimension" => {
            let id = required(a, "matrixId")?;
            api.send(
                Method::POST,
                &format!("/api/matrices/{id}/dimensions"),
                without(a, &["matrixId"]),
            )
            .await?
        }
        "add_rule" => {
            let id = required(a, "matrixId")?;
            api.send(
                Method::POST,
                &format!("/api/matrices/{id}/rules"),
                without(a, &["matrixId"]),
            )
            .await?
        }
        "toggle_cell" => {
            let id = required(a, "matrixId")?;
            api.send(
                Method::PUT,
                &format!("/api/matrices/{id}/cells"),
                without(a, &["matrixId"]),
            )
            .await?
        }
        "link_cell_requirement" => {
            let id = required(a, "matrixId")?;
            let cell = required(a, "cellId")?;
            api.send(
                Method::POST,
                &format!("/api/matrices/{id}/cells/{cell}/requirements"),
                without(a, &["matrixId", "cellId"]),
            )
            .await?
        }
        "link_cell_code" => {
            let id = required(a, "matrixId")?;
            let cell = required(a, "cellId")?;
            api.send(
                Method::POST,
                &format!("/api/matrices/{id}/cells/{cell}/code"),
                without(a, &["matrixId", "cellId"]),
            )
            .await?
        }
        "record_test_result" => {
            let id = required(a, "matrixId")?;
            let cell = required(a, "cellId")?;
            api.send(
                Method::POST,
                &format!("/api/matrices/{id}/cells/{cell}/test-results"),
                without(a, &["matrixId", "cellId"]),
            )
            .await?
        }
        "get_rule_history" => {
            let id = required(a, "matrixId")?;
            let rule = required(a, "ruleId")?;
            api.get(&format!("/api/matrices/{id}/rules/{rule}/history"), &[])
                .await?
        }
        "get_behaviors_for_file" => {
            api.get(
                "/api/matrices/behaviors-for-file",
                &[("path", required(a, "path")?.into())],
            )
            .await?
        }
        "get_matrix_gaps" => {
            let view = api
                .get(
                    &format!("/api/matrices/{}/view", required(a, "matrixId")?),
                    &[],
                )
                .await?;
            matrix_gaps(&view)
        }
        _ => bail!("unknown matrix tool: {name}"),
    };
    json_result(value)
}

pub(crate) fn definitions() -> Vec<Value> {
    vec![
        tool(
            "list_matrices",
            "List behavioral specification matrices",
            object(
                json!({"spaceId":{"type":"string"},"layer":{"type":"string","enum":["invariant","contract"]}}),
                &[],
            ),
        ),
        tool(
            "get_matrix_view",
            "Get a full behavioral matrix grid",
            object(json!({"matrixId":{"type":"string"}}), &["matrixId"]),
        ),
        tool(
            "create_matrix",
            "Create a behavioral specification matrix",
            object(
                json!({"name":{"type":"string"},"layer":{"type":"string","enum":["invariant","contract"]},"description":{"type":"string"},"spaceId":{"type":"string"}}),
                &["name", "layer"],
            ),
        ),
        tool(
            "add_dimension",
            "Add a dimension to a matrix",
            object(
                json!({"matrixId":{"type":"string"},"name":{"type":"string"}}),
                &["matrixId", "name"],
            ),
        ),
        tool(
            "add_rule",
            "Add a rule to a matrix",
            object(
                json!({"matrixId":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"category":{"type":"string"},"rationale":{"type":"string"},"alternatives":{"type":"string"},"consequences":{"type":"string"},"counterexample":{"type":"string"}}),
                &["matrixId", "title"],
            ),
        ),
        tool(
            "toggle_cell",
            "Toggle a matrix cell",
            object(
                json!({"matrixId":{"type":"string"},"ruleId":{"type":"string"},"dimensionId":{"type":"string"}}),
                &["matrixId", "ruleId", "dimensionId"],
            ),
        ),
        tool(
            "link_cell_requirement",
            "Link a requirement to a matrix cell",
            object(
                json!({"matrixId":{"type":"string"},"cellId":{"type":"string"},"requirementId":{"type":"string"}}),
                &["matrixId", "cellId", "requirementId"],
            ),
        ),
        tool(
            "link_cell_code",
            "Link a code reference to a matrix cell",
            object(
                json!({"matrixId":{"type":"string"},"cellId":{"type":"string"},"kind":{"type":"string","enum":["file","symbol","test"]},"ref":{"type":"string"}}),
                &["matrixId", "cellId", "kind", "ref"],
            ),
        ),
        tool(
            "record_test_result",
            "Record a matrix cell test result",
            object(
                json!({"matrixId":{"type":"string"},"cellId":{"type":"string"},"testRef":{"type":"string"},"status":{"type":"string","enum":["pass","fail"]},"detail":{"type":"string"}}),
                &["matrixId", "cellId", "testRef", "status"],
            ),
        ),
        tool(
            "get_rule_history",
            "Get the version history of a matrix rule",
            object(
                json!({"matrixId":{"type":"string"},"ruleId":{"type":"string"}}),
                &["matrixId", "ruleId"],
            ),
        ),
        tool(
            "get_behaviors_for_file",
            "Find behavioral rules governing a file",
            object(json!({"path":{"type":"string"}}), &["path"]),
        ),
        tool(
            "get_matrix_gaps",
            "Get unspecified and violated matrix cells",
            object(json!({"matrixId":{"type":"string"}}), &["matrixId"]),
        ),
    ]
}

fn matrix_gaps(view: &Value) -> Value {
    let dims = map_names(view, "dimensions");
    let rules = map_names(view, "rules");
    let mut gaps = Vec::new();
    if let Some(cells) = view.get("cells").and_then(Value::as_object) {
        for (key, cell) in cells {
            let status = cell.get("status").and_then(Value::as_str);
            if matches!(status, Some("unspecified" | "violated")) {
                let (mut rule, mut dim) = ("", "");
                if let Some((r, d)) = key.split_once(':') {
                    rule = r;
                    dim = d;
                }
                gaps.push(json!({"rule":rules.get(rule).cloned().unwrap_or_else(||rule.into()),"dimension":dims.get(dim).cloned().unwrap_or_else(||dim.into()),"status":status.unwrap_or_default()}));
            }
        }
    }
    json!({"matrix":view["matrix"]["name"],"summary":view["summary"],"gaps":gaps})
}
fn map_names(view: &Value, field: &str) -> std::collections::HashMap<String, String> {
    view.get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some((
                v.get("id")?.as_str()?.into(),
                v.get("name").or_else(|| v.get("title"))?.as_str()?.into(),
            ))
        })
        .collect()
}
fn required<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .with_context(|| format!("{k} must be a string"))
}
fn without(v: &Value, keys: &[&str]) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        for k in keys {
            o.remove(*k);
        }
    }
    v
}
fn push_string(q: &mut Vec<(&'static str, String)>, v: &Value, k: &str, p: &'static str) {
    if let Some(x) = v.get(k).and_then(Value::as_str) {
        q.push((p, x.into()));
    }
}
fn json_result(v: Value) -> Result<Value> {
    Ok(
        json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&v)?}],"structuredContent":v,"isError":false}),
    )
}
fn object(p: Value, r: &[&str]) -> Value {
    json!({"type":"object","properties":p,"required":r,"additionalProperties":false})
}
fn tool(n: &str, d: &str, s: Value) -> Value {
    json!({"name":n,"description":d,"inputSchema":s})
}

#[cfg(test)]
mod tests {
    use super::matrix_gaps;
    use serde_json::json;
    #[test]
    fn gaps_resolve_rule_and_dimension_names() {
        // Given a matrix view with violated and specified cells
        let view = json!({"matrix":{"name":"Auth"},"dimensions":[{"id":"d1","name":"Admin"}],"rules":[{"id":"r1","title":"Require MFA"}],"cells":{"r1:d1":{"status":"violated"},"other":{"status":"specified"}},"summary":{"violated":1}}); // When gaps are derived
        let gaps = matrix_gaps(&view); // Then only actionable cells remain with human names
        assert_eq!(
            gaps["gaps"],
            json!([{"rule":"Require MFA","dimension":"Admin","status":"violated"}])
        );
    }
}
