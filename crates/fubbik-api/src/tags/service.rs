use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::tag::{self, MergeResult, Tag, TagListItem, TagPatch};
use sqlx::PgPool;

use super::dto::{CreateTagBody, MergeBody, UpdateTagBody};

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<TagListItem>> {
    tag::list(pool, user_id).await
}

pub async fn create(pool: &PgPool, user_id: &str, body: CreateTagBody) -> AppResult<Tag> {
    tag::create(pool, user_id, &body.name, body.tag_type_id.as_deref()).await
}

pub async fn update(pool: &PgPool, user_id: &str, id: &str, body: UpdateTagBody) -> AppResult<Tag> {
    // Pre-check a rename for a collision so it surfaces as 400, not the DB's
    // `tag_user_name_idx` unique-index violation bubbling up as a raw 500 —
    // matching Node's `tagNameConflict` guard (service-new.ts:33-58).
    if let Some(name) = &body.name
        && tag::name_conflict(pool, user_id, id, name).await?
    {
        return Err(AppError::Validation(format!(
            "Tag \"{name}\" already exists"
        )));
    }

    // `reviewedBy`/`reviewedAt` are server-set, not client-settable — they
    // change together with `reviewStatus` or not at all, matching Node.
    let (reviewed_by, reviewed_at) = if body.review_status.is_some() {
        (
            Some(user_id.to_string()),
            Some(chrono::Utc::now().naive_utc().into()),
        )
    } else {
        (None, None)
    };

    tag::update(
        pool,
        user_id,
        id,
        TagPatch {
            name: body.name,
            tag_type_id: body.tag_type_id,
            review_status: body.review_status,
            reviewed_by,
            reviewed_at,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("tag".into()))
}

/// Cascades to `chunk_tag` rows via the database's own `ON DELETE CASCADE`
/// foreign key (`tag::delete`) — no explicit cleanup belongs here, matching
/// Node.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if tag::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("tag".into()))
    }
}

pub async fn merge(pool: &PgPool, user_id: &str, body: MergeBody) -> AppResult<MergeResult> {
    if body.source_id == body.target_id {
        return Err(AppError::Validation(
            "Cannot merge a tag into itself".into(),
        ));
    }
    tag::merge(pool, user_id, &body.source_id, &body.target_id).await
}
