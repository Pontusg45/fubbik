use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk;
use fubbik_db::repo::favorite::{self, Favorite};
use sqlx::PgPool;

use super::dto::ReorderEntry;

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Favorite>> {
    favorite::list(pool, user_id).await
}

/// Mirrors Node's `addFavorite` (`packages/api/src/favorites/service.ts:17-31`)
/// exactly, including its two quirks:
///
/// - **404 before insert.** Node checks `getChunkById(chunkId, userId)`
///   first and fails with `NotFoundError({ resource: "Chunk" })` if it
///   doesn't resolve. `favorite::add`'s own `INSERT ... SELECT ... FROM
///   chunk` join would also reject a foreign/missing chunk (returning
///   `Ok(None)`), but that alone can't distinguish "no such chunk" from
///   "already favorited" (see the doc comment on `favorite::add`) — so
///   this check has to happen here, not just in SQL, to reproduce Node's
///   distinct 404 vs. 201-null behavior.
/// - **`order` computed app-side, not DB-generated**, via a read-then-write
///   (list existing favorites, take `max(order) + 1`, or `0` if none). This
///   is the same benign TOCTOU race Node has: concurrent adds can compute
///   the same `next_order` and both succeed, since `"order"` carries no
///   uniqueness constraint of its own — only `(user_id, chunk_id)` does.
///
/// Returns `Ok(None)` — not an error — when the chunk was already
/// favorited: the route sets 201 unconditionally and serializes `None` as
/// JSON `null`, reproducing Node's `created ?? null` flowing straight
/// through as the response body.
pub async fn add(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<Option<Favorite>> {
    if chunk::find_by_id(pool, user_id, chunk_id).await?.is_none() {
        return Err(AppError::NotFound("Chunk".into()));
    }

    let existing = favorite::list(pool, user_id).await?;
    let next_order = existing
        .iter()
        .map(|f| f.order)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);

    let id = fubbik_db::new_id();
    favorite::add(pool, &id, user_id, chunk_id, next_order).await
}

/// Mirrors Node's `removeFavorite`: no existence check, no 404 path — see
/// `favorite::remove`'s doc comment for the full contract citation.
pub async fn remove(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<()> {
    favorite::remove(pool, user_id, chunk_id).await
}

/// Mirrors Node's `reorderFavorites`: a partial, per-entry update with no
/// per-entry ownership error — see `favorite::reorder`'s doc comment.
pub async fn reorder(pool: &PgPool, user_id: &str, entries: Vec<ReorderEntry>) -> AppResult<()> {
    let pairs: Vec<(String, i32)> = entries.into_iter().map(|e| (e.chunk_id, e.order)).collect();
    favorite::reorder(pool, user_id, &pairs).await
}
