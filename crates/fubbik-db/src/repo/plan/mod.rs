//! `plan` repo layer — the port's largest ownership divergence (#13).
//!
//! Node's `getPlan(id)` (`packages/db/src/repository/plan.ts:145-150`)
//! selects by `id` alone, with no `user_id` predicate anywhere in the
//! `WHERE` clause. The service wrapper adds a 404-if-missing check but
//! never an ownership check, and `userId` is otherwise only used for
//! `create`/`list`/`duplicate`. The practical result: any authenticated
//! Node user can read AND write any other user's plan by guessing (or
//! enumerating) its id — 26 of the 29 plan endpoints. Only `GET /plans`
//! filters by owner.
//!
//! This port refuses that. **Every function below takes `user_id` and
//! filters on it in SQL** — never left to a caller-side check that a
//! future refactor could accidentally skip. Rust returns 404 (`None`)
//! where Node would return 200 with someone else's data. Proven
//! load-bearing per function in `tests/plan.rs`; see each function's doc
//! comment for the exact experiment.
//!
//! `status` is deliberately `String`, not a Rust enum, and carries no DB
//! `CHECK` constraint — `plan.status` is unconstrained free text at both
//! the DB and Node-schema level (`packages/db/src/schema/plan.ts:21-22`),
//! validated only by an `Array.includes` check in Node's service layer.
//! Modelling it as an enum here would be validation Node does not have.
//! That validation is the service layer's job (a later task), not this
//! repo's.

mod analyze;
mod link;
mod requirement;
mod task;

pub use analyze::{
    PlanAnalyzeItem, create_analyze_item, delete_analyze_item, list_analyze_items,
    reorder_analyze_items, update_analyze_item,
};
pub use link::{PlanExternalLink, add_link, list_links, remove_link};
pub use requirement::{
    PlanRequirement, add_requirement, list_requirements, remove_requirement, reorder_requirements,
};
pub use task::{
    PlanTask, PlanTaskChunk, PlanTaskChunkWithTitle, PlanTaskDependency, PlanTaskExternalLink,
    add_task_chunk, add_task_dependency, add_task_link, create_task, delete_task, find_task_by_id,
    list_task_chunks_with_titles, list_task_dependencies, list_task_links, list_tasks,
    mark_task_done_and_unblock, remove_task_chunk, remove_task_dependency, remove_task_link,
    reorder_tasks, transition_task_in_tx, update_task,
};

use std::collections::HashMap;

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::repo::activity::Activity;
use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub user_id: String,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub completed_at: Option<UtcTimestamp>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: Json<serde_json::Value>,
}

// Plain column names, no `AS "col: Type"` macro-cast syntax — this constant
// feeds `QueryBuilder`, a runtime-built query the `sqlx::query_as!` macro
// never sees, so that syntax would be sent to Postgres as a literal (if
// unusual) quoted column alias instead of being stripped at compile time,
// and `Plan`'s `FromRow` derive would then fail to find a column actually
// named `created_at`. `FromRow` decodes each column into its declared Rust
// field type directly (`UtcTimestamp`, `Json<serde_json::Value>`) with no
// annotation needed — same pattern as `chunk::list`'s `QueryBuilder` query.
const PLAN_COLUMNS: &str = "id, title, description, status, user_id, space_id, created_at, updated_at, completed_at, metadata";

pub async fn create<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    user_id: &str,
    title: &str,
    description: Option<&str>,
    space_id: Option<&str>,
) -> AppResult<Plan> {
    let id = crate::new_id();
    let p = sqlx::query_as!(
        Plan,
        r#"INSERT INTO plan (id, title, description, user_id, space_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, title, description, status, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     completed_at AS "completed_at: UtcTimestamp",
                     metadata AS "metadata: Json<serde_json::Value>""#,
        id,
        title,
        description,
        user_id,
        space_id
    )
    .fetch_one(executor)
    .await?;
    Ok(p)
}

/// Looks up a plan by id, scoped to its owner in SQL. Removing the
/// `AND user_id = $2` predicate here is exactly divergence #13: Node's
/// `getPlan` has no such predicate at all, so any user could look up any
/// plan by id. Proven load-bearing in
/// `tests/plan.rs::find_by_id_is_user_scoped` — with the predicate
/// removed, Bob's lookup of Alice's plan stops returning `None` and
/// returns `Some(..)` instead.
pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Plan>> {
    let p = sqlx::query_as!(
        Plan,
        r#"SELECT id, title, description, status, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  completed_at AS "completed_at: UtcTimestamp",
                  metadata AS "metadata: Json<serde_json::Value>"
           FROM plan WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(p)
}

/// `list`'s filter set, mirroring Node's `ListPlansFilter`
/// (`packages/db/src/repository/plan.ts:32-38`) minus `userId` — that one
/// is always the mandatory first SQL parameter here, never optional.
#[derive(Debug, Default)]
pub struct ListFilter {
    pub space_id: Option<String>,
    pub status: Option<String>,
    /// Only plans linked (via `plan_requirement`) to this requirement id.
    pub requirement_id: Option<String>,
    /// When `false` (the default) and `status` is unset, plans with
    /// `status = 'archived'` are excluded — matching Node's
    /// `if (!filter.includeArchived && !filter.status) conditions.push(ne(plan.status,
    /// "archived"))` (`plan.ts:45-47`). An explicit `status` filter bypasses this
    /// entirely, same as Node.
    pub include_archived: bool,
}

/// Ordered `created_at ASC, id ASC`. Node's `listPlans` orders only by
/// `asc(plan.createdAt)` with no tiebreaker (`plan.ts:61`) — the `id`
/// tiebreaker is divergence #6, already accepted across this port: an
/// untied `ORDER BY` over rows that can share a `created_at` (routine with
/// batch inserts, or in the stability test below) is a query-plan
/// artifact, not a stable order. Proven load-bearing in
/// `list_breaks_created_at_ties_by_id`: with `, id ASC` removed, 20 plans
/// sharing one forced-identical `created_at` no longer come back in
/// ascending-id order.
pub async fn list(pool: &PgPool, user_id: &str, filter: ListFilter) -> AppResult<Vec<Plan>> {
    let mut qb = sqlx::QueryBuilder::new(format!("SELECT {PLAN_COLUMNS} FROM plan"));
    qb.push(" WHERE user_id = ").push_bind(user_id.to_string());

    if let Some(space_id) = &filter.space_id {
        qb.push(" AND space_id = ").push_bind(space_id.clone());
    }
    if let Some(status) = &filter.status {
        qb.push(" AND status = ").push_bind(status.clone());
    } else if !filter.include_archived {
        qb.push(" AND status <> 'archived'");
    }
    if let Some(requirement_id) = &filter.requirement_id {
        qb.push(" AND id IN (SELECT plan_id FROM plan_requirement WHERE requirement_id = ")
            .push_bind(requirement_id.clone())
            .push(")");
    }

    qb.push(" ORDER BY created_at ASC, id ASC");

    let rows = qb.build_query_as::<Plan>().fetch_all(pool).await?;
    Ok(rows)
}

/// Partial update of `title`/`description`/`status`. Plain `Option<&str>`
/// per field (not the tri-state `Option<Option<&str>>` some other repos
/// use for nullable columns) — `None` means "leave unchanged" for all
/// three, matching the `COALESCE` pattern (`chunk::update`,
/// `workspace::update`). If none of the three are present, this falls
/// back to a plain re-select under the same `WHERE id = $1 AND user_id =
/// $2` guard rather than issuing a no-op `UPDATE` — so `updated_at` is not
/// bumped by a no-op patch, matching Drizzle's `$onUpdate` hook, which
/// only fires on an actual `.update().set(...)` call.
///
/// The `WHERE id = $1 AND user_id = $2` guard — on BOTH the `UPDATE` and
/// the re-select branch — is divergence #13 again: Node's `updatePlan`
/// (`plan.ts:160-170`) updates by `id` alone. Proven load-bearing in
/// `tests/plan.rs::update_is_user_scoped_and_leaves_the_victims_row_intact`:
/// with `AND user_id = $2` removed from the `UPDATE` branch, Bob's
/// "Hijacked" title write against Alice's plan id stops returning `None`
/// and actually overwrites Alice's title.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
) -> AppResult<Option<Plan>> {
    let has_changes = title.is_some() || description.is_some() || status.is_some();

    let p = if has_changes {
        sqlx::query_as!(
            Plan,
            r#"UPDATE plan SET
                 title = COALESCE($3, title),
                 description = COALESCE($4, description),
                 status = COALESCE($5, status),
                 updated_at = now()
               WHERE id = $1 AND user_id = $2
               RETURNING id, title, description, status, user_id, space_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp",
                         completed_at AS "completed_at: UtcTimestamp",
                         metadata AS "metadata: Json<serde_json::Value>""#,
            id,
            user_id,
            title,
            description,
            status
        )
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as!(
            Plan,
            r#"SELECT id, title, description, status, user_id, space_id,
                      created_at AS "created_at: UtcTimestamp",
                      updated_at AS "updated_at: UtcTimestamp",
                      completed_at AS "completed_at: UtcTimestamp",
                      metadata AS "metadata: Json<serde_json::Value>"
               FROM plan WHERE id = $1 AND user_id = $2"#,
            id,
            user_id
        )
        .fetch_optional(pool)
        .await?
    };
    Ok(p)
}

/// Deletes a plan, scoped to its owner in SQL. Child rows in
/// `plan_requirement`, `plan_analyze_item`, `plan_task`, and
/// `plan_external_link` (and, transitively, `plan_task_chunk`,
/// `plan_task_dependency`, `plan_task_external_link` off `plan_task`) are
/// removed by the database's own `ON DELETE CASCADE` foreign keys
/// (`migrations/0001_init.sql`), matching Node's `deletePlan`
/// (`plan.ts:286-290`), which relies on the same cascade and does not
/// touch any child table itself.
///
/// Node's `deletePlan(id)` deletes by `id` alone with no ownership check
/// in either the repo or (per divergence #13) the service layer above it.
/// This is where an unscoped delete would be most damaging — proven
/// load-bearing in `tests/plan.rs::delete_is_user_scoped`: with `AND
/// user_id = $2` removed, Bob's delete of Alice's plan id stops returning
/// `false` and actually deletes her row (and, via cascade, everything
/// under it).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM plan WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Deep-copies a plan and everything under it — requirements, analyze
/// items, tasks (reset to `pending`), task-chunk links, and task
/// dependencies — inside one transaction, matching Node's `duplicatePlan`
/// (`plan.ts:177-284`), the one place Node's own plan repo already uses a
/// transaction. `plan_external_link`/`plan_task_external_link` are
/// deliberately NOT copied — Node's `duplicatePlan` does not copy them
/// either.
///
/// The child-table repo helpers (`plan_requirement`, `plan_analyze_item`,
/// `plan_task`, `plan_task_chunk`, `plan_task_dependency`) do not exist
/// yet — later tasks add them to this file — so this function reaches
/// those tables directly with SQL rather than waiting on them, exactly as
/// the task brief directs.
///
/// Returns `None` if `source_id` does not name a plan owned by `user_id`
/// — unlike Node, whose repo-level ownership check
/// (`and(eq(plan.id, sourceId), eq(plan.userId, userId))`, `plan.ts:180-183`)
/// throws inside the transaction, which `dbEffect` turns into a
/// `DatabaseError` (HTTP 500) even though the service layer's own
/// unscoped `getPlan(sourceId)` pre-check already passed. Surfacing that
/// as a clean `None` here (mapped to 404 by the service layer, like every
/// other guard in this file) is more correct than reproducing Node's
/// 500-on-cross-user-duplicate accident. Proven load-bearing in
/// `tests/plan.rs::duplicate_is_user_scoped`: with the ownership predicate
/// removed from the source `SELECT`, Bob's duplicate of Alice's plan id
/// stops returning `None` and actually creates a copy owned by Bob.
pub async fn duplicate(pool: &PgPool, user_id: &str, source_id: &str) -> AppResult<Option<Plan>> {
    let mut tx = pool.begin().await?;

    let source = sqlx::query_as!(
        Plan,
        r#"SELECT id, title, description, status, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  completed_at AS "completed_at: UtcTimestamp",
                  metadata AS "metadata: Json<serde_json::Value>"
           FROM plan WHERE id = $1 AND user_id = $2"#,
        source_id,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(source) = source else {
        tx.rollback().await?;
        return Ok(None);
    };

    let new_plan_id = crate::new_id();
    let new_title = format!("{} (copy)", source.title);
    let new_plan = sqlx::query_as!(
        Plan,
        r#"INSERT INTO plan (id, title, description, status, user_id, space_id, metadata)
           VALUES ($1, $2, $3, 'draft', $4, $5, $6)
           RETURNING id, title, description, status, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     completed_at AS "completed_at: UtcTimestamp",
                     metadata AS "metadata: Json<serde_json::Value>""#,
        new_plan_id,
        new_title,
        source.description,
        source.user_id,
        source.space_id,
        source.metadata as Json<serde_json::Value>
    )
    .fetch_one(&mut *tx)
    .await?;

    // Requirements — preserve order, same requirement_id.
    let reqs = sqlx::query!(
        r#"SELECT requirement_id, "order" FROM plan_requirement WHERE plan_id = $1"#,
        source_id
    )
    .fetch_all(&mut *tx)
    .await?;
    for r in reqs {
        sqlx::query!(
            r#"INSERT INTO plan_requirement (id, plan_id, requirement_id, "order")
               VALUES ($1, $2, $3, $4)"#,
            crate::new_id(),
            new_plan_id,
            r.requirement_id,
            r.order
        )
        .execute(&mut *tx)
        .await?;
    }

    // Analyze items — preserve kind/order/chunk_id/file_path/text/metadata.
    let items = sqlx::query!(
        r#"SELECT kind, "order", chunk_id, file_path, text, metadata
           FROM plan_analyze_item WHERE plan_id = $1"#,
        source_id
    )
    .fetch_all(&mut *tx)
    .await?;
    for i in items {
        sqlx::query!(
            r#"INSERT INTO plan_analyze_item
                 (id, plan_id, kind, "order", chunk_id, file_path, text, metadata)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            crate::new_id(),
            new_plan_id,
            i.kind,
            i.order,
            i.chunk_id,
            i.file_path,
            i.text,
            i.metadata
        )
        .execute(&mut *tx)
        .await?;
    }

    // Tasks — remap ids so chunk links and dependencies can be rewritten.
    // Status is reset to "pending", matching Node's `status: "pending" as
    // PlanTaskStatus` (`plan.ts:241`) regardless of the source task's status.
    let source_tasks = sqlx::query!(
        r#"SELECT id, title, description, acceptance_criteria, "order", metadata
           FROM plan_task WHERE plan_id = $1"#,
        source_id
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut task_id_map: HashMap<String, String> = HashMap::new();
    for t in &source_tasks {
        task_id_map.insert(t.id.clone(), crate::new_id());
    }
    for t in &source_tasks {
        let new_task_id = &task_id_map[&t.id];
        sqlx::query!(
            r#"INSERT INTO plan_task
                 (id, plan_id, title, description, acceptance_criteria, status, "order", metadata)
               VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7)"#,
            new_task_id,
            new_plan_id,
            t.title,
            t.description,
            t.acceptance_criteria,
            t.order,
            t.metadata
        )
        .execute(&mut *tx)
        .await?;
    }

    if !source_tasks.is_empty() {
        let source_task_ids: Vec<String> = source_tasks.iter().map(|t| t.id.clone()).collect();

        // Task -> chunk links.
        let links = sqlx::query!(
            r#"SELECT task_id, chunk_id, relation
               FROM plan_task_chunk WHERE task_id = ANY($1)"#,
            &source_task_ids
        )
        .fetch_all(&mut *tx)
        .await?;
        for l in links {
            let Some(new_task_id) = task_id_map.get(&l.task_id) else {
                continue;
            };
            sqlx::query!(
                r#"INSERT INTO plan_task_chunk (id, task_id, chunk_id, relation)
                   VALUES ($1, $2, $3, $4)"#,
                crate::new_id(),
                new_task_id,
                l.chunk_id,
                l.relation
            )
            .execute(&mut *tx)
            .await?;
        }

        // Task dependencies — only copied when BOTH ends remap, matching
        // Node's `.filter(d => taskIdMap.has(d.taskId) && taskIdMap.has(d.dependsOnTaskId))`
        // (`plan.ts:271`).
        let deps = sqlx::query!(
            r#"SELECT task_id, depends_on_task_id
               FROM plan_task_dependency WHERE task_id = ANY($1)"#,
            &source_task_ids
        )
        .fetch_all(&mut *tx)
        .await?;
        for d in deps {
            let (Some(new_task_id), Some(new_depends_on)) = (
                task_id_map.get(&d.task_id),
                task_id_map.get(&d.depends_on_task_id),
            ) else {
                continue;
            };
            sqlx::query!(
                r#"INSERT INTO plan_task_dependency (id, task_id, depends_on_task_id)
                   VALUES ($1, $2, $3)"#,
                crate::new_id(),
                new_task_id,
                new_depends_on
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(Some(new_plan))
}

/// Row shape of `GET /api/plans` — `Plan`'s own columns plus four rollups
/// used by the list page: the linked space's name (`codebaseName`), a
/// task-count progress pair, the title of the first non-`done` task
/// (`nextAction`), and the more-recent of the plan's own `updated_at` and
/// its tasks' `updated_at` (`lastActivityAt`). Mirrors Node's
/// `listPlansWithRollups` / `PlanListRow`
/// (`packages/db/src/repository/plan.ts:65-143`) field-for-field — see
/// `tests/fixtures/node-contract-2c/plans-list.json`, captured live, which
/// carries all five rollup fields even though the task brief's own endpoint
/// table just says "bare array". Trusting the fixture over the table's
/// shorthand is deliberate, per this slice's own instruction to check each
/// fixture rather than reason by analogy.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanListRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub user_id: String,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub completed_at: Option<UtcTimestamp>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: Json<serde_json::Value>,
    pub codebase_name: Option<String>,
    pub task_total: i64,
    pub task_done: i64,
    pub next_action: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub last_activity_at: UtcTimestamp,
}

#[derive(Debug, sqlx::FromRow)]
struct RollupRow {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    user_id: String,
    space_id: Option<String>,
    created_at: UtcTimestamp,
    updated_at: UtcTimestamp,
    completed_at: Option<UtcTimestamp>,
    metadata: Json<serde_json::Value>,
    codebase_name: Option<String>,
    task_total: i64,
    task_done: i64,
    last_task_update: Option<UtcTimestamp>,
}

/// `plan::list` + per-row rollups, used by `GET /api/plans`. Same
/// `user_id`/`ListFilter` scoping as `plan::list` — `GET /plans` is the one
/// plan endpoint Node itself already scopes by owner, so this adds no new
/// divergence, just the extra columns.
///
/// `LEFT JOIN plan_task` + `GROUP BY p.id, s.name` mirrors Node's own query
/// (`plan.ts:100-113`) exactly, including relying on `plan.id` being a
/// primary key so Postgres allows every other selected `plan` column
/// without listing it in `GROUP BY` (functional dependency) — the same
/// trick Node's Drizzle query leans on.
pub async fn list_with_rollups(
    pool: &PgPool,
    user_id: &str,
    filter: ListFilter,
) -> AppResult<Vec<PlanListRow>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT p.id, p.title, p.description, p.status, p.user_id, p.space_id, \
         p.created_at, p.updated_at, p.completed_at, p.metadata, \
         s.name AS codebase_name, \
         COUNT(pt.id) AS task_total, \
         COUNT(*) FILTER (WHERE pt.status = 'done') AS task_done, \
         MAX(pt.updated_at) AS last_task_update \
         FROM plan p \
         LEFT JOIN space s ON s.id = p.space_id \
         LEFT JOIN plan_task pt ON pt.plan_id = p.id",
    );
    qb.push(" WHERE p.user_id = ")
        .push_bind(user_id.to_string());

    if let Some(space_id) = &filter.space_id {
        qb.push(" AND p.space_id = ").push_bind(space_id.clone());
    }
    if let Some(status) = &filter.status {
        qb.push(" AND p.status = ").push_bind(status.clone());
    } else if !filter.include_archived {
        qb.push(" AND p.status <> 'archived'");
    }
    if let Some(requirement_id) = &filter.requirement_id {
        qb.push(" AND p.id IN (SELECT plan_id FROM plan_requirement WHERE requirement_id = ")
            .push_bind(requirement_id.clone())
            .push(")");
    }

    qb.push(" GROUP BY p.id, s.name");
    qb.push(" ORDER BY p.created_at ASC, p.id ASC");

    let rows = qb.build_query_as::<RollupRow>().fetch_all(pool).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    // Second pass: title of the first non-done task per plan, matching
    // Node's `SELECT DISTINCT ON (plan_id) ... ORDER BY plan_id, "order"`
    // (`plan.ts:119-128`).
    let plan_ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let next_rows = sqlx::query!(
        r#"SELECT DISTINCT ON (plan_id) plan_id, title
           FROM plan_task
           WHERE plan_id = ANY($1) AND status <> 'done'
           ORDER BY plan_id, "order""#,
        &plan_ids
    )
    .fetch_all(pool)
    .await?;
    let mut next_action_map: HashMap<String, String> = HashMap::new();
    for r in next_rows {
        next_action_map.insert(r.plan_id, r.title);
    }

    Ok(rows
        .into_iter()
        .map(|r| {
            let last_activity_at = match r.last_task_update {
                Some(t) if t > r.updated_at => t,
                _ => r.updated_at,
            };
            let next_action = next_action_map.get(&r.id).cloned();
            PlanListRow {
                id: r.id,
                title: r.title,
                description: r.description,
                status: r.status,
                user_id: r.user_id,
                space_id: r.space_id,
                created_at: r.created_at,
                updated_at: r.updated_at,
                completed_at: r.completed_at,
                metadata: r.metadata,
                codebase_name: r.codebase_name,
                task_total: r.task_total,
                task_done: r.task_done,
                next_action,
                last_activity_at,
            }
        })
        .collect())
}

/// How a `PATCH` should treat `completed_at`, computed by the service layer
/// from the existing row's status vs. the incoming one — mirrors Node's
/// `updatePlan` (`packages/api/src/plans/service.ts:178-184`): entering
/// `"completed"` sets `completed_at = now()`, leaving it sets it back to
/// `NULL`, anything else leaves the column untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletedAtPatch {
    Unchanged,
    SetNow,
    Clear,
}

/// General `PATCH /api/plans/{id}` update, superseding `fubbik_db::repo::
/// plan::update` for every field that endpoint actually needs to touch:
/// that function only threads `title`/`description`/`status` and has no
/// way to express "clear this column" (a bound `NULL` under `COALESCE`
/// means "don't change", not "clear"). `description` and `space_id` both
/// need real tri-state ("omitted" vs "explicit null" vs "a value"), and
/// `status`'s `completed_at` side effect and `metadata`'s wholesale
/// replacement aren't expressible through `plan::update` at all.
///
/// Tri-state fields use the `CASE WHEN $flag::bool THEN $value ELSE column
/// END` shape (not `COALESCE`, which cannot represent "set to NULL") — same
/// pattern as `fubbik_db::repo::workspace::update`'s `description`
/// handling. `title`/`status`/`metadata` stay plain `COALESCE`, since none
/// of the three is tri-state in Node's own schema.
///
/// `AND user_id = $2` on the `UPDATE` — divergence #13 again, same guard
/// shape as `plan::update`.
#[allow(clippy::too_many_arguments)]
pub async fn apply_patch<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    user_id: &str,
    id: &str,
    title: Option<&str>,
    description: Option<Option<&str>>,
    status: Option<&str>,
    space_id: Option<Option<&str>>,
    metadata: Option<serde_json::Value>,
    completed_at: CompletedAtPatch,
) -> AppResult<Option<Plan>> {
    let (desc_set, desc_val) = match description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let (space_set, space_val) = match space_id {
        Some(v) => (true, v),
        None => (false, None),
    };
    let (completed_set, completed_clear) = match completed_at {
        CompletedAtPatch::Unchanged => (false, false),
        CompletedAtPatch::SetNow => (true, false),
        CompletedAtPatch::Clear => (true, true),
    };

    let has_changes = title.is_some()
        || desc_set
        || status.is_some()
        || space_set
        || metadata.is_some()
        || completed_set;

    let p = if has_changes {
        sqlx::query_as!(
            Plan,
            r#"UPDATE plan SET
                 title = COALESCE($3, title),
                 description = CASE WHEN $4::bool THEN $5 ELSE description END,
                 status = COALESCE($6, status),
                 space_id = CASE WHEN $7::bool THEN $8 ELSE space_id END,
                 metadata = COALESCE($9, metadata),
                 completed_at = CASE
                   WHEN $10::bool THEN (CASE WHEN $11::bool THEN NULL::timestamp ELSE now() END)
                   ELSE completed_at
                 END,
                 updated_at = now()
               WHERE id = $1 AND user_id = $2
               RETURNING id, title, description, status, user_id, space_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp",
                         completed_at AS "completed_at: UtcTimestamp",
                         metadata AS "metadata: Json<serde_json::Value>""#,
            id,
            user_id,
            title,
            desc_set,
            desc_val,
            status,
            space_set,
            space_val,
            metadata,
            completed_set,
            completed_clear
        )
        .fetch_optional(executor)
        .await?
    } else {
        sqlx::query_as!(
            Plan,
            r#"SELECT id, title, description, status, user_id, space_id,
                      created_at AS "created_at: UtcTimestamp",
                      updated_at AS "updated_at: UtcTimestamp",
                      completed_at AS "completed_at: UtcTimestamp",
                      metadata AS "metadata: Json<serde_json::Value>"
               FROM plan WHERE id = $1 AND user_id = $2"#,
            id,
            user_id
        )
        .fetch_optional(executor)
        .await?
    };
    Ok(p)
}

/// Used internally by the plans domain to build the merged plan+task
/// activity feed (`GET /api/plans/{id}/activity`). Matches Node's internal
/// `listActivity(userId, {entityType, entityId, limit})` call shape at
/// `packages/api/src/plans/routes.ts:165-173`: a plan-level call passes
/// `entity_id = Some(planId)`, a task-level call passes `entity_id = None`
/// (fetches every `plan_task` event for the user, filtered down to this
/// plan's task ids by the caller).
pub async fn list_activity_by_entity(
    pool: &PgPool,
    user_id: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<Activity>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, user_id, entity_type, entity_id, entity_title, action, space_id, created_at \
         FROM activity_log WHERE user_id = ",
    );
    qb.push_bind(user_id.to_string());
    qb.push(" AND entity_type = ")
        .push_bind(entity_type.to_string());
    if let Some(entity_id) = entity_id {
        qb.push(" AND entity_id = ")
            .push_bind(entity_id.to_string());
    }
    qb.push(" ORDER BY created_at DESC, id ASC");
    qb.push(" LIMIT ").push_bind(limit);

    let rows = qb.build_query_as::<Activity>().fetch_all(pool).await?;
    Ok(rows)
}
