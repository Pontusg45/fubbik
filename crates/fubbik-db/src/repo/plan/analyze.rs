//! `plan_analyze_item` query surface, split out of `super` once the
//! combined `plan` repo file passed the ~900-line split threshold.
//! Re-exported at `crate::repo::plan::…` by the parent module so no
//! caller's import path changes.

use fubbik_core::error::{AppError, AppResult};
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

/// Ordered `kind ASC, "order" ASC, id ASC` — the API layer groups these into
/// the five fixed `{chunk,file,risk,assumption,question}` buckets, matching
/// Node's `groupByKind` (`packages/api/src/plans/analyze.ts:14-28`). Node's
/// own `orderBy(asc(planAnalyzeItem.kind), asc(planAnalyzeItem.order))`
/// (`plan.ts:342`) has no further tiebreaker; `"order"` defaults to `0` and
/// is (re)set to plain array indices by `reorder_analyze_items`, so ties are
/// routine, not theoretical — the trailing `id ASC` is this port's usual
/// divergence #6 total-ordering fix. Proven load-bearing in `tests/plan.rs::
/// list_analyze_items_breaks_order_ties_by_id`: with `, id ASC` removed, 20
/// items sharing one forced-identical `"order"` no longer come back in
/// ascending-id order.
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
           ORDER BY kind ASC, "order" ASC, id ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Creates an analyze item under a plan the caller owns, appending at
/// `maxOrder + 1` **scoped to `(planId, kind)`** — not just `planId` — so
/// each of the five kind buckets orders independently, matching Node's
/// `createAnalyzeItem` (`plan.ts:346-360`). `metadata` defaults to `{}`
/// when omitted, matching the column's own `NOT NULL DEFAULT '{}'`.
///
/// Unlike `add_requirement`/`add_link` (which return `Option<T>` and let
/// the service layer 404), this returns a bare `AppResult<PlanAnalyzeItem>`
/// and raises [`AppError::NotFound`] itself when the `INSERT ... SELECT
/// ... FROM plan p WHERE p.id = $2 AND p.user_id = $8` guard matches no
/// row — same "raise `NotFound` from inside the repo" shape as
/// `tag::merge`. Proven load-bearing in `tests/plan.rs::
/// cannot_create_an_analyze_item_for_another_users_plan`: with the
/// `AND p.user_id = $8` predicate removed, Bob's attempt to create an item
/// under Alice's plan id stops erroring and actually creates one.
#[allow(clippy::too_many_arguments)]
pub async fn create_analyze_item(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    kind: &str,
    chunk_id: Option<&str>,
    file_path: Option<&str>,
    text: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> AppResult<PlanAnalyzeItem> {
    let id = crate::new_id();
    let metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    let row = sqlx::query_as!(
        PlanAnalyzeItem,
        r#"INSERT INTO plan_analyze_item (id, plan_id, kind, "order", chunk_id, file_path, text, metadata)
           SELECT $1, p.id, $3,
                  COALESCE((SELECT MAX(pai."order") FROM plan_analyze_item pai
                            WHERE pai.plan_id = p.id AND pai.kind = $3), -1) + 1,
                  $4, $5, $6, $7
           FROM plan p
           WHERE p.id = $2 AND p.user_id = $8
           RETURNING id, plan_id, kind, "order", chunk_id, file_path, text,
                     metadata AS "metadata: Json<serde_json::Value>",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        plan_id,
        kind,
        chunk_id,
        file_path,
        text,
        metadata,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    row.ok_or_else(|| AppError::NotFound("Plan".to_string()))
}

/// Partial update of `text`/`metadata`/`chunk_id`/`file_path` — `kind` is
/// deliberately not settable here, matching Node's `updateAnalyzeItem`
/// (`plan.ts:362-372`), whose route body schema
/// (`packages/api/src/plans/analyze.ts:79-91`) has no `kind` field at all.
/// Plain `COALESCE` per field (not tri-state): Node's body schema declares
/// every field `t.Optional(t.String())`/`t.Optional(t.Record(...))` with no
/// `t.Null()` union, so there is no client-facing way to *clear* any of
/// these columns, only to leave them or overwrite them.
///
/// Scoped by `id = $1 AND plan_id = $2 AND EXISTS (... p.user_id = $3)` —
/// Node's own `updateAnalyzeItem` scopes by `itemId` alone (no `planId`,
/// no ownership check whatsoever) and `throw`s an untagged `Error` (->
/// 500) when the id doesn't match any row. Returning `None` here instead
/// (mapped to a clean 404 by the service layer) is the same
/// "don't reproduce Node's 500-on-guard-failure" choice already made for
/// `plan::duplicate`.
#[allow(clippy::too_many_arguments)]
pub async fn update_analyze_item(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    item_id: &str,
    text: Option<&str>,
    metadata: Option<serde_json::Value>,
    chunk_id: Option<&str>,
    file_path: Option<&str>,
) -> AppResult<Option<PlanAnalyzeItem>> {
    let row = sqlx::query_as!(
        PlanAnalyzeItem,
        r#"UPDATE plan_analyze_item SET
             text = COALESCE($4, text),
             metadata = COALESCE($5, metadata),
             chunk_id = COALESCE($6, chunk_id),
             file_path = COALESCE($7, file_path),
             updated_at = now()
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)
           RETURNING id, plan_id, kind, "order", chunk_id, file_path, text,
                     metadata AS "metadata: Json<serde_json::Value>",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        item_id,
        plan_id,
        user_id,
        text,
        metadata,
        chunk_id,
        file_path
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Deletes an analyze item, scoped to `(id, plan_id)` plus the same
/// parent-ownership guard as everything else in this file. `false` covers
/// "item never existed", "item belongs to a different plan", and "plan
/// isn't the caller's" alike — same domain-wide convention as
/// `requirement::remove_requirement` / `link::remove_link`, diverging from
/// Node's bare-`void` `deleteAnalyzeItem` (`plan.ts:374-378`) on purpose.
pub async fn delete_analyze_item(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    item_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_analyze_item
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)"#,
        item_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Applies a partial reorder inside one transaction, matching Node's
/// `reorderAnalyzeItems` (`plan.ts:380-393`): one `UPDATE ... SET "order" =
/// i WHERE id = ? AND planId = ? AND kind = ?` per entry. Rows the request
/// doesn't mention — including items of a *different* `kind` under the
/// same plan — keep whatever `"order"` they already had. Proven
/// load-bearing in `tests/plan.rs::reorder_leaves_unmentioned_rows_untouched`
/// (verbatim from the task brief).
pub async fn reorder_analyze_items(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    kind: &str,
    item_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (i, item_id) in item_ids.iter().enumerate() {
        let order = i as i32;
        sqlx::query!(
            r#"UPDATE plan_analyze_item SET "order" = $4
               WHERE id = $1 AND plan_id = $2 AND kind = $3
                 AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $5)"#,
            item_id,
            plan_id,
            kind,
            order,
            user_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
