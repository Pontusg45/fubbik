//! `saved_graph` — a user's saved graph-view snapshot: which chunks to
//! show, their canvas positions, and the layout algorithm
//! (`packages/db/src/schema/saved-graph.ts:7-31`). Ownership is a plain
//! `user_id` column on the row itself, like `saved_query` and unlike
//! `favorite` (whose ownership is derived from the chunk it points at).
//!
//! Unlike `collection.filter` / `saved_query.query` (both opaque
//! `serde_json::Value` blobs never re-validated on read), `chunk_ids` and
//! `positions` are **structurally typed** end to end: Node's Elysia route
//! schema requires `chunkIds: t.Array(t.String())` and
//! `positions: t.Record(t.String(), t.Object({ x: t.Number(), y: t.Number() }))`
//! (`packages/api/src/saved-graphs/routes.ts:29-37`), so a client cannot
//! store an arbitrary shape there the way it can with a collection's filter
//! or a saved query. This port keeps that same structure — `Vec<String>`
//! and `HashMap<String, Position>` — rather than degrading to
//! `serde_json::Value`; serde's own deserialization failure on a malformed
//! body plays the same role Elysia's schema validation does in Node.

use std::collections::HashMap;

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// A node position on the graph canvas. `f64` matches Elysia's `t.Number()`
/// (a JS `number`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
// Distinct OpenAPI name: `vocabulary::parser::Position` is a different shape
// ({start,end} vs {x,y}) and both would otherwise register as
// `#/components/schemas/Position`, silently publishing the wrong one.
#[schema(as = GraphNodePosition)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedGraph {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub chunk_ids: Json<Vec<String>>,
    #[schema(value_type = HashMap<String, Position>)]
    pub positions: Json<HashMap<String, Position>>,
    pub layout_algorithm: String,
    pub user_id: String,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

pub struct NewSavedGraph {
    pub name: String,
    pub description: Option<String>,
    pub chunk_ids: Vec<String>,
    pub positions: HashMap<String, Position>,
    pub layout_algorithm: String,
    pub space_id: Option<String>,
}

/// Inserts a saved graph. No check that `space_id` belongs to the caller —
/// Node's `createSavedGraph`
/// (`packages/db/src/repository/saved-graph.ts:29-34`) is a bare insert
/// with no such guard either; a bad `space_id` fails only via the column's
/// own `ON DELETE SET NULL` foreign key at insert time (a DB error, not a
/// silent no-op), matching Node exactly.
pub async fn create(pool: &PgPool, user_id: &str, new: NewSavedGraph) -> AppResult<SavedGraph> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        SavedGraph,
        r#"INSERT INTO saved_graph (id, name, description, chunk_ids, positions, layout_algorithm, user_id, space_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, name, description,
                     chunk_ids AS "chunk_ids: Json<Vec<String>>",
                     positions AS "positions: Json<HashMap<String, Position>>",
                     layout_algorithm, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        new.name,
        new.description,
        Json(&new.chunk_ids) as _,
        Json(&new.positions) as _,
        new.layout_algorithm,
        user_id,
        new.space_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Scoped by `user_id` in SQL — the only thing standing between this and
/// returning another user's saved graph, proven in
/// `tests/saved_graph.rs::find_by_id_is_user_scoped`.
pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<SavedGraph>> {
    let row = sqlx::query_as!(
        SavedGraph,
        r#"SELECT id, name, description,
                  chunk_ids AS "chunk_ids: Json<Vec<String>>",
                  positions AS "positions: Json<HashMap<String, Position>>",
                  layout_algorithm, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM saved_graph WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Lists a user's saved graphs, optionally narrowed to one space. Node's
/// `listSavedGraphs` (`packages/db/src/repository/saved-graph.ts:48-57`)
/// treats `spaceId` as truthy-checked (`if (spaceId) ...`), so an empty
/// string behaves as "no filter" there — reproduced here by filtering it
/// out before binding.
///
/// `ORDER BY created_at DESC, id ASC`: Node's query has **no** `ORDER BY`
/// at all (`db.select().from(savedGraph).where(...)`, no `.orderBy()`), so
/// row order is whatever Postgres's query planner happens to return —
/// unstable across runs. This port adds a total order with an `id`
/// tiebreaker (same convention as `saved_query::list`, `notification::list`,
/// `chunk::list`), proven stable in
/// `tests/saved_graph.rs::list_breaks_created_at_ties_by_id`.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<SavedGraph>> {
    let space_id = space_id.filter(|s| !s.is_empty());
    let rows = sqlx::query_as!(
        SavedGraph,
        r#"SELECT id, name, description,
                  chunk_ids AS "chunk_ids: Json<Vec<String>>",
                  positions AS "positions: Json<HashMap<String, Position>>",
                  layout_algorithm, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM saved_graph
           WHERE user_id = $1 AND ($2::text IS NULL OR space_id = $2)
           ORDER BY created_at DESC, id ASC"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `description` is tri-state (`None` = leave untouched, `Some(None)` =
/// clear, `Some(Some(v))` = set), matching Node's
/// `UpdateSavedGraphParams.description?: string | null` and the repo's own
/// `if (params.description !== undefined) setClause.description = ...`
/// (`packages/db/src/repository/saved-graph.ts:19-25,61-66`). Every other
/// field is plain two-state.
#[derive(Default)]
pub struct SavedGraphPatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub chunk_ids: Option<Vec<String>>,
    pub positions: Option<HashMap<String, Position>>,
    pub layout_algorithm: Option<String>,
}

/// Mirrors Node's `updateSavedGraph`
/// (`packages/db/src/repository/saved-graph.ts:59-83`) exactly: when the
/// patch has no fields set at all, Node skips the `UPDATE` entirely and
/// re-selects the row unchanged (`updated_at` does NOT bump) — the same
/// no-op shape as `workspace::update`, and unlike `collection::update`
/// (whose Drizzle `$onUpdate` hook fires even on an empty `.set(...)`).
/// Proven in `tests/saved_graph.rs::update_with_no_fields_does_not_touch_updated_at`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: SavedGraphPatch,
) -> AppResult<Option<SavedGraph>> {
    let (desc_set, desc_val) = match patch.description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let has_changes = patch.name.is_some()
        || desc_set
        || patch.chunk_ids.is_some()
        || patch.positions.is_some()
        || patch.layout_algorithm.is_some();

    let row = if has_changes {
        sqlx::query_as!(
            SavedGraph,
            r#"UPDATE saved_graph SET
                 name = COALESCE($3, name),
                 description = CASE WHEN $4::bool THEN $5::text ELSE description END,
                 chunk_ids = COALESCE($6, chunk_ids),
                 positions = COALESCE($7, positions),
                 layout_algorithm = COALESCE($8, layout_algorithm),
                 updated_at = now()
               WHERE id = $1 AND user_id = $2
               RETURNING id, name, description,
                         chunk_ids AS "chunk_ids: Json<Vec<String>>",
                         positions AS "positions: Json<HashMap<String, Position>>",
                         layout_algorithm, user_id, space_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp""#,
            id,
            user_id,
            patch.name,
            desc_set,
            desc_val,
            patch.chunk_ids.map(Json) as _,
            patch.positions.map(Json) as _,
            patch.layout_algorithm
        )
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as!(
            SavedGraph,
            r#"SELECT id, name, description,
                      chunk_ids AS "chunk_ids: Json<Vec<String>>",
                      positions AS "positions: Json<HashMap<String, Position>>",
                      layout_algorithm, user_id, space_id,
                      created_at AS "created_at: UtcTimestamp",
                      updated_at AS "updated_at: UtcTimestamp"
               FROM saved_graph WHERE id = $1 AND user_id = $2"#,
            id,
            user_id
        )
        .fetch_optional(pool)
        .await?
    };
    Ok(row)
}

/// Deletes a saved graph, scoped by `user_id` in SQL — the only thing
/// standing between this and deleting another user's row, proven in
/// `tests/saved_graph.rs::delete_removes_row_and_is_user_scoped`.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM saved_graph WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
