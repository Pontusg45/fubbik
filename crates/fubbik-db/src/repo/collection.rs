//! Collections are **saved queries, not containers**. `collection` is
//! `(id, name, description, filter jsonb, user_id, space_id, created_at,
//! updated_at)` — there is no join table. `GET /collections/{id}/chunks`
//! (`fubbik_api::collections::service::get_chunks`) does not build a query
//! of its own: it loads this row and calls the exact same `chunk::list` /
//! `chunk::count` that back `GET /api/chunks`, mapping the nine
//! `CollectionFilter` keys onto `chunk::ListParams`
//! (`packages/api/src/collections/service.ts:59-78`). See
//! `fubbik_api::collections::service` for that mapping.
//!
//! `collection.filter` is stored **verbatim** — every one of its nine keys
//! is `Option<String>` with no shape or value validation beyond "is a
//! string", matching `CollectionFilterSchema`
//! (`packages/api/src/collections/routes.ts:7-17`). A client can (and
//! seed data does) store `{"sort": "bogus-value"}`; it round-trips
//! unchanged on every read and is only *interpreted* — leniently, silently
//! ignoring anything it doesn't recognise — at the moment
//! `GET /collections/{id}/chunks` actually maps it onto `ListParams`.
//!
//! **`collection.space_id` ownership is checked on create — a divergence
//! from Node.** Node's `createCollection`
//! (`packages/db/src/repository/collection.ts:22-34`) is a bare insert with
//! no check that `spaceId` belongs to the caller; any authenticated user
//! could create a collection pinned to another user's space. This port adds
//! an `EXISTS (SELECT 1 FROM space s WHERE s.id = .. AND s.user_id = ..)`
//! guard, the same shape as accepted divergences #4 (`space::reset`), #9
//! (`codebase_settings`), and #10 (`activity`'s `space_id` filter) — see
//! `create`'s doc comment and `tests/collection.rs` for the load-bearing
//! proof.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// The nine keys `CollectionFilterSchema` accepts, all optional strings,
/// stored and echoed back exactly as received —
/// `#[serde(skip_serializing_if = "Option::is_none")]` on every field keeps
/// a partial filter (e.g. seed data's `{"type": "convention"}`) from
/// growing eight extra `null` keys on the way back out. See
/// `tests/fixtures/node-contract-2b/collections-list.json`.
#[derive(
    Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionFilter {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub filter_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrichment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_connections: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_status: Option<String>,
}

/// `camelCase` serialisation matches every other wire type in this crate.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[schema(value_type = CollectionFilter)]
    pub filter: Json<CollectionFilter>,
    pub user_id: String,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

pub struct NewCollection {
    pub name: String,
    pub description: Option<String>,
    pub filter: CollectionFilter,
    pub space_id: Option<String>,
}

/// Creates a collection. When `space_id` is `Some`, the insert carries an
/// `EXISTS` ownership guard in the same statement (the `set_codebase_setting`
/// / `workspace::add_space` "proven pattern" — see this module's doc
/// comment for why Node itself has no such guard); a `space_id` that
/// doesn't belong to the caller inserts nothing and this returns `None`.
/// The caller (`collections::service::create`) also pre-checks ownership
/// via `space::find_by_id` before reaching here, matching the two-layer
/// shape `settings::service::set_codebase_setting` uses — see
/// `tests/collection.rs` for proof both layers are independently
/// load-bearing.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    new: NewCollection,
) -> AppResult<Option<Collection>> {
    let id = crate::new_id();
    let row = match &new.space_id {
        Some(space_id) => {
            sqlx::query_as!(
                Collection,
                r#"INSERT INTO collection (id, name, description, filter, user_id, space_id)
                   SELECT $1, $2, $3, $4, $5, $6
                   WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $6 AND s.user_id = $5)
                   RETURNING id, name, description,
                             filter AS "filter: Json<CollectionFilter>",
                             user_id, space_id,
                             created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp""#,
                id,
                new.name,
                new.description,
                Json(&new.filter) as _,
                user_id,
                space_id
            )
            .fetch_optional(pool)
            .await?
        }
        None => Some(
            sqlx::query_as!(
                Collection,
                r#"INSERT INTO collection (id, name, description, filter, user_id, space_id)
                   VALUES ($1, $2, $3, $4, $5, NULL)
                   RETURNING id, name, description,
                             filter AS "filter: Json<CollectionFilter>",
                             user_id, space_id,
                             created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp""#,
                id,
                new.name,
                new.description,
                Json(&new.filter) as _,
                user_id
            )
            .fetch_one(pool)
            .await?,
        ),
    };
    Ok(row)
}

pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Collection>> {
    let row = sqlx::query_as!(
        Collection,
        r#"SELECT id, name, description, filter AS "filter: Json<CollectionFilter>",
                  user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM collection WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `, id ASC` is a tiebreaker over `name`, which is not unique across rows
/// in the same sense `created_at` isn't elsewhere in this crate — see
/// `chunk::list`'s equivalent comment. Node's `listCollections`
/// (`packages/db/src/repository/collection.ts:8-10`) orders by `name` alone
/// with no secondary key.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Collection>> {
    let rows = sqlx::query_as!(
        Collection,
        r#"SELECT id, name, description, filter AS "filter: Json<CollectionFilter>",
                  user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM collection WHERE user_id = $1 ORDER BY name ASC, id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// All three fields are plain two-state (`None` = leave untouched, `Some` =
/// set) — unlike `workspace::WorkspacePatch::description`, Node's PATCH body
/// here (`t.Optional(t.String())` / `t.Optional(CollectionFilterSchema)`,
/// `packages/api/src/collections/routes.ts:54-58`) has no `t.Null()`
/// variant on any field, so there is no way to explicitly clear
/// `description` through this endpoint at all. `filter`, when provided,
/// **replaces the stored object wholesale** — `COALESCE($5, filter)` with a
/// full new JSON value does not merge keys, matching Node's `.set(params)`
/// where `params.filter` is passed through as-is
/// (`packages/db/src/repository/collection.ts:36-53`, confirmed in
/// `_mutating.md`).
#[derive(Default)]
pub struct CollectionPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub filter: Option<CollectionFilter>,
}

/// Node's `updateCollection` always issues the `UPDATE` (no no-op
/// short-circuit the way `workspace::update` has) — `collection.updatedAt`
/// has a Drizzle `$onUpdate` hook that fires on any `.set(...)` call, even
/// an empty one, so an all-omitted PATCH body still bumps `updated_at`.
/// This mirrors that: the `UPDATE` always runs, `updated_at = now()`
/// unconditionally.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: CollectionPatch,
) -> AppResult<Option<Collection>> {
    let row = sqlx::query_as!(
        Collection,
        r#"UPDATE collection SET
             name = COALESCE($3, name),
             description = COALESCE($4, description),
             filter = COALESCE($5, filter),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, description, filter AS "filter: Json<CollectionFilter>",
                     user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.name,
        patch.description,
        patch.filter.map(Json) as _
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM collection WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
