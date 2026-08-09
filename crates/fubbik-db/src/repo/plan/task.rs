//! `plan_task` (+ its own children `plan_task_chunk`, `plan_task_dependency`)
//! query surface, split out of `super` once the combined `plan` repo file
//! passed the ~900-line split threshold. Re-exported at `crate::repo::
//! plan::…` by the parent module so no caller's import path changes.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// Raw `plan_task` row — `acceptance_criteria` stays the untouched stored
/// JSON; the service layer normalises it to `{text,done}[]` on the way out.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanTask {
    pub id: String,
    pub plan_id: String,
    pub title: String,
    pub description: Option<String>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub acceptance_criteria: Json<serde_json::Value>,
    pub status: String,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: Json<serde_json::Value>,
}

pub async fn list_tasks(pool: &PgPool, user_id: &str, plan_id: &str) -> AppResult<Vec<PlanTask>> {
    let rows = sqlx::query_as!(
        PlanTask,
        r#"SELECT id, plan_id, title, description,
                  acceptance_criteria AS "acceptance_criteria: Json<serde_json::Value>",
                  status, "order",
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  metadata AS "metadata: Json<serde_json::Value>"
           FROM plan_task
           WHERE plan_id = $1
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $2)
           ORDER BY "order" ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Creates a task under a plan the caller owns, appending at
/// `maxOrder + 1` with `status` hardcoded to `'pending'` regardless of any
/// caller input — matching Node's `createTask` (`plan.ts:401-412`) called
/// from `createPlan`, which always passes `status: "pending"` for a plan's
/// initial tasks. `None` means the plan isn't the caller's.
pub async fn create_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    title: &str,
    description: Option<&str>,
    acceptance_criteria: serde_json::Value,
) -> AppResult<Option<PlanTask>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        PlanTask,
        r#"INSERT INTO plan_task (id, plan_id, title, description, acceptance_criteria, status, "order")
           SELECT $1, p.id, $3, $4, $5, 'pending',
                  COALESCE((SELECT MAX(pt."order") FROM plan_task pt WHERE pt.plan_id = p.id), -1) + 1
           FROM plan p
           WHERE p.id = $2 AND p.user_id = $6
           RETURNING id, plan_id, title, description,
                     acceptance_criteria AS "acceptance_criteria: Json<serde_json::Value>",
                     status, "order",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     metadata AS "metadata: Json<serde_json::Value>""#,
        id,
        plan_id,
        title,
        description,
        acceptance_criteria,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// A `plan_task_chunk` row joined with the linked chunk's title/type, for
/// the detail page's task chunk chips. Matches Node's
/// `listTaskChunksWithTitles` (`packages/db/src/repository/plan.ts:449-481`).
/// No ownership guard here — `task_id` only ever reaches this function
/// already scoped, from a `list_tasks` result.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskChunkWithTitle {
    pub id: String,
    pub task_id: String,
    pub chunk_id: String,
    pub relation: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    pub chunk_title: Option<String>,
    pub chunk_type: Option<String>,
}

pub async fn list_task_chunks_with_titles(
    pool: &PgPool,
    task_id: &str,
) -> AppResult<Vec<PlanTaskChunkWithTitle>> {
    let rows = sqlx::query_as!(
        PlanTaskChunkWithTitle,
        r#"SELECT ptc.id, ptc.task_id, ptc.chunk_id, ptc.relation,
                  ptc.created_at AS "created_at: UtcTimestamp",
                  c.title AS "chunk_title?", c.type AS "chunk_type?"
           FROM plan_task_chunk ptc
           LEFT JOIN chunk c ON c.id = ptc.chunk_id
           WHERE ptc.task_id = $1"#,
        task_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `camelCase` wire shape of a `plan_task_dependency` row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskDependency {
    pub id: String,
    pub task_id: String,
    pub depends_on_task_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// All task-dependency rows for a plan's tasks — joins through `plan_task`
/// since `plan_task_dependency` carries no `plan_id` column, matching
/// Node's `listTaskDependencies` (`plan.ts:503-516`). Node has no
/// `ORDER BY` here; this adds `created_at, id` as a tiebreaker for this
/// port's usual total-ordering reason — the fixture's `dependencies` array
/// is empty in the only captured plan, so this can't diverge from anything
/// actually observed.
pub async fn list_task_dependencies(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<PlanTaskDependency>> {
    let rows = sqlx::query_as!(
        PlanTaskDependency,
        r#"SELECT ptd.id, ptd.task_id, ptd.depends_on_task_id,
                  ptd.created_at AS "created_at: UtcTimestamp"
           FROM plan_task_dependency ptd
           JOIN plan_task pt ON pt.id = ptd.task_id
           WHERE pt.plan_id = $1
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $2)
           ORDER BY ptd.created_at ASC, ptd.id ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
