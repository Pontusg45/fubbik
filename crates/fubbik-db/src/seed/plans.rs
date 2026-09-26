use anyhow::Result;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::spaces::DEV_USER_ID;

pub struct PlanSeed {
    pub plan_id: String,
    pub task_ids: BTreeMap<String, String>,
}

pub async fn seed_plans(
    tx: &mut Transaction<'_, Postgres>,
    requirement_ids: &BTreeMap<String, String>,
    chunk_ids: &BTreeMap<String, String>,
) -> Result<PlanSeed> {
    let space_id: String =
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_one(&mut **tx)
            .await?;
    let title = "Round 2: per-codebase vocabulary CRUD UI";
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM plan WHERE user_id = $1 AND title = $2")
            .bind(DEV_USER_ID)
            .bind(title)
            .fetch_optional(&mut **tx)
            .await?;
    let plan_id = existing.unwrap_or_else(|| "seed-plan-vocab-crud".to_string());
    sqlx::query(
        "INSERT INTO plan (id, title, description, status, user_id, space_id, metadata) \
         VALUES ($1, $2, $3, 'in_progress', $4, $5, $6) ON CONFLICT (id) DO UPDATE SET \
         description = EXCLUDED.description, metadata = EXCLUDED.metadata, updated_at = now()",
    )
    .bind(&plan_id)
    .bind(title)
    .bind("Ship /settings/vocabulary editors for chunk types + connection relations.")
    .bind(DEV_USER_ID)
    .bind(&space_id)
    .bind(serde_json::json!({"tokenEstimate": 15000, "effortHours": 6}))
    .execute(&mut **tx)
    .await?;
    if let Some(requirement_id) = requirement_ids.get("catalog-driven-types") {
        sqlx::query(
            "INSERT INTO plan_requirement (id, plan_id, requirement_id) VALUES ($1, $2, $3) \
             ON CONFLICT (plan_id, requirement_id) DO NOTHING",
        )
        .bind("seed-plan-vocab-requirement")
        .bind(&plan_id)
        .bind(requirement_id)
        .execute(&mut **tx)
        .await?;
    }
    let tasks = [
        (
            "schema",
            "Add chunk_type + connection_relation tables + seed builtins",
            "done",
        ),
        ("read-api", "Read-through API + react-query hooks", "done"),
        (
            "ui-catalog",
            "Use catalog in graph-node icon + chunk-type-icon",
            "done",
        ),
        (
            "fk-cutover",
            "FK cutover on chunk.type and chunk_connection.relation",
            "done",
        ),
        ("crud-ui", "CRUD UI at /settings/vocabulary", "done"),
        (
            "inverse-ui",
            "Inverse relation labels on chunk detail",
            "in_progress",
        ),
    ];
    let mut task_ids = BTreeMap::new();
    for (order, (name, title, status)) in tasks.iter().enumerate() {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM plan_task WHERE plan_id = $1 AND title = $2")
                .bind(&plan_id)
                .bind(title)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-plan-task-{name}"));
        sqlx::query(
            "INSERT INTO plan_task (id, plan_id, title, status, \"order\", metadata) \
             VALUES ($1, $2, $3, $4, $5, '{}'::jsonb) ON CONFLICT (id) DO UPDATE SET \
             title = EXCLUDED.title, status = EXCLUDED.status, \"order\" = EXCLUDED.\"order\", updated_at = now()",
        )
        .bind(&id)
        .bind(&plan_id)
        .bind(title)
        .bind(status)
        .bind(order as i32)
        .execute(&mut **tx)
        .await?;
        task_ids.insert((*name).to_string(), id);
    }
    let analyze = [
        (
            "risk",
            "FK cutover could fail if any existing chunk.type doesn't map to a catalog row.",
            serde_json::json!({"severity": "medium"}),
            None,
        ),
        (
            "assumption",
            "Users will edit vocabulary rarely — a per-codebase JSON dump is plenty for now.",
            serde_json::json!({"verified": false}),
            None,
        ),
        (
            "chunk",
            "Existing catalog-pattern convention chunk",
            serde_json::json!({}),
            chunk_ids.get("catalog-pattern"),
        ),
    ];
    for (order, (kind, body, metadata, chunk_id)) in analyze.iter().enumerate() {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM plan_analyze_item WHERE plan_id = $1 AND \"order\" = $2",
        )
        .bind(&plan_id)
        .bind(order as i32)
        .fetch_optional(&mut **tx)
        .await?;
        let id = existing.unwrap_or_else(|| format!("seed-plan-analyze-{order}"));
        sqlx::query(
            "INSERT INTO plan_analyze_item (id, plan_id, kind, \"order\", chunk_id, text, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO UPDATE SET \
             kind = EXCLUDED.kind, chunk_id = EXCLUDED.chunk_id, text = EXCLUDED.text, \
             metadata = EXCLUDED.metadata, updated_at = now()",
        )
        .bind(&id)
        .bind(&plan_id)
        .bind(kind)
        .bind(order as i32)
        .bind(chunk_id)
        .bind(body)
        .bind(metadata)
        .execute(&mut **tx)
        .await?;
    }
    Ok(PlanSeed { plan_id, task_ids })
}
