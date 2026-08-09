//! `saved_query` — a user's stored search queries (`packages/db/src/schema/
//! saved-query.ts:7-20`). The row carries its own `user_id`; ownership is
//! not derived from anything else (unlike, say, `favorite`'s ownership via
//! the `chunk` it points at). `query` is opaque JSONB — never re-validated
//! on read, matching Node's `createSavedQuery`/`listSavedQueries`, which
//! store and return whatever shape the caller sent without inspecting it.
//! There is no unique constraint on `(user_id, name)` — see [`create`].

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    #[schema(value_type = serde_json::Value)]
    pub query: Json<serde_json::Value>,
    pub user_id: String,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Lists a user's saved queries, optionally narrowed to one space.
///
/// `ORDER BY created_at DESC, id ASC` is a total order: `created_at` is not
/// unique (Node's `listSavedQueries` orders by `desc(createdAt)` alone,
/// with no tiebreaker at all — see the module doc on `differential.rs` for
/// why this port adds one anyway), so `id ASC` is the load-bearing
/// tiebreaker across ties — proven in
/// `tests/saved_query.rs::list_breaks_created_at_ties_by_id`.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<SavedQuery>> {
    let rows = sqlx::query_as!(
        SavedQuery,
        r#"SELECT id, name, query AS "query: Json<serde_json::Value>", user_id, space_id,
                  created_at AS "created_at: UtcTimestamp"
           FROM saved_query
           WHERE user_id = $1 AND ($2::text IS NULL OR space_id = $2)
           ORDER BY created_at DESC, id ASC"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Inserts a saved query. No uniqueness check on `(user_id, name)` — Node's
/// schema carries no such constraint (`saved-query.ts:7-20` only indexes
/// `user_id`), so duplicate names are allowed by design, not by omission —
/// proven in `tests/saved_query.rs::duplicate_names_are_allowed`.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    query: serde_json::Value,
    space_id: Option<&str>,
) -> AppResult<SavedQuery> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        SavedQuery,
        r#"INSERT INTO saved_query (id, name, query, user_id, space_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, name, query AS "query: Json<serde_json::Value>", user_id, space_id,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        name,
        query,
        user_id,
        space_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Deletes a saved query, scoped by `user_id` in SQL. The route built on
/// top of this (`DELETE /api/search/saved/{id}`) ignores whether a row was
/// actually removed and always answers `{"message":"Deleted"}` — matching
/// Node's `deleteSavedQuery`, whose result is likewise discarded by
/// `packages/api/src/search/routes.ts:105-112` — so this returns `()`, not
/// a `bool`: there is nothing the caller does differently either way. The
/// `user_id = $2` predicate is still load-bearing even though the response
/// can't reveal it: it's the only thing standing between this and deleting
/// another user's row, proven in
/// `tests/saved_query.rs::delete_is_user_scoped_and_leaves_the_victims_row_intact`
/// (a surviving-row assertion, since the never-404 response is
/// indistinguishable either way).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM saved_query WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
