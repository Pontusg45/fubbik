//! `plan_analyze_item` query surface, split out of `super` once the
//! combined `plan` repo file passed the ~900-line split threshold.
//! Re-exported at `crate::repo::plan::…` by the parent module so no
//! caller's import path changes.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// `camelCase` wire shape of a `plan_analyze_item` row. `kind` stays plain
/// `String` for the same reason `plan.status` does — no DB `CHECK`,
/// validated only by Node's service layer.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanAnalyzeItem {
    pub id: String,
    pub plan_id: String,
    pub kind: String,
    pub order: i32,
    pub chunk_id: Option<String>,
    pub file_path: Option<String>,
    pub text: Option<String>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: Json<serde_json::Value>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Ordered `kind, "order"` — the API layer groups these into the five fixed
/// `{chunk,file,risk,assumption,question}` buckets, matching Node's
/// `groupByKind` (`packages/api/src/plans/analyze.ts:14-28`).
pub async fn list_analyze_items(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<PlanAnalyzeItem>> {
    let rows = sqlx::query_as!(
        PlanAnalyzeItem,
        r#"SELECT id, plan_id, kind, "order", chunk_id, file_path, text,
                  metadata AS "metadata: Json<serde_json::Value>",
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM plan_analyze_item
           WHERE plan_id = $1
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $2)
           ORDER BY kind ASC, "order" ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
