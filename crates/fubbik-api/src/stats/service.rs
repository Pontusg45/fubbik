use fubbik_core::error::AppResult;
use fubbik_db::repo::stats::{self, Stats};
use sqlx::PgPool;

/// Thin pass-through — the user-scoping SQL lives in
/// `fubbik_db::repo::stats::get_stats`, not here. No business logic sits
/// between the route and the query for this domain.
pub async fn get_stats(pool: &PgPool, user_id: &str) -> AppResult<Stats> {
    stats::get_stats(pool, user_id).await
}
