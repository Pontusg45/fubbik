//! `activity_log` is a read-only audit trail from the HTTP surface's point
//! of view: Node exposes exactly one route, `GET /activity`
//! (`packages/api/src/activity/routes.ts`). `createActivity`
//! (`packages/db/src/repository/activity.ts:31-44`) exists only as an
//! internal function other domains call directly for fire-and-forget audit
//! logging — there is no `POST /activity` route in Node, and this port
//! does not invent one (the same call made for `notification`, which has
//! no repo-level `create` either). Tests here seed rows with a raw
//! `INSERT`, same approach as `notification.rs`'s tests.
//!
//! `action` and `entity_type` are both `text NOT NULL` with **no CHECK
//! constraint and no DB enum** — `packages/db/src/schema/activity.ts:6-25`
//! only comments example values (`"created"`, `"updated"`, `"deleted"`,
//! `"archived"`, `"restored"` for `action`; `"chunk"`, `"requirement"`,
//! `"connection"`, `"tag"`, `"codebase"` for `entity_type`). Same shape as
//! `notification::Notification::notification_type` — see that module's
//! doc comment for why the Phase 2b plan's "model constrained sets as
//! enums" guidance does not apply here. Both fields stay plain `String`.
//!
//! `entity_id` has no filter exposed over HTTP at all: the service's
//! options type accepts `entityId`, but the Elysia query schema
//! (`routes.ts:23-28`) never declares it, so it's dead from the client's
//! perspective. Not reproduced as a query param here for the same reason —
//! there is no route to reach it through.
//!
//! **Ownership scoping is a deliberate divergence from Node.** Node's
//! `listActivity` (`packages/db/src/repository/activity.ts:6-29`) scopes
//! every query by `user_id` in SQL — that part is faithfully reproduced
//! below — but its optional `spaceId` filter is a bare
//! `eq(activityLog.spaceId, opts.spaceId)` with no check that the space
//! belongs to the caller. Because every row is already restricted to the
//! caller's own `activity_log` entries by the `user_id` predicate, this is
//! not the same class of leak Task 5 found in `codebase_settings` (there,
//! `codebaseId` pointed directly at another user's row with no `user_id`
//! predicate at all) — a foreign `spaceId` here can only ever match zero
//! of the caller's own rows. The task brief still calls for the same
//! `EXISTS`-based guard shape used by `codebase_settings` and the
//! `chunk_applies_to` / `chunk_file_ref` parent-scoping pattern, both for
//! consistency and as defense against a future caller that filters by
//! `space_id` without also filtering by `user_id`. Combined with the
//! service-level ownership pre-check
//! (`fubbik_api::activity::service::list`), this makes a foreign
//! `spaceId` a clean 404 in the Rust port where Node returns 200 with an
//! empty array — see that service function's doc comment for the fuller
//! account of the divergence, and `tests/activity.rs` in this crate for
//! proof both guards are load-bearing.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub user_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub entity_title: Option<String>,
    pub action: String,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone)]
pub struct ListParams {
    pub space_id: Option<String>,
    pub entity_type: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListParams {
    /// Node's repo defaults (`opts.limit ?? 50`, `opts.offset ?? 0`), with
    /// no clamping beyond that — Node passes whatever numeric value the
    /// caller sends straight to `.limit()`/`.offset()`, so this port
    /// doesn't invent a clamp it doesn't have either.
    fn default() -> Self {
        Self {
            space_id: None,
            entity_type: None,
            limit: 50,
            offset: 0,
        }
    }
}

/// Lists a user's activity log entries, newest first.
///
/// Uses `QueryBuilder` rather than `query_as!` because the filter set
/// (`space_id`, `entity_type`) is dynamic, same rationale as
/// `chunk::list`.
///
/// `WHERE user_id = ...` is the sole scoping predicate Node has; the
/// `AND EXISTS (SELECT 1 FROM space s WHERE s.id = ... AND s.user_id =
/// ...)` clause on the `space_id` filter is the deliberate divergence
/// described in this module's doc comment — see `tests/activity.rs` for
/// the load-bearing proof that removing either predicate breaks a named
/// test.
///
/// `ORDER BY created_at DESC, id ASC`: `created_at` is not unique —
/// activity rows are written in bursts (several audit-log inserts firing
/// off the same chunk/requirement/connection mutation), so ties are the
/// norm, not the exception, here more than in most other domains. Same bug
/// class as `chunk::list`, `notification::list`, `tag::list`,
/// `tag_type::list`, `space::list` — an `ORDER BY` over tied rows with no
/// deterministic tiebreaker is a query-plan artifact, not a stable order,
/// and can skip or duplicate rows across a `LIMIT`/`OFFSET` walk.
pub async fn list(pool: &PgPool, user_id: &str, params: &ListParams) -> AppResult<Vec<Activity>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, user_id, entity_type, entity_id, entity_title, action, space_id, created_at \
         FROM activity_log WHERE user_id = ",
    );
    qb.push_bind(user_id.to_string());

    if let Some(space_id) = &params.space_id {
        qb.push(" AND space_id = ").push_bind(space_id.clone());
        qb.push(" AND EXISTS (SELECT 1 FROM space s WHERE s.id = ")
            .push_bind(space_id.clone());
        qb.push(" AND s.user_id = ").push_bind(user_id.to_string());
        qb.push(")");
    }

    if let Some(entity_type) = &params.entity_type {
        qb.push(" AND entity_type = ")
            .push_bind(entity_type.clone());
    }

    qb.push(" ORDER BY created_at DESC, id ASC");
    qb.push(" LIMIT ").push_bind(params.limit);
    qb.push(" OFFSET ").push_bind(params.offset);

    let rows = qb.build_query_as::<Activity>().fetch_all(pool).await?;
    Ok(rows)
}

/// Writes one fire-and-forget audit-log row, matching Node's
/// `createActivity` (`packages/db/src/repository/activity.ts:31-44`) —
/// called directly by other domains' service layers (first consumer: the
/// `plans` domain's task create/update/delete handlers), never exposed as
/// its own HTTP route, same as Node. No ownership guard needed here: unlike
/// every read path in this module, a `create` call always writes under the
/// caller's own `user_id`, supplied by the service layer from the
/// authenticated session — there is no id a caller could substitute to
/// write into someone else's log.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    entity_type: &str,
    entity_id: &str,
    entity_title: Option<&str>,
    action: &str,
    space_id: Option<&str>,
) -> AppResult<Activity> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        Activity,
        r#"INSERT INTO activity_log (id, user_id, entity_type, entity_id, entity_title, action, space_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, user_id, entity_type, entity_id, entity_title, action, space_id,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id,
        entity_type,
        entity_id,
        entity_title,
        action,
        space_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}
