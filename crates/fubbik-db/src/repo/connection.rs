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
    let res = sqlx::query!(
        r#"DELETE FROM chunk_connection c
           WHERE c.id = $1
             AND (
               EXISTS (SELECT 1 FROM chunk s WHERE s.id = c.source_id AND s.user_id = $2)
               OR EXISTS (SELECT 1 FROM chunk t WHERE t.id = c.target_id AND t.user_id = $2)
             )"#,
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
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
