use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::notification::{self, Notification};
use sqlx::PgPool;

pub async fn list(
    pool: &PgPool,
    user_id: &str,
    unread_only: bool,
    limit: i64,
) -> AppResult<Vec<Notification>> {
    notification::list(pool, user_id, unread_only, limit).await
}

pub async fn count_unread(pool: &PgPool, user_id: &str) -> AppResult<i64> {
    notification::count_unread(pool, user_id).await
}

pub async fn mark_read(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Notification> {
    notification::mark_read(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("notification".into()))
}

pub async fn mark_all_read(pool: &PgPool, user_id: &str) -> AppResult<()> {
    notification::mark_all_read(pool, user_id).await
}

/// The database's own row-existence check via `rows_affected()` is the only
/// thing standing between "deleted" and "never existed / not yours" — see
/// `notification::delete`.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if notification::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("notification".into()))
    }
}
