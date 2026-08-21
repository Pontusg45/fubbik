//! The five queries behind `GET /api/health/knowledge` — chunks that need
//! attention, each returned as a capped sample plus a full count.
//!
//! # The space filter means "this space, or no space at all"
//!
//! Every query shares one predicate: with a `space_id`, a chunk qualifies if
//! it is in that space **or** in no space whatsoever. Global chunks are
//! everyone's business, so they show up under every space's health view. This
//! is Node's `spaceConditions` verbatim
//! (`packages/db/src/repository/knowledge-health.ts:9-14`), and it matches
//! `chunk::list`'s `space_id` branch.
//!
//! # Every list is capped, and Node's caps are non-deterministic
//!
//! Node applies `LIMIT 50` (100 for file refs) with **no `ORDER BY` at all**,
//! so which 50 of 200 orphans you see is a query-plan artifact and can change
//! between identical requests. Each query here adds a deterministic order —
//! the most useful one for the panel, with `id` as a tiebreaker. The `count`
//! is always the full total, not the capped length, so the UI can say "50 of
//! 213" honestly either way.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// A capped sample plus the true total.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthBucket<T> {
    pub chunks: Vec<T>,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrphanChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaleChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    /// When this chunk's most recently-touched neighbour was updated — the
    /// signal that the chunk may have fallen behind its context.
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub newest_neighbor_update: Option<UtcTimestamp>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThinChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub content_length: i32,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaleEmbedding {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub embedding_updated_at: Option<UtcTimestamp>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthFileRef {
    pub ref_id: String,
    pub chunk_id: String,
    pub chunk_title: String,
    pub chunk_type: String,
    pub path: String,
    pub relation: String,
}

/// The `refs`/`count` pair — named separately from [`HealthBucket`] because
/// Node calls the array `refs` here and `chunks` everywhere else.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRefBucket {
    pub refs: Vec<HealthFileRef>,
    pub count: i64,
}

/// Chunks with no connections in either direction.
///
/// Ordered newest first: a freshly created orphan is the one most likely to
/// be an oversight worth fixing, and an orphan from two years ago has already
/// been lived with.
pub async fn orphan_chunks(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<HealthBucket<OrphanChunk>> {
    let chunks = sqlx::query_as!(
        OrphanChunk,
        r#"SELECT c.id, c.title, c.type AS chunk_type,
                  c.created_at AS "created_at: UtcTimestamp"
           FROM chunk c
           WHERE c.user_id = $1
             AND NOT EXISTS (SELECT 1 FROM chunk_connection cc
                             WHERE cc.source_id = c.id OR cc.target_id = c.id)
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))
           ORDER BY c.created_at DESC, c.id
           LIMIT 50"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM chunk c
           WHERE c.user_id = $1
             AND NOT EXISTS (SELECT 1 FROM chunk_connection cc
                             WHERE cc.source_id = c.id OR cc.target_id = c.id)
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))"#,
        user_id,
        space_id
    )
    .fetch_one(pool)
    .await?;

    Ok(HealthBucket { chunks, count })
}

/// Chunks untouched for 30+ days whose neighbours HAVE moved in the last 7 —
/// the chunk has fallen behind its context, which is a stronger signal than
/// age alone.
///
/// Ordered by the neighbour's update time, newest first: the chunk whose
/// context moved most recently is the most urgent.
pub async fn stale_chunks(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<HealthBucket<StaleChunk>> {
    let chunks = sqlx::query_as!(
        StaleChunk,
        r#"SELECT c.id, c.title, c.type AS chunk_type,
                  c.updated_at AS "updated_at: UtcTimestamp",
                  (SELECT MAX(n.updated_at)
                   FROM chunk_connection cc
                   JOIN chunk n ON n.id = CASE WHEN cc.source_id = c.id
                                               THEN cc.target_id ELSE cc.source_id END
                   WHERE cc.source_id = c.id OR cc.target_id = c.id)
                  AS "newest_neighbor_update: UtcTimestamp"
           FROM chunk c
           WHERE c.user_id = $1
             AND c.updated_at < NOW() - INTERVAL '30 days'
             AND EXISTS (SELECT 1
                         FROM chunk_connection cc
                         JOIN chunk n ON n.id = CASE WHEN cc.source_id = c.id
                                                     THEN cc.target_id ELSE cc.source_id END
                         WHERE (cc.source_id = c.id OR cc.target_id = c.id)
                           AND n.updated_at > NOW() - INTERVAL '7 days')
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))
           ORDER BY 5 DESC, c.id
           LIMIT 50"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM chunk c
           WHERE c.user_id = $1
             AND c.updated_at < NOW() - INTERVAL '30 days'
             AND EXISTS (SELECT 1
                         FROM chunk_connection cc
                         JOIN chunk n ON n.id = CASE WHEN cc.source_id = c.id
                                                     THEN cc.target_id ELSE cc.source_id END
                         WHERE (cc.source_id = c.id OR cc.target_id = c.id)
                           AND n.updated_at > NOW() - INTERVAL '7 days')
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))"#,
        user_id,
        space_id
    )
    .fetch_one(pool)
    .await?;

    Ok(HealthBucket { chunks, count })
}

/// Chunks with under 100 characters of content. Ordered shortest first — the
/// emptiest is the most obviously unfinished.
pub async fn thin_chunks(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<HealthBucket<ThinChunk>> {
    let chunks = sqlx::query_as!(
        ThinChunk,
        r#"SELECT c.id, c.title, c.type AS chunk_type,
                  LENGTH(c.content)::int AS "content_length!"
           FROM chunk c
           WHERE c.user_id = $1
             AND LENGTH(c.content) < 100
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))
           ORDER BY LENGTH(c.content), c.id
           LIMIT 50"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM chunk c
           WHERE c.user_id = $1
             AND LENGTH(c.content) < 100
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))"#,
        user_id,
        space_id
    )
    .fetch_one(pool)
    .await?;

    Ok(HealthBucket { chunks, count })
}

/// Chunks edited since their embedding was last computed, so semantic search
/// is matching against text that no longer exists.
///
/// `embedding IS NOT NULL` is load-bearing: a chunk that never had an
/// embedding is not *stale*, it is simply unenriched, and belongs to a
/// different problem. Ordered most-recently-edited first.
pub async fn stale_embeddings(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<HealthBucket<StaleEmbedding>> {
    let chunks = sqlx::query_as!(
        StaleEmbedding,
        r#"SELECT c.id, c.title, c.type AS chunk_type,
                  c.updated_at AS "updated_at: UtcTimestamp",
                  c.embedding_updated_at AS "embedding_updated_at: UtcTimestamp"
           FROM chunk c
           WHERE c.user_id = $1
             AND c.embedding IS NOT NULL
             AND c.updated_at > c.embedding_updated_at
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))
           ORDER BY c.updated_at DESC, c.id
           LIMIT 50"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM chunk c
           WHERE c.user_id = $1
             AND c.embedding IS NOT NULL
             AND c.updated_at > c.embedding_updated_at
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))"#,
        user_id,
        space_id
    )
    .fetch_one(pool)
    .await?;

    Ok(HealthBucket { chunks, count })
}

/// Every file reference the user's chunks carry, capped at 100. Ordered by
/// path so the panel groups references to the same file together.
pub async fn file_refs(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<FileRefBucket> {
    let refs = sqlx::query_as!(
        HealthFileRef,
        r#"SELECT fr.id AS ref_id, fr.chunk_id,
                  c.title AS chunk_title, c.type AS chunk_type,
                  fr.path, fr.relation
           FROM chunk_file_ref fr
           JOIN chunk c ON c.id = fr.chunk_id
           WHERE c.user_id = $1
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))
           ORDER BY fr.path, fr.id
           LIMIT 100"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM chunk_file_ref fr
           JOIN chunk c ON c.id = fr.chunk_id
           WHERE c.user_id = $1
             AND ($2::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))"#,
        user_id,
        space_id
    )
    .fetch_one(pool)
    .await?;

    Ok(FileRefBucket { refs, count })
}

/// Backs `GET /api/health`'s `db` field. A trivial round trip — the point is
/// that it goes through the pool, so a exhausted-connection state reports as
/// degraded rather than as healthy.
pub async fn db_reachable(pool: &PgPool) -> bool {
    sqlx::query_scalar!(r#"SELECT 1 AS "one!""#)
        .fetch_one(pool)
        .await
        .is_ok()
}
