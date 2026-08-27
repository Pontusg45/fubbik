//! Ports `packages/db/src/repository/similarity.ts`'s
//! `findSimilarByEmbedding`. `findDuplicatePairs` and
//! `findDuplicatePairsWithGraphSignal` are NOT ported here — their only
//! caller is the staleness duplicate scan, which is out of this slice.
use fubbik_core::error::AppResult;
use sqlx::PgPool;

use super::semantic::to_pgvector_text;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimilarChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub similarity: f64,
}

/// The threshold is applied after `LIMIT`, mirroring Node's two-step shape
/// (`similarity.ts:78-81`): order and limit in SQL, then filter by
/// threshold. For this query the ordering of these two steps is cosmetic,
/// not behavioural — `similarity` and the `ORDER BY` distance key are the
/// same monotonic function of cosine distance, so filtering in `WHERE`
/// instead would produce an identical result set for any input. The
/// Rust-side filter exists so this function's shape matches
/// `similarity.ts` line-for-line, not because moving it into SQL would
/// change behaviour.
pub async fn find_similar_by_embedding(
    pool: &PgPool,
    embedding: &[f32],
    user_id: &str,
    exclude_id: Option<&str>,
    threshold: f64,
    limit: i64,
) -> AppResult<Vec<SimilarChunk>> {
    let vector = to_pgvector_text(embedding);

    let rows = sqlx::query_as!(
        SimilarChunk,
        r#"
        SELECT
            c.id, c.title,
            c.type AS chunk_type,
            (1 - (c.embedding <=> $1::text::vector))::float8 AS "similarity!"
        FROM chunk c
        WHERE c.user_id = $2
          AND c.embedding IS NOT NULL
          AND ($3::text IS NULL OR c.id <> $3)
        ORDER BY c.embedding <=> $1::text::vector
        LIMIT $4
        "#,
        vector,
        user_id,
        exclude_id,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter(|r| r.similarity >= threshold)
        .collect())
}
