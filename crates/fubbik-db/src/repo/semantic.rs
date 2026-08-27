//! Vector reads behind semantic search and `{id}/neighbors`.
//!
//! Ports `packages/db/src/repository/semantic.ts`. Node builds these with
//! Drizzle's dynamic `and(...conditions)`, which cannot be reproduced under
//! `query_as!` without giving up compile-time checking. Each optional
//! filter is instead expressed as a `$n IS NULL OR <predicate>` pair, which
//! the planner short-circuits and which keeps the macro's type inference.
//!
//! `exclude` is passed as a `text[]` rather than N separate parameters
//! because its length is not known at compile time; the predicate is
//! logically Node's per-term loop (`semantic.ts:70-74`) collapsed into one
//! `NOT EXISTS`.
use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticHit {
    pub id: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: String,
    #[schema(value_type = Vec<String>)]
    pub aliases: Json<Vec<String>>,
    #[schema(value_type = std::collections::HashMap<String, String>)]
    pub scope: Json<serde_json::Value>,
    pub similarity: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NeighborRow {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub distance: f64,
}

/// Renders an embedding as pgvector's text literal form (`[f0,f1,...]`).
///
/// `pub(crate)` rather than private: Task 5's `repo/similarity.rs` imports
/// this instead of copying it verbatim.
pub(crate) fn to_pgvector_text(embedding: &[f32]) -> String {
    let joined = embedding
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{joined}]")
}

pub async fn semantic_search(
    pool: &PgPool,
    embedding: &[f32],
    user_id: Option<&str>,
    exclude: &[String],
    scope: Option<&serde_json::Value>,
    limit: i64,
) -> AppResult<Vec<SemanticHit>> {
    let vector = to_pgvector_text(embedding);

    let rows = sqlx::query_as!(
        SemanticHit,
        r#"
        SELECT
            c.id, c.title, c.content, c.summary,
            c.type AS chunk_type,
            c.aliases AS "aliases: Json<Vec<String>>",
            c.scope AS "scope: Json<serde_json::Value>",
            (1 - (c.embedding <=> $1::text::vector))::float8 AS "similarity!"
        FROM chunk c
        WHERE c.embedding IS NOT NULL
          AND ($2::text IS NULL OR c.user_id = $2)
          AND NOT EXISTS (
                SELECT 1 FROM unnest($3::text[]) AS term
                WHERE c.not_about @> to_jsonb(ARRAY[term])
              )
          AND ($4::jsonb IS NULL OR c.scope @> $4)
        ORDER BY c.embedding <=> $1::text::vector
        LIMIT $5
        "#,
        vector,
        user_id,
        exclude,
        scope,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Node's `findNeighborsByChunkId` (`semantic.ts:24-61`). The source row is
/// a CTE that yields nothing when the chunk has no embedding, so the
/// `FROM chunk c, source` join produces an empty result — that is why the
/// no-embedding case needs no explicit branch here.
///
/// Note the asymmetry with `semantic_search`: this carries
/// `AND c.archived_at IS NULL` (matching `semantic.ts:41`), while
/// `semantic_search`'s Drizzle builder never filters on `archivedAt`. That
/// difference is Node's, not an oversight here — do not "fix" it.
pub async fn find_neighbors_by_chunk_id(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
    k: i64,
) -> AppResult<Vec<NeighborRow>> {
    let rows = sqlx::query_as!(
        NeighborRow,
        r#"
        WITH source AS (
            SELECT embedding FROM chunk
            WHERE id = $1 AND user_id = $2 AND embedding IS NOT NULL
        )
        SELECT
            c.id, c.title, c.summary,
            c.type AS chunk_type,
            (c.embedding <=> (SELECT embedding FROM source))::float8 AS "distance!"
        FROM chunk c, source
        WHERE c.id <> $1
          AND c.user_id = $2
          AND c.embedding IS NOT NULL
          AND c.archived_at IS NULL
        ORDER BY c.embedding <=> (SELECT embedding FROM source)
        LIMIT $3
        "#,
        chunk_id,
        user_id,
        k,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
