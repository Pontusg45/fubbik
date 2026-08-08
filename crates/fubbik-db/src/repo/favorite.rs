use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `user_favorite` row shape. `order` is `"order"` in every SQL statement
/// in this module — it is a SQL reserved word, quoted exactly the way
/// `"user"` is quoted everywhere else in this codebase. Unlike the
/// composite join tables (`chunk_tag`, `chunk_space`), this table carries
/// its own `id` and `user_id`, so listing/removing don't need to go
/// through the parent `chunk` row — but the row it references still
/// belongs to someone, so `add` below does scope through `chunk` in SQL.
///
/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    pub user_id: String,
    pub chunk_id: String,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Lists a user's favorites in display order.
///
/// `ORDER BY "order" ASC, id ASC` is a total order: `"order"` is
/// explicitly *not* unique (two favorites can legitimately share a
/// position — Node's `addFavorite` even has a documented TOCTOU race that
/// can produce exactly that), so `id ASC` is the load-bearing tiebreaker
/// here, not a theoretical one — see
/// `tests/favorite.rs::list_breaks_order_ties_by_id`.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Favorite>> {
    let rows = sqlx::query_as!(
        Favorite,
        r#"SELECT id, user_id, chunk_id, "order", created_at AS "created_at: UtcTimestamp"
           FROM user_favorite
           WHERE user_id = $1
           ORDER BY "order" ASC, id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Inserts a favorite, but only if `chunk_id` resolves to a `chunk` owned
/// by `user_id` — verified in SQL via the `INSERT ... SELECT ... FROM
/// chunk c WHERE c.id = $3 AND c.user_id = $2` join, not pre-checked in
/// application code and then trusted (same shape as
/// `connection::create`'s ownership guard).
///
/// `ON CONFLICT DO NOTHING` reproduces Node's `.onConflictDoNothing()` on
/// `favorite_user_chunk_idx` (unique on `(user_id, chunk_id)`): favoriting
/// an already-favorited chunk is a silent no-op here too, returning
/// `Ok(None)` — not an error. That means `Ok(None)` from this function is
/// ambiguous between "no such chunk for this user" and "already
/// favorited"; the caller (`favorites::service::add`) disambiguates by
/// checking chunk ownership itself first, matching Node's `addFavorite`,
/// which does the same `getChunkById` check before ever attempting the
/// insert.
pub async fn add(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    chunk_id: &str,
    order: i32,
) -> AppResult<Option<Favorite>> {
    let row = sqlx::query_as!(
        Favorite,
        r#"INSERT INTO user_favorite (id, user_id, chunk_id, "order")
           SELECT $1, $2, c.id, $4
           FROM chunk c
           WHERE c.id = $3 AND c.user_id = $2
           ON CONFLICT DO NOTHING
           RETURNING id, user_id, chunk_id, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id,
        chunk_id,
        order
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Removes a favorite. Unconditional `WHERE user_id = $1 AND chunk_id =
/// $2` with no existence check — matches Node's `removeFavorite`, which
/// has no 404 path at the route or service level (`_mutating.md`: "no way
/// to distinguish 'deleted something' from 'there was nothing to delete'
/// from the response"). Returning `()` instead of a `bool` is deliberate:
/// there is nothing the caller does differently based on whether a row
/// existed.
pub async fn remove(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM user_favorite WHERE user_id = $1 AND chunk_id = $2",
        user_id,
        chunk_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Applies a partial reorder: one `UPDATE ... WHERE user_id = $1 AND
/// chunk_id = $2` per entry, inside a single transaction, matching Node's
/// `reorderFavorites` (`packages/db/src/repository/favorite.ts:37-48`).
///
/// Two Node quirks reproduced deliberately, not "fixed":
/// - **Partial is fine.** Entries not present in `entries` keep whatever
///   `"order"` they already had — there is no requirement to mention every
///   favorite the caller has.
/// - **No cross-user check beyond the `WHERE`.** An entry naming a
///   `chunk_id` the caller doesn't have favorited (never favorited, or
///   favorited by someone else) simply updates zero rows, silently — no
///   per-entry error surfaces to the caller.
pub async fn reorder(pool: &PgPool, user_id: &str, entries: &[(String, i32)]) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (chunk_id, order) in entries {
        sqlx::query!(
            r#"UPDATE user_favorite SET "order" = $3 WHERE user_id = $1 AND chunk_id = $2"#,
            user_id,
            chunk_id,
            order
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
