use chrono::NaiveDateTime;
use fubbik_core::error::AppResult;
use sqlx::PgPool;

use super::chunk::Chunk;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkVersion {
    pub id: String,
    pub chunk_id: String,
    pub version: i32,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub created_at: NaiveDateTime,
}

/// Appends the chunk's current state to its history. Call before applying
/// an update so the snapshot captures the pre-edit version.
///
/// The version number is derived inside the INSERT rather than by a prior
/// SELECT, avoiding a client-side read-then-write gap. That alone is NOT
/// atomic against a second concurrent transaction: under READ COMMITTED
/// (the default, and there is no explicit transaction here), two
/// overlapping `snapshot()` calls for the same chunk can each see the same
/// prior `MAX(version)` and attempt to insert the same value. The
/// `UNIQUE (chunk_id, version)` constraint (migration 0003) is what
/// actually prevents the duplicate: the second insert fails with a unique
/// violation rather than silently corrupting history. Retrying on that
/// conflict is deferred to a later phase. The aggregate over an empty set
/// yields NULL, which COALESCE turns into the first version, 1.
///
/// `tags` is written as an empty array because the tag domain does not
/// exist in Phase 1. Phase 2 must revisit this when tags land, or version
/// history will record every chunk as untagged.
pub async fn snapshot(pool: &PgPool, current: &Chunk) -> AppResult<()> {
    let id = crate::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_version
             (id, chunk_id, version, title, content, type, tags,
              rationale, consequences, created_at)
           SELECT $1, $2, COALESCE(MAX(v.version), 0) + 1, $3, $4, $5,
                  '[]'::jsonb, $6, $7, now()
           FROM chunk_version v WHERE v.chunk_id = $2"#,
        id,
        current.id,
        current.title,
        current.content,
        current.chunk_type,
        current.rationale,
        current.consequences
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Scoped by `user_id` in the SQL itself (via the parent `chunk` row), not
/// just by the caller having already checked ownership via `service::get`.
/// A `chunk_id`/`user_id` pair that does not correspond to an owned chunk
/// yields an empty history, never another user's rows.
pub async fn list_for_chunk(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
) -> AppResult<Vec<ChunkVersion>> {
    let rows = sqlx::query_as!(
        ChunkVersion,
        r#"SELECT id, chunk_id, version, title, content, type AS chunk_type,
                  rationale, consequences, created_at
           FROM chunk_version
           WHERE chunk_id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)
           ORDER BY version DESC"#,
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
