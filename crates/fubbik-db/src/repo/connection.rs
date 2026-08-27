use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `chunk_connection` row shape, matching Node's bare-row response for both
/// `POST /api/connections` (create) and the implicit shape returned by
/// `getConnectionById`/`deleteConnection` in
/// `packages/db/src/repository/connection.ts`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    pub origin: String,
    pub review_status: String,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<UtcTimestamp>,
    pub weight: i32,
}

/// Inserts a `chunk_connection` row, but only if **both** `source_id` and
/// `target_id` resolve to a `chunk` owned by `user_id` — verified in SQL,
/// not pre-checked in application code and then trusted. `s.user_id = $5
/// AND t.user_id = $5` are both load-bearing: dropping either one lets a
/// caller wire their own chunk to (or from) someone else's, which would
/// both leak that chunk's existence and create a cross-tenant edge. See
/// `tests/connection.rs` for the two independent proofs (foreign source,
/// foreign target) — same "each direction gets its own test" shape as
/// `chunk_tag`/`chunk_space`, except here ownership of *both* ends of a
/// single row must hold simultaneously, not one of two parent rows.
///
/// Returns `Ok(None)` (not an error) when the ownership guard rejects the
/// pair — the `FROM chunk s, chunk t WHERE ...` join simply matches no
/// rows, so the `INSERT ... SELECT` inserts nothing and `RETURNING`
/// produces nothing. Callers distinguish this from the two real error
/// paths this can also take, both left to propagate via `?` and matched at
/// the call site the same way `auth::routes::sign_up` matches
/// `user::create`'s unique-violation race:
///
/// - a duplicate `(source_id, target_id, relation)` triggers
///   `connection_unique_idx` → `db_err.is_unique_violation()` → 409, not a
///   raw 500;
/// - an unrecognized `relation` violates the FK into `connection_relation`
///   → `db_err.is_foreign_key_violation()` → 400, not a raw 500.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    source_id: &str,
    target_id: &str,
    relation: &str,
    origin: &str,
    review_status: &str,
) -> AppResult<Option<Connection>> {
    let row = sqlx::query_as!(
        Connection,
        r#"INSERT INTO chunk_connection (id, source_id, target_id, relation, origin, review_status)
           SELECT $1, s.id, t.id, $4, $6, $7
           FROM chunk s, chunk t
           WHERE s.id = $2 AND t.id = $3 AND s.user_id = $5 AND t.user_id = $5
           RETURNING id, source_id, target_id, relation,
                     created_at AS "created_at: UtcTimestamp",
                     origin, review_status, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp",
                     weight"#,
        id,
        source_id,
        target_id,
        relation,
        user_id,
        origin,
        review_status
    )
    .fetch_optional(pool)
    .await?;

    // Project into AGE *after* the SQL insert has already succeeded,
    // mirroring Node's ordering (`packages/db/src/repository/connection.ts`
    // calls `ensureVertex` twice then `createEdge` only once the row
    // exists). Deliberately NOT rolled back if projection fails: Node's own
    // projection errors don't undo the insert either, so a stricter Rust
    // here would silently diverge — the connection would vanish where Node
    // keeps it. Logged and swallowed instead; the row this function just
    // created is still returned as a success.
    if let Some(conn) = &row {
        if let Err(e) = crate::age::ensure_vertex(pool, &conn.source_id).await {
            tracing::warn!(error = %e, connection_id = %conn.id, chunk_id = %conn.source_id, "failed to project source vertex into AGE graph");
        }
        if let Err(e) = crate::age::ensure_vertex(pool, &conn.target_id).await {
            tracing::warn!(error = %e, connection_id = %conn.id, chunk_id = %conn.target_id, "failed to project target vertex into AGE graph");
        }
        if let Err(e) =
            crate::age::create_edge(pool, &conn.relation, &conn.source_id, &conn.target_id).await
        {
            tracing::warn!(error = %e, connection_id = %conn.id, "failed to project connects edge into AGE graph");
        }
    }

    Ok(row)
}

/// Unscoped by `user_id` — matches Node's `getConnectionById`, which is a
/// plain `id` lookup with no ownership filter of its own. The service
/// layer (`connections::service::delete`) is what turns this into an
/// authorization check, by requiring the connection's `source`/`target` to
/// resolve for the caller before deleting it (see [`delete`] below, which
/// folds that same check into the delete itself rather than doing a
/// separate unscoped lookup first).
pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<Connection>> {
    let row = sqlx::query_as!(
        Connection,
        r#"SELECT id, source_id, target_id, relation,
                  created_at AS "created_at: UtcTimestamp",
                  origin, review_status, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp",
                  weight
           FROM chunk_connection WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Deletes a `chunk_connection` row by id, but only if **at least one** of
/// its two endpoints belongs to `user_id` — an `OR`, not an `AND`, which is
/// the one place this domain's ownership rule differs from every other
/// guard in this slice. This matches Node's `deleteConnection`
/// (`packages/api/src/connections/service.ts:36-51`) exactly: it re-fetches
/// both `source` and `target` scoped to `userId` and requires `source ||
/// target` to resolve — "a connection between two chunks neither owned by
/// the caller is invisible/undeletable, but only one end needs to belong
/// to the caller for the delete to proceed" (see
/// `tests/fixtures/node-contract/_mutating.md`).
///
/// Folded into one SQL statement rather than Node's three round trips
/// (fetch connection, fetch source, fetch target): whether `id` doesn't
/// exist at all, or exists but neither endpoint resolves for `user_id`,
/// both collapse to `rows_affected() == 0` — indistinguishable at this
/// layer, matching Node, which maps both cases to the same 404 `{resource:
/// "Connection"}`.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let deleted = sqlx::query!(
        r#"DELETE FROM chunk_connection c
           WHERE c.id = $1
             AND (
               EXISTS (SELECT 1 FROM chunk s WHERE s.id = c.source_id AND s.user_id = $2)
               OR EXISTS (SELECT 1 FROM chunk t WHERE t.id = c.target_id AND t.user_id = $2)
             )
           RETURNING c.source_id, c.target_id, c.relation"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = deleted else {
        return Ok(false);
    };

    // Remove the projected edge after the row is gone, same "log and
    // swallow, never roll back the SQL write" stance as `create` above.
    if let Err(e) =
        crate::age::delete_edge(pool, &row.relation, &row.source_id, &row.target_id).await
    {
        tracing::warn!(error = %e, connection_id = %id, "failed to remove projected AGE edge for deleted connection");
    }

    Ok(true)
}

/// One `(chunkId, count)` pair — the bulk shape `search::service` uses to
/// enrich a page of search results with connection counts in one query
/// instead of Node's `getChunkConnections(chunkId).length` called once per
/// result row.
#[derive(Debug, Clone)]
pub struct ChunkConnectionCount {
    pub chunk_id: String,
    pub count: i64,
}

/// Counts connections touching each of `chunk_ids` (either as `source_id`
/// or `target_id` — matching `getChunkConnections`'s `WHERE source_id = ..
/// OR target_id = ..`, which counts a self-referencing edge, if one ever
/// existed, once — not twice). Ids with zero connections still appear in
/// the result with `count = 0`, via `unnest($1) LEFT JOIN`, so callers
/// don't need to treat "missing from the result" as "zero" themselves. Not
/// scoped by `user_id`: the caller is expected to have already proven
/// ownership of every id in `chunk_ids` (the same trust boundary
/// `chunk::push_filters`'s `tags` branch documents), and counting an edge
/// touching a foreign chunk can't leak anything beyond a number.
pub async fn count_for_chunks(
    pool: &PgPool,
    chunk_ids: &[String],
) -> AppResult<Vec<ChunkConnectionCount>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as!(
        ChunkConnectionCount,
        r#"SELECT c.id AS "chunk_id!", COUNT(cc.id) AS "count!"
           FROM unnest($1::text[]) AS c(id)
           LEFT JOIN chunk_connection cc
             ON cc.source_id = c.id OR cc.target_id = c.id
           GROUP BY c.id"#,
        chunk_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One row of `GET /api/chunks/{id}`'s `connections` array, matching Node's
/// `getChunkConnections` projection exactly
/// (`packages/db/src/repository/chunk.ts:259-282`): the edge's own
/// `id`/`sourceId`/`targetId`/`relation`, plus the **other** end's title and
/// one of that chunk's space names.
///
/// `codebase_name` keeps the pre-rename wire key `codebaseName` because
/// that is the alias Node's `.select({ codebaseName: space.name })` emits
/// and the web app reads. The `codebase → space` rename never reached this
/// projection.
///
/// Both `title` and `codebase_name` are `Option` because both joins are
/// LEFT joins in Node: a dangling edge (target chunk deleted) yields a null
/// title, and a chunk in no space yields a null space name.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkConnectionDetail {
    pub id: String,
    pub target_id: String,
    pub source_id: String,
    pub relation: String,
    pub title: Option<String>,
    pub codebase_name: Option<String>,
}

/// Every connection touching `chunk_id`, in either direction, joined to the
/// other end's title and space name.
///
/// **The `chunk_space` LEFT JOIN multiplies rows.** A connected chunk that
/// belongs to N spaces produces N rows for the same edge, each with a
/// different `codebaseName`; a chunk in zero spaces produces one row with a
/// null one. That is Node's behaviour verbatim
/// (`packages/db/src/repository/chunk.ts:271-280` joins `chunk_space` and
/// `space` without any aggregation or `DISTINCT`), and it is load-bearing
/// downstream: `getChunkDetail` feeds `connections.length` straight into
/// `computeHealthScore`'s `connectionCount`, so a multi-space neighbour
/// inflates the connectivity score as well as duplicating a row in the
/// detail page's connection list. Reproduced rather than fixed — parity is
/// the job — and pinned by
/// `tests/connection.rs::connections_for_chunk_multiplies_rows_per_space`
/// so any future move to `DISTINCT`/aggregation is a deliberate, visible
/// change on both stacks rather than a silent divergence.
///
/// Scoped through the *subject* chunk's owner in SQL — Node's version takes
/// only a `chunkId` and relies on `getChunkDetail` having already loaded
/// the chunk under `userId` first. Same "through the parent" hardening
/// applied to `feature::deltas_for_chunk` and `tag::tags_for_chunk`. The
/// *other* end of an edge may still be a chunk this user does not own; that
/// is inherent to connections being global (cross-space, cross-user
/// linking is the feature), not something this guard is meant to prevent.
///
/// **No `ORDER BY`**, matching Node exactly — callers that need a stable
/// order must sort.
pub async fn connections_for_chunk(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
) -> AppResult<Vec<ChunkConnectionDetail>> {
    let rows = sqlx::query_as!(
        ChunkConnectionDetail,
        r#"SELECT cc.id, cc.target_id, cc.source_id, cc.relation,
                  ch.title AS "title?", s.name AS "codebase_name?"
           FROM chunk_connection cc
           LEFT JOIN chunk ch
             ON (cc.target_id = ch.id AND cc.source_id = $1)
             OR (cc.source_id = ch.id AND cc.target_id = $1)
           LEFT JOIN chunk_space cs ON cs.chunk_id = ch.id
           LEFT JOIN space s ON s.id = cs.space_id
           WHERE (cc.source_id = $1 OR cc.target_id = $1)
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)"#,
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every `chunk_connection` row touching any of `chunk_ids` on either end,
/// globally — matching Node's `getConnectionsForChunks`
/// (`packages/db/src/repository/connection.ts:31-37`), a bare
/// `or(inArray(sourceId, chunkIds), inArray(targetId, chunkIds))` with no
/// ownership scoping of its own. Connections are a global entity (cross-space,
/// cross-user linking is the feature — see this module's other doc
/// comments), so this is not a trust-boundary gap: the caller
/// (`context_for_file::service::get_context_for_file`) has already produced
/// `chunk_ids` from its own `user_id`-scoped lookups, and a foreign
/// connection touching one of them can only ever be *read*, never used to
/// reach a foreign chunk's contents through this function alone.
///
/// **No `ORDER BY`**, matching Node exactly.
pub async fn connections_for_chunks(
    pool: &PgPool,
    chunk_ids: &[String],
) -> AppResult<Vec<Connection>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as!(
        Connection,
        r#"SELECT id, source_id, target_id, relation,
                  created_at AS "created_at: UtcTimestamp",
                  origin, review_status, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp",
                  weight
           FROM chunk_connection
           WHERE source_id = ANY($1) OR target_id = ANY($1)"#,
        chunk_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Bumps `weight` by one on every `chunk_connection` row whose **both**
/// ends are in `chunk_ids` — ports `incrementConnectionWeights`
/// (`packages/db/src/repository/connection.ts:89-98`), which fires this as
/// a fire-and-forget side effect after `getContextForFile` returns, to
/// record which chunks tend to be retrieved together. A no-op (returns
/// `Ok(0)` without touching the database) when `chunk_ids` has fewer than
/// two entries, matching Node's own early return — a single co-accessed
/// chunk has no pair to strengthen.
pub async fn increment_connection_weights(pool: &PgPool, chunk_ids: &[String]) -> AppResult<u64> {
    if chunk_ids.len() < 2 {
        return Ok(0);
    }
    let result = sqlx::query!(
        "UPDATE chunk_connection SET weight = weight + 1 \
         WHERE source_id = ANY($1) AND target_id = ANY($1)",
        chunk_ids
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
