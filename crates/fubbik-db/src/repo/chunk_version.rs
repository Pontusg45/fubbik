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
/// SELECT, so two concurrent snapshots cannot both compute the same value.
/// The aggregate over an empty set yields NULL, which COALESCE turns into
/// the first version, 1.
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

pub async fn list_for_chunk(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<ChunkVersion>> {
    let rows = sqlx::query_as!(
        ChunkVersion,
        r#"SELECT id, chunk_id, version, title, content, type AS chunk_type,
                  rationale, consequences, created_at
           FROM chunk_version WHERE chunk_id = $1 ORDER BY version DESC"#,
        chunk_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
