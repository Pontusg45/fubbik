use chrono::{Duration, Utc};
use fubbik_core::error::AppResult;
use sqlx::PgPool;

use super::user::User;

/// Creates a session and returns its opaque token.
pub async fn create(pool: &PgPool, user_id: &str, ttl: Duration) -> AppResult<String> {
    let id = crate::new_id();
    let token = format!("{}{}", crate::new_id(), crate::new_id());
    let expires_at = Utc::now() + ttl;

    sqlx::query!(
        r#"INSERT INTO session (id, token, user_id, expires_at, created_at, updated_at)
           VALUES ($1, $2, $3, $4, now(), now())"#,
        id,
        token,
        user_id,
        expires_at.naive_utc()
    )
    .execute(pool)
    .await?;

    Ok(token)
}

/// Returns the owning user if the token exists and has not expired.
pub async fn find_valid(pool: &PgPool, token: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT u.id, u.email, u.name, u.password_hash
           FROM session s
           JOIN "user" u ON u.id = s.user_id
           WHERE s.token = $1 AND s.expires_at > now()"#,
        token
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn delete(pool: &PgPool, token: &str) -> AppResult<()> {
    sqlx::query!("DELETE FROM session WHERE token = $1", token)
        .execute(pool)
        .await?;
    Ok(())
}
