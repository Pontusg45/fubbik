//! `chunk_comment` — discussion threads on a chunk.
//!
//! # Two different owners
//!
//! This table has a `user_id`, but it is the **comment author's**, not the
//! authority for reading the thread. A thread on your chunk may contain other
//! people's comments, so:
//!
//! - **reading** a thread is gated on owning the *chunk* (join through
//!   `chunk.user_id`);
//! - **editing or deleting** a single comment is gated on having *written it*
//!   (`chunk_comment.user_id`).
//!
//! Node's `listComments` originally took only a `chunkId` and checked
//! neither, so `GET /chunks/{id}/comments` returned any chunk's discussion to
//! any authenticated caller — fixed in `bdfa534` and reproduced correctly
//! here from the start.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkComment {
    pub id: String,
    pub chunk_id: String,
    /// The author. See the module doc — this is not who may read the thread.
    pub user_id: String,
    pub content: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// The thread on a chunk, oldest first.
///
/// `, id` breaks ties on `created_at`: comments posted in the same
/// millisecond (a script, a burst) would otherwise come back in whatever
/// order the plan produced. Node orders by `created_at` alone.
pub async fn list(pool: &PgPool, chunk_id: &str, user_id: &str) -> AppResult<Vec<ChunkComment>> {
    let rows = sqlx::query_as!(
        ChunkComment,
        r#"SELECT cc.id, cc.chunk_id, cc.user_id, cc.content,
                  cc.created_at AS "created_at: UtcTimestamp",
                  cc.updated_at AS "updated_at: UtcTimestamp"
           FROM chunk_comment cc
           WHERE cc.chunk_id = $1
             AND EXISTS (SELECT 1 FROM chunk c
                         WHERE c.id = $1 AND c.user_id = $2)
           ORDER BY cc.created_at, cc.id"#,
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Posts a comment. Guarded on the chunk being the caller's — Node's
/// `createComment` is a bare insert whose only check is the FK, so it would
/// accept a comment on any chunk that exists.
///
/// Returns `Ok(None)` when the chunk is not the caller's.
pub async fn create(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
    content: &str,
) -> AppResult<Option<ChunkComment>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        ChunkComment,
        r#"INSERT INTO chunk_comment (id, chunk_id, user_id, content)
           SELECT $1, $2, $3, $4
           WHERE EXISTS (SELECT 1 FROM chunk c WHERE c.id = $2 AND c.user_id = $3)
           RETURNING id, chunk_id, user_id, content,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        chunk_id,
        user_id,
        content
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Edits a comment the caller **wrote**. `updated_at` is bumped explicitly:
/// Node relies on Drizzle's `$onUpdate` hook, which lives in the ORM and has
/// no database-level equivalent, so a Rust UPDATE that did not set it would
/// silently leave the column at its creation time.
pub async fn update(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    content: &str,
) -> AppResult<Option<ChunkComment>> {
    let row = sqlx::query_as!(
        ChunkComment,
        r#"UPDATE chunk_comment SET content = $3, updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, chunk_id, user_id, content,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        content
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Deletes a comment the caller **wrote**.
///
/// Note this means the owner of a chunk cannot delete someone else's comment
/// on it — faithful to Node, and arguably wrong for a moderation story, but
/// changing it is a product decision rather than a port decision.
pub async fn delete(pool: &PgPool, id: &str, user_id: &str) -> AppResult<Option<ChunkComment>> {
    let row = sqlx::query_as!(
        ChunkComment,
        r#"DELETE FROM chunk_comment WHERE id = $1 AND user_id = $2
           RETURNING id, chunk_id, user_id, content,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Scoped through the chunk's owner, like [`list`]. Node's `getCommentCount`
/// takes only a `chunkId`, so it leaks how much discussion sits on any chunk.
pub async fn count(pool: &PgPool, chunk_id: &str, user_id: &str) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_comment cc
           WHERE cc.chunk_id = $1
             AND EXISTS (SELECT 1 FROM chunk c
                         WHERE c.id = $1 AND c.user_id = $2)"#,
        chunk_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}
