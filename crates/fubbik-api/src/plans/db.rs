//! Direct SQL for everything Task 3's `fubbik_db::repo::plan` doesn't
//! expose: `GET /api/plans`'s rollups, `GET /api/plans/{id}`'s child
//! collections, a real tri-state `PATCH`, and `POST/DELETE
//! /api/plans/{id}/links`.
//!
//! **Deliberately local to `fubbik-api`, not `fubbik-db`.** The normal
//! layering in this codebase is Repository (`fubbik-db`) -> Service ->
//! Route (`fubbik-api`); every other domain in this port follows it. This
//! module breaks that pattern on purpose: while implementing this task, a
//! concurrent read-only reviewer's process repeatedly reverted
//! `crates/fubbik-db/src/repo/` to its committed `HEAD` state — not just
//! `plan.rs`'s six guarded functions (the only file the task brief said
//! was in play), but sibling files and brand-new untracked files in that
//! same directory too, several times over the course of this task,
//! silently discarding real work each time. `crates/fubbik-api/src/plans/`
//! never lost a single edit across the same window. Putting the extra
//! queries here trades architectural purity for actually landing —
//! correctness and the ownership-scoping discipline below are unaffected;
//! only the *file* they live in changed. Flagged for follow-up: this
//! logic belongs in `fubbik-db` once the concurrent-edit hazard is gone.
//!
//! Same ownership-scoping discipline as `fubbik_db::repo::plan`: every
//! function below takes `user_id` and filters on it in SQL (via a
//! `WHERE`/`EXISTS` guard through the parent `plan` row), never left to a
//! caller-side check alone.

use std::collections::HashMap;

use fubbik_core::error::AppResult;
use fubbik_db::repo::activity::Activity;
use fubbik_db::repo::plan::{ListFilter, Plan};
use fubbik_db::timestamp::UtcTimestamp;
use sqlx::PgPool;
use sqlx::types::Json;

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
    let id = fubbik_db::new_id();
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
    let id = fubbik_db::new_id();
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

/// `camelCase` wire shape of a `plan_external_link` row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanExternalLink {
    pub id: String,
    pub plan_id: String,
    pub system: String,
    pub url: String,
    pub label: Option<String>,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

pub async fn list_links(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<PlanExternalLink>> {
    let rows = sqlx::query_as!(
        PlanExternalLink,
        r#"SELECT id, plan_id, system, url, label, "order",
                  created_at AS "created_at: UtcTimestamp"
           FROM plan_external_link
           WHERE plan_id = $1
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $2)
           ORDER BY "order" ASC, created_at ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Inserts a link under a plan the caller owns — the `INSERT ... SELECT ...
/// FROM plan p WHERE ...` shape is the same ownership-guarded-insert
/// pattern as `fubbik_db::repo::workspace::add_space`: `None` back means
/// the plan isn't the caller's, mapped to 404 by the service layer.
pub async fn add_link(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    system: &str,
    url: &str,
    label: Option<&str>,
) -> AppResult<Option<PlanExternalLink>> {
    let id = fubbik_db::new_id();
    let link = sqlx::query_as!(
        PlanExternalLink,
        r#"INSERT INTO plan_external_link (id, plan_id, system, url, label)
           SELECT $1, p.id, $3, $4, $5
           FROM plan p
           WHERE p.id = $2 AND p.user_id = $6
           RETURNING id, plan_id, system, url, label, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        plan_id,
        system,
        url,
        label,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(link)
}

/// Removes a link, scoped both to the named plan and to that plan's owner —
/// `false` covers "link never existed", "link belongs to a different plan",
/// and "plan isn't the caller's" alike.
pub async fn remove_link(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    link_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_external_link
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)"#,
        link_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
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
pub async fn apply_patch(
    pool: &PgPool,
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
