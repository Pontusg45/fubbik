use fubbik_core::error::AppResult;
use sqlx::PgPool;

/// Aggregate per-user counts backing `GET /api/stats`. Bare-object shape,
/// not an array and not the `{chunks,total,limit,offset}` envelope the
/// chunks *list* endpoint uses — matches Node's `getUserStats`
/// (`packages/api/src/stats/service.ts`) field-for-field. No fields beyond
/// what Node returns: this is a parity port, not a place to add "useful"
/// counts.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub chunks: i64,
    pub connections: i64,
    pub tags: i64,
}

/// Every count is independently scoped to `user_id` in SQL — an unscoped
/// `count(*)` here would leak both the existence and the volume of other
/// users' data through what looks like an innocuous aggregate. Three
/// scalar subqueries in one round trip, matching Node's `Effect.all` over
/// three independent repository calls:
///
/// - `chunks`: rows owned directly by the user (`chunk.user_id`).
/// - `connections`: scoped through the parent chunk, matching Node's
///   `getConnectionCount` exactly — it joins `chunk_connection` to `chunk`
///   on `source_id` only (not `target_id`), so a connection whose source is
///   foreign but whose target belongs to the user is *not* counted here.
///   That's a deliberate parity choice, not a bug to "fix": Task 6 ports
///   Node's behavior, including this asymmetry.
/// - `tags`: rows owned directly by the user (`tag.user_id`), deduplicated
///   with `COUNT(DISTINCT id)` to mirror Node's `countDistinct(tag.id)`
///   (a plain `COUNT(*)` would already be equivalent here since `id` is
///   the primary key, but this keeps the SQL a literal mirror of Node's
///   query intent).
pub async fn get_stats(pool: &PgPool, user_id: &str) -> AppResult<Stats> {
    let stats = sqlx::query_as!(
        Stats,
        r#"SELECT
             (SELECT count(*) FROM chunk WHERE user_id = $1) AS "chunks!",
             (SELECT count(*)
                FROM chunk_connection cc
                JOIN chunk c ON c.id = cc.source_id
                WHERE c.user_id = $1) AS "connections!",
             (SELECT count(DISTINCT id) FROM tag WHERE user_id = $1) AS "tags!""#,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(stats)
}
