//! `plan_task` (+ its own children `plan_task_chunk`, `plan_task_dependency`,
//! `plan_task_external_link`) query surface, split out of `super` once the
//! combined `plan` repo file passed the ~900-line split threshold.
//! Re-exported at `crate::repo::plan::…` by the parent module so no
//! caller's import path changes.
//!
//! **Divergence #12 — the port's first `sqlx::Transaction`.** Node issues
//! `updateTask` and `unblockDependentsOf` as two independent effects
//! (`packages/api/src/plans/tasks.ts:113-122`), and `unblockDependentsOf`
//! itself is an unwrapped `SELECT` then `UPDATE`
//! (`packages/db/src/repository/plan.ts:536-551`). A crash between them
//! leaves a task `done` with its dependents still `blocked` forever — no
//! retry recovers that state, because nothing re-triggers the unblock once
//! the "just transitioned to done" moment has passed. [`mark_task_done_and_unblock`]
//! does the whole "set `status='done'`, then flip any dependents that are
//! **exactly** `'blocked'` to `'pending'`" sequence inside one transaction.
//! Atomicity was originally checked by hand — a temporary `SELECT 1/0`
//! spliced between the two statements, run once under a debugger, then
//! deleted — **not** by an automated test; no such regression test existed
//! until `tests/plan.rs::injecting_a_fault_between_mark_done_and_unblock_rolls_back_the_done_flag`
//! was added. That test drives [`mark_done_step`] and [`unblock_step`]
//! directly inside its own transaction and injects a real fault between
//! them, with no fault-injection branch in the production code path.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// Raw `plan_task` row — `acceptance_criteria` stays the untouched stored
/// JSON; the service layer normalises it to `{text,done}[]` on the way out
/// for read paths, but every mutating endpoint in this file (`POST`/`PATCH`)
/// returns this raw row unmodified, matching the task brief's "raw (NOT
/// normalised)" instruction.
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

/// Ordered `"order" ASC, id ASC`. **B1 fix**: this previously ordered by
/// `"order" ASC` alone, with no tiebreaker — live-reproduced with 20 tasks
/// all at `"order" = 0` plus `ANALYZE`: ordering came back non-deterministic
/// across runs, the same divergence-#6 bug class already fixed for
/// `plan::list`, `analyze::list_analyze_items`, and `requirement::
/// list_requirements`. `reorder_tasks` (this file) makes `"order"` ties
/// routine, not theoretical — the exact sequence that made this bite for
/// analyze items and requirements. Proven load-bearing in `tests/plan.rs::
/// list_tasks_breaks_order_ties_by_id`: with `, id ASC` removed, 20 tasks
/// sharing one forced-identical `"order"` no longer come back in
/// ascending-id order.
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
           ORDER BY "order" ASC, id ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Looks up a single task by id, scoped to `(plan_id, user_id)` — the
/// single-row counterpart to `list_tasks`, used both by `update_task`'s
/// no-op reselect branch and by the service layer to refetch a task's
/// current row after `mark_task_done_and_unblock` (which itself only
/// returns the unblocked dependent ids, not the task). Proven load-bearing
/// in `tests/plan.rs::find_task_by_id_is_user_scoped`.
pub async fn find_task_by_id(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Option<PlanTask>> {
    let row = sqlx::query_as!(
        PlanTask,
        r#"SELECT id, plan_id, title, description,
                  acceptance_criteria AS "acceptance_criteria: Json<serde_json::Value>",
                  status, "order",
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  metadata AS "metadata: Json<serde_json::Value>"
           FROM plan_task
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)"#,
        task_id,
        plan_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Creates a task under a plan the caller owns, appending at
/// `maxOrder + 1` with `status` hardcoded to `'pending'` regardless of any
/// caller input — matching Node's `createTask` (`plan.ts:401-412`) and the
/// `POST /plans/:id/tasks` handler, which always passes `status: "pending"`
/// (`tasks.ts:60`) and never accepts a caller-supplied status. `metadata`
/// defaults to `{}` when omitted, matching the column's own `NOT NULL
/// DEFAULT '{}'`. `None` means the plan isn't the caller's. Proven
/// load-bearing in `tests/plan.rs::cannot_create_a_task_for_another_users_plan`.
pub async fn create_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    title: &str,
    description: Option<&str>,
    acceptance_criteria: serde_json::Value,
    metadata: Option<serde_json::Value>,
) -> AppResult<Option<PlanTask>> {
    let id = crate::new_id();
    let metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    let row = sqlx::query_as!(
        PlanTask,
        r#"INSERT INTO plan_task (id, plan_id, title, description, acceptance_criteria, status, "order", metadata)
           SELECT $1, p.id, $3, $4, $5, 'pending',
                  COALESCE((SELECT MAX(pt."order") FROM plan_task pt WHERE pt.plan_id = p.id), -1) + 1,
                  $7
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
        user_id,
        metadata
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// General `PATCH /plans/:id/tasks/:taskId` update — `title`, tri-state
/// `description`, `acceptance_criteria`, `metadata`, and `status` all in one
/// `UPDATE`, matching Node's single `db.update(planTask).set({...patch,
/// updatedAt})` (`plan.ts:414-424`). `status` is plain `COALESCE` here (not
/// tri-state — Node's body schema has no `t.Null()` union for it); the
/// service layer never routes an incoming `status: "done"` through this
/// `status` parameter — see `service::update_task`'s doc comment for why
/// the "done" transition is funneled through `mark_task_done_and_unblock`
/// instead, keeping the divergence-#12 fix as the sole place `status`
/// becomes `'done'`.
///
/// Falls back to a plain guarded re-select when every field is `None`/
/// unset, same "don't bump `updated_at` on a no-op patch" convention as
/// `plan::update`/`analyze::update_analyze_item`. `AND EXISTS (... p.user_id
/// = $3)` on both branches is divergence #13 again. Proven load-bearing in
/// `tests/plan.rs::cannot_update_another_users_task`.
#[allow(clippy::too_many_arguments)]
pub async fn update_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    title: Option<&str>,
    description: Option<Option<&str>>,
    acceptance_criteria: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    status: Option<&str>,
) -> AppResult<Option<PlanTask>> {
    let (desc_set, desc_val) = match description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let has_changes = title.is_some()
        || desc_set
        || acceptance_criteria.is_some()
        || metadata.is_some()
        || status.is_some();

    let row = if has_changes {
        sqlx::query_as!(
            PlanTask,
            r#"UPDATE plan_task SET
                 title = COALESCE($4, title),
                 description = CASE WHEN $5::bool THEN $6 ELSE description END,
                 acceptance_criteria = COALESCE($7, acceptance_criteria),
                 metadata = COALESCE($8, metadata),
                 status = COALESCE($9, status),
                 updated_at = now()
               WHERE id = $1 AND plan_id = $2
                 AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)
               RETURNING id, plan_id, title, description,
                         acceptance_criteria AS "acceptance_criteria: Json<serde_json::Value>",
                         status, "order",
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp",
                         metadata AS "metadata: Json<serde_json::Value>""#,
            task_id,
            plan_id,
            user_id,
            title,
            desc_set,
            desc_val,
            acceptance_criteria,
            metadata,
            status
        )
        .fetch_optional(pool)
        .await?
    } else {
        find_task_by_id(pool, user_id, plan_id, task_id).await?
    };
    Ok(row)
}

/// Deletes a task, scoped to `(id, plan_id)` plus the parent-ownership
/// guard. Child rows in `plan_task_chunk`, `plan_task_dependency`, and
/// `plan_task_external_link` are removed by the database's own `ON DELETE
/// CASCADE` foreign keys, matching Node's `deleteTask` (`plan.ts:426-430`),
/// a bare delete with no cascade logic of its own. Proven load-bearing in
/// `tests/plan.rs::cannot_delete_another_users_task`.
pub async fn delete_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_task
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)"#,
        task_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Applies a partial reorder inside one transaction, matching Node's
/// `reorderTasks` (`plan.ts:432-445`): one `UPDATE ... SET "order" = i
/// WHERE id = ? AND planId = ?` per entry. Rows the request doesn't mention
/// keep whatever `"order"` they already had. Each `UPDATE` carries the same
/// parent-ownership guard as every other write in this file. Proven
/// load-bearing in `tests/plan.rs::reorder_tasks_leaves_unmentioned_rows_untouched`
/// and `tests/plan.rs::cannot_reorder_another_users_tasks`.
pub async fn reorder_tasks(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (i, task_id) in task_ids.iter().enumerate() {
        let order = i as i32;
        sqlx::query!(
            r#"UPDATE plan_task SET "order" = $3
               WHERE id = $1 AND plan_id = $2
                 AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $4)"#,
            task_id,
            plan_id,
            order,
            user_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// A `plan_task_chunk` row joined with the linked chunk's title/type, for
/// the detail page's task chunk chips. Matches Node's
/// `listTaskChunksWithTitles` (`packages/db/src/repository/plan.ts:449-481`).
///
/// **B2 fix**: this previously took only `task_id`, with no ownership guard
/// at all (`WHERE ptc.task_id = $1`) — not exploitable at the time because
/// its only caller sourced ids from an already-scoped `list_tasks` result,
/// but the function was `pub`, so any future caller could read another
/// user's task-chunk links (id, chunk_id, relation, chunk title, chunk
/// type). This task adds task-chunk endpoints that call exactly this
/// function with a caller-supplied `task_id`, so the guard is no longer
/// optional. Ownership derives through **two** levels — `plan_task_chunk ->
/// plan_task -> plan` — matching divergence #13's usual scoping-in-SQL
/// rule (the same pattern Phase 1's review found missing in `chunk_meta`).
/// No `ORDER BY` — matches Node exactly, which has none either; not added
/// here per the task brief's explicit instruction to leave that alone.
/// Proven load-bearing in `tests/plan.rs::list_task_chunks_with_titles_is_user_scoped`.
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
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Vec<PlanTaskChunkWithTitle>> {
    let rows = sqlx::query_as!(
        PlanTaskChunkWithTitle,
        r#"SELECT ptc.id, ptc.task_id, ptc.chunk_id, ptc.relation,
                  ptc.created_at AS "created_at: UtcTimestamp",
                  c.title AS "chunk_title?", c.type AS "chunk_type?"
           FROM plan_task_chunk ptc
           LEFT JOIN chunk c ON c.id = ptc.chunk_id
           WHERE ptc.task_id = $1
             AND EXISTS (SELECT 1 FROM plan_task pt WHERE pt.id = $1 AND pt.plan_id = $2
                         AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3))"#,
        task_id,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Bare `plan_task_chunk` row — the `POST /plans/:id/tasks/:taskId/chunks`
/// response shape, distinct from [`PlanTaskChunkWithTitle`] (which is only
/// used by the plan-detail envelope).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskChunk {
    pub id: String,
    pub task_id: String,
    pub chunk_id: String,
    pub relation: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Links a chunk to a task the caller owns — ownership derives through
/// `plan_task -> plan`, divergence #13's two-level scoping (SCOPING
/// section). `relation` is validated by the service layer only (`context |
/// created | modified`), never a DB constraint or Rust enum, matching
/// Node's `addTaskChunk` (`plan.ts:483-493`) and its `t.String()` body
/// schema. `None` means the task isn't under a plan the caller owns. Proven
/// load-bearing in `tests/plan.rs::cannot_add_a_chunk_to_another_users_task`.
pub async fn add_task_chunk(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    chunk_id: &str,
    relation: &str,
) -> AppResult<Option<PlanTaskChunk>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        PlanTaskChunk,
        r#"INSERT INTO plan_task_chunk (id, task_id, chunk_id, relation)
           SELECT $1, pt.id, $4, $5
           FROM plan_task pt
           WHERE pt.id = $2 AND pt.plan_id = $3
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $3 AND p.user_id = $6)
           RETURNING id, task_id, chunk_id, relation, created_at AS "created_at: UtcTimestamp""#,
        id,
        task_id,
        plan_id,
        chunk_id,
        relation,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Removes a task-chunk link, scoped to `(id, task_id)` plus the two-level
/// parent-ownership guard. `false` covers "link never existed", "link
/// belongs to a different task", and "task's plan isn't the caller's"
/// alike. Proven load-bearing in
/// `tests/plan.rs::cannot_remove_another_users_task_chunk_link`.
pub async fn remove_task_chunk(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    link_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_task_chunk
           WHERE id = $1 AND task_id = $2
             AND EXISTS (SELECT 1 FROM plan_task pt WHERE pt.id = $2 AND pt.plan_id = $3
                         AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $3 AND p.user_id = $4))"#,
        link_id,
        task_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
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

/// Records that `task_id` depends on `depends_on_task_id`, scoped through
/// `plan_task -> plan` the same two-level way as `add_task_chunk`. **No
/// self-reference guard** (`task_id = depends_on_task_id` is unenforced) —
/// deliberately not added, matching Node's `addTaskDependency`
/// (`plan.ts:518-524`), which has none either (the task brief calls this
/// out explicitly as a gap that must NOT be "fixed" here, since doing so
/// would be an unrequested divergence). `None` means the task isn't under a
/// plan the caller owns. Proven load-bearing in
/// `tests/plan.rs::cannot_add_a_dependency_to_another_users_task`.
pub async fn add_task_dependency(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    depends_on_task_id: &str,
) -> AppResult<Option<PlanTaskDependency>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        PlanTaskDependency,
        r#"INSERT INTO plan_task_dependency (id, task_id, depends_on_task_id)
           SELECT $1, pt.id, $4
           FROM plan_task pt
           WHERE pt.id = $2 AND pt.plan_id = $3
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $3 AND p.user_id = $5)
           RETURNING id, task_id, depends_on_task_id, created_at AS "created_at: UtcTimestamp""#,
        id,
        task_id,
        plan_id,
        depends_on_task_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Removes a task dependency, scoped to `(id, task_id)` plus the two-level
/// parent-ownership guard. Proven load-bearing in
/// `tests/plan.rs::cannot_remove_another_users_task_dependency`.
pub async fn remove_task_dependency(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    dep_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_task_dependency
           WHERE id = $1 AND task_id = $2
             AND EXISTS (SELECT 1 FROM plan_task pt WHERE pt.id = $2 AND pt.plan_id = $3
                         AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $3 AND p.user_id = $4))"#,
        dep_id,
        task_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// The reason this whole file exists as a reviewable unit — divergence #12.
/// Sets `status = 'done'` on the caller's own task, then, in the **same**
/// transaction, flips any task that depends on it (`plan_task_dependency.
/// depends_on_task_id = task_id`) from `'blocked'` to `'pending'`, leaving
/// dependents in `pending`/`in_progress`/`done`/`skipped` untouched.
/// Matches the combined semantics of Node's `updateTask(taskId, {status:
/// "done"})` + `unblockDependentsOf(taskId)` (`tasks.ts:113-122`,
/// `plan.ts:536-551`), except atomically: a failure between the two
/// `UPDATE`s here rolls back the whole transaction, so a task can never end
/// up `done` with a dependent stuck `blocked` — the exact failure mode
/// Node has no protection against.
///
/// Returns the ids of tasks actually flipped `pending`. Returns an empty
/// vec both when the task isn't the caller's own (the first `UPDATE`'s
/// `RETURNING` guard matches no row, so the transaction rolls back without
/// touching anything) and when the task legitimately has no `blocked`
/// dependents — callers that need to distinguish "not found" from "nothing
/// to unblock" must already know the task exists (e.g. via a preceding
/// `update_task`/`find_task_by_id` call), same shape the service layer
/// uses.
///
/// Atomicity is exercised by
/// `tests/plan.rs::injecting_a_fault_between_mark_done_and_unblock_rolls_back_the_done_flag`,
/// which drives [`mark_done_step`] and [`unblock_step`] — the same two
/// functions this wrapper calls — inside its own transaction, injects a
/// real Postgres error between them (`SELECT 1/0`, executed by the test,
/// not by any production code path), and confirms the task is still not
/// `done` after rolling back. Cross-user guard proven in
/// `tests/plan.rs::cannot_mark_another_users_task_done`.
pub async fn mark_task_done_and_unblock(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Vec<String>> {
    let mut tx = pool.begin().await?;

    let updated = mark_done_step(&mut tx, user_id, plan_id, task_id).await?;
    if updated.is_none() {
        tx.rollback().await?;
        return Ok(vec![]);
    }

    let unblocked = unblock_step(&mut tx, plan_id, task_id).await?;

    tx.commit().await?;
    Ok(unblocked)
}

/// First half of [`mark_task_done_and_unblock`]'s transaction: flips the
/// caller's own task to `'done'`. Extracted to its own `&mut Transaction`
/// function (rather than inlined) so a test can drive it and
/// [`unblock_step`] inside a transaction it controls itself, inject a fault
/// between the two calls, and assert the rollback — without any
/// fault-injection code living in the production path. Returns `None` when
/// the `id`/`plan_id`/`user_id` guard matches no row (wrong task, wrong
/// plan, or a plan not owned by `user_id`), same as the row-count check the
/// former inlined version used.
pub async fn mark_done_step(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Option<String>> {
    let updated = sqlx::query_scalar!(
        r#"UPDATE plan_task SET status = 'done', updated_at = now()
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)
           RETURNING id"#,
        task_id,
        plan_id,
        user_id
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(updated)
}

/// Second half of [`mark_task_done_and_unblock`]'s transaction: flips every
/// dependent that is **exactly** `'blocked'` to `'pending'`. See
/// [`mark_done_step`] for why this is a separate `&mut Transaction`
/// function rather than inlined.
pub async fn unblock_step(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Vec<String>> {
    let unblocked = sqlx::query_scalar!(
        r#"UPDATE plan_task SET status = 'pending', updated_at = now()
           WHERE plan_id = $2 AND status = 'blocked'
             AND id IN (SELECT task_id FROM plan_task_dependency WHERE depends_on_task_id = $1)
           RETURNING id"#,
        task_id,
        plan_id
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(unblocked)
}

/// `camelCase` wire shape of a `plan_task_external_link` row — the task-level
/// counterpart to `link::PlanExternalLink`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskExternalLink {
    pub id: String,
    pub task_id: String,
    pub system: String,
    pub url: String,
    pub label: Option<String>,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Ordered `"order" ASC, created_at ASC`, mirroring `link::list_links`'
/// tiebreaker choice for the plan-level table — not `, id ASC`, since this
/// table's own precedent already uses `created_at` as the app-level
/// tiebreaker rather than id. Ownership derives through `plan_task -> plan`.
pub async fn list_task_links(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Vec<PlanTaskExternalLink>> {
    let rows = sqlx::query_as!(
        PlanTaskExternalLink,
        r#"SELECT id, task_id, system, url, label, "order",
                  created_at AS "created_at: UtcTimestamp"
           FROM plan_task_external_link
           WHERE task_id = $1
             AND EXISTS (SELECT 1 FROM plan_task pt WHERE pt.id = $1 AND pt.plan_id = $2
                         AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3))
           ORDER BY "order" ASC, created_at ASC"#,
        task_id,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Inserts a link under a task the caller owns — same ownership-guarded-
/// insert shape as `link::add_link`, scoped through `plan_task -> plan`.
/// `None` means the task isn't under a plan the caller owns. Proven
/// load-bearing in `tests/plan.rs::cannot_add_a_link_to_another_users_task`.
pub async fn add_task_link(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    system: &str,
    url: &str,
    label: Option<&str>,
) -> AppResult<Option<PlanTaskExternalLink>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        PlanTaskExternalLink,
        r#"INSERT INTO plan_task_external_link (id, task_id, system, url, label)
           SELECT $1, pt.id, $4, $5, $6
           FROM plan_task pt
           WHERE pt.id = $2 AND pt.plan_id = $3
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $3 AND p.user_id = $7)
           RETURNING id, task_id, system, url, label, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        task_id,
        plan_id,
        system,
        url,
        label,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Removes a task link, scoped to `(id, task_id)` plus the two-level
/// parent-ownership guard. Proven load-bearing in
/// `tests/plan.rs::cannot_remove_another_users_task_link`.
pub async fn remove_task_link(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    link_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_task_external_link
           WHERE id = $1 AND task_id = $2
             AND EXISTS (SELECT 1 FROM plan_task pt WHERE pt.id = $2 AND pt.plan_id = $3
                         AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $3 AND p.user_id = $4))"#,
        link_id,
        task_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
