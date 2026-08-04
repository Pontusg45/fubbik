use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::tag_type::{self, TagType};
use sqlx::PgPool;

use super::dto::{CreateTagTypeBody, UpdateTagTypeBody};

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<TagType>> {
    tag_type::list(pool, user_id).await
}

pub async fn create(pool: &PgPool, user_id: &str, body: CreateTagTypeBody) -> AppResult<TagType> {
    tag_type::create(
        pool,
        user_id,
        &body.name,
        body.color.as_deref(),
        body.icon.as_deref(),
    )
    .await
}

pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateTagTypeBody,
) -> AppResult<TagType> {
    tag_type::update(
        pool,
        user_id,
        id,
        body.name.as_deref(),
        body.color.as_deref(),
        body.icon.as_deref(),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("tag type".into()))
}

/// The database's `ON DELETE SET NULL` foreign key handles referencing
/// `tag` rows on its own (see `tag_type::delete`) — no restrict/conflict
/// check belongs here, matching Node.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if tag_type::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("tag type".into()))
    }
}
