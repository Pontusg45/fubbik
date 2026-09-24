use fubbik_core::error::AppResult;
use sqlx::{PgConnection, PgPool};

use sqlx::types::Json;

use super::chunk::Chunk;
use crate::timestamp::UtcTimestamp;

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
    /// Nullable, unlike `chunk.alternatives`'s sibling on the live row —
    /// `chunk_version.alternatives` is `jsonb` with no `NOT NULL` and no
    /// default (`0001_init.sql:312`), so a snapshot of a chunk that had
    /// none records `null`, not `[]`.
    #[schema(value_type = Option<Vec<String>>)]
    pub alternatives: Option<Json<Vec<String>>>,
    pub consequences: Option<String>,
    #[schema(value_type = Option<std::collections::HashMap<String, String>>)]
    pub scope: Option<Json<serde_json::Value>>,
    /// Free-text label the caller may attach to a write ("feature-x"), so a
    /// run of related edits can be found together later. Set from the
    /// `updateTag` field on chunk create/update.
    pub update_tag: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TaggedChunkVersion {
    pub version_id: String,
    pub chunk_id: String,
    pub version: i32,
    pub update_tag: Option<String>,
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
    pub alternatives: Option<Json<Vec<String>>>,
    pub consequences: Option<String>,
    pub scope: Option<Json<serde_json::Value>>,
    pub created_at: UtcTimestamp,
    pub chunk_title: String,
    pub chunk_content: String,
    pub current_type: String,
    pub chunk_rationale: Option<String>,
    pub chunk_alternatives: Option<Json<Vec<String>>>,
    pub chunk_consequences: Option<String>,
    pub chunk_scope: Json<serde_json::Value>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct UpdateTagCount {
    pub tag: String,
    pub count: i64,
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
/// `tags` is written as an empty array — **matching Node**, which passes
/// `tags: []` at both of its call sites (`chunk-mutations.ts:159,171`)
/// despite the tag domain existing there. The Phase 1 comment this replaces
/// read as a known gap to close later; it is actually parity. Version
/// history genuinely records every chunk as untagged, on both stacks.
///
/// `alternatives`, `scope` and `update_tag` were NOT written until the
/// chunk write-surface port, though `chunk_version` has had all three
/// columns since `0001_init.sql:312-315` and Node writes the first two on
/// every update. `GET /api/chunks/{id}/history` therefore served `null` for
/// them regardless of what the chunk held — the same
/// column-missing-from-the-projection bug the applies-to/file-refs
/// sub-resources had.
pub async fn snapshot(pool: &PgPool, current: &Chunk, update_tag: Option<&str>) -> AppResult<()> {
    let mut connection = pool.acquire().await?;
    snapshot_in(&mut connection, current, update_tag).await
}

/// Transaction-aware form used by chunk aggregate writers.
pub async fn snapshot_in(
    connection: &mut PgConnection,
    current: &Chunk,
    update_tag: Option<&str>,
) -> AppResult<()> {
    let id = crate::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_version
             (id, chunk_id, version, title, content, type, tags,
              rationale, alternatives, consequences, scope, update_tag,
              created_at)
           SELECT $1, $2, COALESCE(MAX(v.version), 0) + 1, $3, $4, $5,
                  '[]'::jsonb, $6, $7, $8, $9, $10, now()
           FROM chunk_version v WHERE v.chunk_id = $2"#,
        id,
        current.id,
        current.title,
        current.content,
        current.chunk_type,
        current.rationale,
        current.alternatives.as_ref().map(|a| Json(&a.0)) as _,
        current.consequences,
        Some(Json(&current.scope.0)) as _,
        update_tag
    )
    .execute(&mut *connection)
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
                  rationale,
                  alternatives AS "alternatives: Json<Vec<String>>",
                  consequences,
                  scope AS "scope: Json<serde_json::Value>",
                  update_tag,
                  created_at AS "created_at: UtcTimestamp"
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

pub async fn list_by_update_tag(
    pool: &PgPool,
    user_id: &str,
    tag: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<TaggedChunkVersion>> {
    Ok(sqlx::query_as::<_, TaggedChunkVersion>(
        r#"SELECT v.id AS version_id, v.chunk_id, v.version, v.update_tag,
                  v.title, v.content, v.type AS chunk_type, v.rationale,
                  v.alternatives, v.consequences, v.scope, v.created_at,
                  c.title AS chunk_title, c.content AS chunk_content,
                  c.type AS current_type, c.rationale AS chunk_rationale,
                  c.alternatives AS chunk_alternatives,
                  c.consequences AS chunk_consequences, c.scope AS chunk_scope
           FROM chunk_version v
           JOIN chunk c ON c.id = v.chunk_id
           WHERE v.update_tag = $1 AND c.user_id = $2
             AND ($3::text IS NULL OR EXISTS (
                 SELECT 1 FROM chunk_space cs
                 WHERE cs.chunk_id = v.chunk_id AND cs.space_id = $3))
           ORDER BY v.created_at DESC"#,
    )
    .bind(tag)
    .bind(user_id)
    .bind(space_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_update_tags(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<UpdateTagCount>> {
    Ok(sqlx::query_as::<_, UpdateTagCount>(
        r#"SELECT v.update_tag AS tag, count(*)::bigint AS count
           FROM chunk_version v
           JOIN chunk c ON c.id = v.chunk_id
           WHERE v.update_tag IS NOT NULL AND c.user_id = $1
             AND ($2::text IS NULL OR EXISTS (
                 SELECT 1 FROM chunk_space cs
                 WHERE cs.chunk_id = v.chunk_id AND cs.space_id = $2))
           GROUP BY v.update_tag
           ORDER BY v.update_tag"#,
    )
    .bind(user_id)
    .bind(space_id)
    .fetch_all(pool)
    .await?)
}
