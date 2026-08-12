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

/// Fixed id for the Better Auth `user` row backing the implicit dev
/// session. Shared literal, not derived from `DEV_EMAIL` — Node's
/// `IMPLICIT_DEV_USER_ID` (`packages/db/src/repository/implicit-dev-user.ts`)
/// is likewise a standalone constant, and both stacks must agree on the
/// exact row.
pub const IMPLICIT_DEV_USER_ID: &str = "dev-user";

/// Idempotent bootstrap of the dev-user row, matching Node's
/// `ensureImplicitDevUserRow` field-for-field: `id = "dev-user"`,
/// `name = "Dev User"`, `email = "dev@localhost"`, `email_verified = false`.
/// Select-check by id, `ON CONFLICT (id) DO NOTHING`, re-select — same shape
/// as Node's select -> insert().onConflictDoNothing() -> select.
///
/// The `ON CONFLICT` target is `id`, not a bare `ON CONFLICT DO NOTHING`,
/// deliberately mirroring Node's `onConflictDoNothing({ target: user.id })`
/// literally. That means it only absorbs a same-id collision; two callers
/// racing through the initial "does it exist" check on a truly empty
/// database can both reach the insert and collide on the separate `email`
/// unique constraint instead, which `ON CONFLICT (id)` does not cover and
/// which surfaces as a database error. Node's own implementation has this
/// same window — not a gap introduced by this port. Sequential callers
/// (by far the common case: the row is created once, then just read) are
/// unaffected.
pub async fn ensure_implicit_dev_user(pool: &PgPool) -> AppResult<User> {
    if let Some(existing) = find_by_id(pool, IMPLICIT_DEV_USER_ID).await? {
        return Ok(existing);
    }

    sqlx::query!(
        r#"INSERT INTO "user" (id, name, email, email_verified)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (id) DO NOTHING"#,
        IMPLICIT_DEV_USER_ID,
        "Dev User",
        "dev@localhost",
        false,
    )
    .execute(pool)
    .await?;

    let user = find_by_id(pool, IMPLICIT_DEV_USER_ID)
        .await?
        .expect("row exists after insert-or-conflict-then-reselect");
    Ok(user)
}
