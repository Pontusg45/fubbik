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

/// Ordered `"order" ASC, id ASC`. Node's `listPlanRequirements` orders only
/// by `asc(planRequirement.order)` (`plan.ts:296`) with no tiebreaker;
/// `"order"` defaults to `0` and is (re)set to plain array indices by
/// `reorder_requirements`, so ties are routine, not theoretical — the
/// trailing `id ASC` is this port's usual divergence #6 total-ordering fix.
/// Proven load-bearing in `tests/plan.rs::
/// list_requirements_breaks_order_ties_by_id`: with `, id ASC` removed, 20
/// links sharing one forced-identical `"order"` no longer come back in
/// ascending-id order.
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
           ORDER BY "order" ASC, id ASC"#,
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
pub async fn add_requirement<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
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
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Unlinks a requirement from a plan the caller owns. `false` covers "link
/// never existed", "requirement belongs to a different plan", and "plan
/// isn't the caller's" alike — same shape as `plan::remove_link`.
///
/// Node's `removePlanRequirement` (`plan.ts:313-317`) is a bare `void`
/// delete with no rowcount signal at all — the route always returns
/// `{ok:true}`. This port's plans domain already breaks from that for
/// `remove_link` (proven in `tests/plans.rs::
/// remove_link_returns_ok_true_and_404_on_missing`, a deliberate,
/// domain-wide choice to surface a real 404 instead of Node's silent
/// no-op), so `remove_requirement` follows the same convention rather than
/// being the one child-delete in this file that stays silent.
pub async fn remove_requirement(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    requirement_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_requirement
           WHERE plan_id = $1 AND requirement_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $3)"#,
        plan_id,
        requirement_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Applies a partial reorder inside one transaction, matching Node's
/// `reorderPlanRequirements` (`plan.ts:319-332`): one `UPDATE ... SET
/// "order" = i WHERE planId = ? AND requirementId = ?` per entry. Rows the
/// request doesn't mention keep whatever `"order"` they already had — they
/// are not renumbered and not deleted. Each `UPDATE` carries the same
/// parent-ownership guard as every other write in this file; an entry
/// naming a requirement under a plan the caller doesn't own simply updates
/// zero rows for that entry, same as an entry naming a requirement id that
/// was never linked at all.
pub async fn reorder_requirements(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    requirement_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (i, requirement_id) in requirement_ids.iter().enumerate() {
        let order = i as i32;
        sqlx::query!(
            r#"UPDATE plan_requirement SET "order" = $3
               WHERE plan_id = $1 AND requirement_id = $2
                 AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $4)"#,
            plan_id,
            requirement_id,
            order,
            user_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
