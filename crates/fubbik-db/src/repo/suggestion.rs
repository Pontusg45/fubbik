//! Read model for connection suggestions.
//!
//! Candidate discovery belongs here rather than in the HTTP service: both
//! strategies are set-based database queries, and keeping their ownership and
//! exclusion predicates next to the SQL makes the trust boundary explicit.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SuggestionCandidate {
    pub id: String,
    pub title: String,
    pub chunk_type: String,
    pub shared_count: Option<i64>,
}

pub async fn sharing_tags(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
    excluded_ids: &[String],
) -> AppResult<Vec<SuggestionCandidate>> {
    let rows = sqlx::query_as::<_, SuggestionCandidate>(
        r#"SELECT candidate.id, candidate.title, candidate.type AS chunk_type,
                  COUNT(*)::bigint AS shared_count
           FROM chunk_tag target_tag
           JOIN chunk_tag candidate_tag ON candidate_tag.tag_id = target_tag.tag_id
           JOIN chunk candidate ON candidate.id = candidate_tag.chunk_id
           WHERE target_tag.chunk_id = $1
             AND candidate.user_id = $2
             AND NOT (candidate.id = ANY($3))
           GROUP BY candidate.id, candidate.title, candidate.type
           ORDER BY COUNT(*) DESC
           LIMIT 5"#,
    )
    .bind(chunk_id)
    .bind(user_id)
    .bind(excluded_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn similar_titles(
    pool: &PgPool,
    title: &str,
    user_id: &str,
    excluded_ids: &[String],
) -> AppResult<Vec<SuggestionCandidate>> {
    let rows = sqlx::query_as::<_, SuggestionCandidate>(
        r#"SELECT id, title, type AS chunk_type, NULL::bigint AS shared_count
           FROM chunk
           WHERE user_id = $1
             AND similarity(title, $2) > 0.15
             AND NOT (id = ANY($3))
           ORDER BY similarity(title, $2) DESC
           LIMIT 5"#,
    )
    .bind(user_id)
    .bind(title)
    .bind(excluded_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
