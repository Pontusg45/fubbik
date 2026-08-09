//! Deliberately minimal: the `requirement` table has no CRUD surface in
//! this port yet (no route domain, no service layer) — the only reason
//! this module exists is to back `GET /api/search/autocomplete?field=requirement`,
//! the one place Node's search domain reaches into `requirement` at all
//! (`packages/api/src/search/service.ts:274-279`). Do not add more to this
//! module speculatively; a real `requirement` domain port belongs in its
//! own task.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

/// One `(id, title)` match from [`search_titles`].
#[derive(Debug, Clone)]
pub struct RequirementTitleMatch {
    pub id: String,
    pub title: String,
}

/// Direct port of Node's `searchRequirementTitles`
/// (`packages/db/src/repository/requirement.ts:292-299`) — same shape as
/// `chunk::search_titles`, and the same two things that look like bugs but
/// are the ported behaviour: `ILIKE '%prefix%'` with the pattern
/// unescaped, and no `ORDER BY`. See `chunk::search_titles`'s doc comment
/// for the full rationale; it applies here identically.
///
/// One intentional deviation from Node — divergence #17 (Phase 2c task
/// 8b): this now adds `AND user_id = $2`, scoping the leak Node's
/// unscoped original had. See `chunk::search_titles`'s doc comment.
pub async fn search_titles(
    pool: &PgPool,
    user_id: &str,
    prefix: &str,
    limit: i64,
) -> AppResult<Vec<RequirementTitleMatch>> {
    let pattern = format!("%{prefix}%");
    let rows = sqlx::query_as!(
        RequirementTitleMatch,
        r#"SELECT id, title FROM requirement WHERE title ILIKE $1 AND user_id = $2 LIMIT $3"#,
        pattern,
        user_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
