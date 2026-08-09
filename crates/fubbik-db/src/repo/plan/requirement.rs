//! `plan_requirement` query surface, split out of `super` once the
//! combined `plan` repo file passed the ~900-line split threshold.
//! Re-exported at `crate::repo::plan::…` by the parent module so no
//! caller's import path changes.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` wire shape of a `plan_requirement` row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequirement {
    pub id: String,
    pub plan_id: String,
    pub requirement_id: String,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

pub async fn list_requirements(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<PlanRequirement>> {
    let rows = sqlx::query_as!(
        PlanRequirement,
        r#"SELECT id, plan_id, requirement_id, "order", created_at AS "created_at: UtcTimestamp"
           FROM plan_requirement
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

/// Links an existing requirement to a plan the caller owns, appending at
/// `maxOrder + 1`, matching Node's `addPlanRequirement` (`plan.ts:300-311`).
/// `None` means the plan isn't the caller's.
pub async fn add_requirement(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    requirement_id: &str,
) -> AppResult<Option<PlanRequirement>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        PlanRequirement,
        r#"INSERT INTO plan_requirement (id, plan_id, requirement_id, "order")
           SELECT $1, p.id, $3,
                  COALESCE((SELECT MAX(pr."order") FROM plan_requirement pr WHERE pr.plan_id = p.id), -1) + 1
           FROM plan p
           WHERE p.id = $2 AND p.user_id = $4
           RETURNING id, plan_id, requirement_id, "order", created_at AS "created_at: UtcTimestamp""#,
        id,
        plan_id,
        requirement_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
