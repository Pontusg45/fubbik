use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    #[serde(skip)]
    pub password_hash: Option<String>,
}

pub async fn create(
    pool: &PgPool,
    email: &str,
    name: &str,
    password_hash: Option<&str>,
) -> AppResult<User> {
    let id = crate::new_id();
    let user = sqlx::query_as!(
        User,
        r#"INSERT INTO "user" (id, email, name, password_hash, email_verified)
           VALUES ($1, $2, $3, $4, true)
           RETURNING id, email, name, password_hash"#,
        id,
        email,
        name,
        password_hash
    )
    .fetch_one(pool)
    .await?;
    Ok(user)
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, name, password_hash FROM "user" WHERE email = $1"#,
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id, email, name, password_hash FROM "user" WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}
