//! `learning_path` — an ordered reading list of chunks.
//!
//! The membership is a plain `jsonb` array of chunk ids on the row, not a
//! join table, so **order is the data** and there is no FK: a chunk deleted
//! after being added leaves a dangling id in the list. That is Node's design
//! and this port keeps it; see [`update`] for the one consequence worth
//! knowing.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningPath {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    /// Ordered chunk ids. `NOT NULL DEFAULT '[]'`, so an empty path reads
    /// back as `[]` rather than `null`.
    #[schema(value_type = Vec<String>)]
    pub chunk_ids: Json<Vec<String>>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Most recently updated first. `, id` is this port's tiebreaker — Node
/// orders by `updated_at` alone, and paths created in one batch share it.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<LearningPath>> {
    let rows = sqlx::query_as!(
        LearningPath,
        r#"SELECT id, title, description,
                  chunk_ids AS "chunk_ids: Json<Vec<String>>",
                  user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM learning_path WHERE user_id = $1
           ORDER BY updated_at DESC, id"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_by_id(pool: &PgPool, id: &str, user_id: &str) -> AppResult<Option<LearningPath>> {
    let row = sqlx::query_as!(
        LearningPath,
        r#"SELECT id, title, description,
                  chunk_ids AS "chunk_ids: Json<Vec<String>>",
                  user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM learning_path WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub struct NewLearningPath {
    pub title: String,
    pub description: Option<String>,
    pub chunk_ids: Vec<String>,
}

/// Every id in `chunk_ids` must belong to `user_id` — the whole list is
/// rejected if any does not, and `Ok(None)` is returned.
///
/// **Divergence.** Node's `createLearningPath` is a bare insert into a `jsonb`
/// column, so it will happily store another user's chunk ids, or ids of
/// chunks that do not exist. There is no FK to catch it because the
/// membership is not a join table. Checking here is the only place it can be
/// checked at all.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    new: NewLearningPath,
) -> AppResult<Option<LearningPath>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        LearningPath,
        r#"INSERT INTO learning_path (id, title, description, chunk_ids, user_id)
           SELECT $1, $2, $3, $4, $5
           WHERE NOT EXISTS (
             SELECT 1 FROM unnest($6::text[]) AS wanted(id)
             WHERE NOT EXISTS (
               SELECT 1 FROM chunk c WHERE c.id = wanted.id AND c.user_id = $5
             )
           )
           RETURNING id, title, description,
                     chunk_ids AS "chunk_ids: Json<Vec<String>>",
                     user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        new.title,
        new.description,
        Json(&new.chunk_ids) as _,
        user_id,
        &new.chunk_ids[..]
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `description` is two-state here, not tri-state: Node's PATCH body types it
/// `t.Optional(t.String())` with no null variant, so there is no way to clear
/// it once set. Reproduced — adding a clear would be a new capability, not a
/// port.
#[derive(Default)]
pub struct LearningPathPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub chunk_ids: Option<Vec<String>>,
}

/// Bumps `updated_at` explicitly — Node relies on Drizzle's `$onUpdate`,
/// which has no database-level equivalent, and the list is ordered by that
/// column, so omitting it would leave edited paths sorting as if untouched.
///
/// `chunk_ids` is validated exactly as in [`create`].
pub async fn update(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    patch: LearningPathPatch,
) -> AppResult<Option<LearningPath>> {
    // `is_some()`, NOT `!is_empty()`: `chunkIds: []` means "empty this path",
    // which is distinct from omitting the key. Node's `.set(params)` spreads
    // only present keys and so makes the same distinction; keying off
    // emptiness would silently turn "remove every chunk" into a no-op while
    // every add-a-chunk test still passed.
    let touches_ids = patch.chunk_ids.is_some();
    let ids = patch.chunk_ids.unwrap_or_default();
    let row = sqlx::query_as!(
        LearningPath,
        r#"UPDATE learning_path SET
             title = COALESCE($3, title),
             description = COALESCE($4, description),
             chunk_ids = CASE WHEN $5 THEN $6 ELSE chunk_ids END,
             updated_at = now()
           WHERE id = $1 AND user_id = $2
             AND (NOT $5 OR NOT EXISTS (
               SELECT 1 FROM unnest($7::text[]) AS wanted(id)
               WHERE NOT EXISTS (
                 SELECT 1 FROM chunk c WHERE c.id = wanted.id AND c.user_id = $2
               )
             ))
           RETURNING id, title, description,
                     chunk_ids AS "chunk_ids: Json<Vec<String>>",
                     user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.title,
        patch.description,
        touches_ids,
        Json(&ids) as _,
        &ids[..]
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: &str, user_id: &str) -> AppResult<bool> {
    let n = sqlx::query!(
        "DELETE FROM learning_path WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n > 0)
}
