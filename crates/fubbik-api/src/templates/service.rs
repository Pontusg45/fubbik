use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::template::{self, NewTemplate, Template, TemplatePatch};
use sqlx::PgPool;

use super::dto::{CreateTemplateBody, UpdateTemplateBody};

/// Built-in templates plus the caller's own — see `template::list`'s doc
/// comment for the `ORDER BY` this port adds that Node's `listTemplates`
/// (`packages/api/src/templates/service.ts:14-16`) doesn't have.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Template>> {
    template::list(pool, user_id).await
}

/// Mirrors Node's `createTemplate`
/// (`packages/api/src/templates/service.ts:18-44`): a thin pass-through
/// insert, always non-built-in and owned by the caller. There is no path
/// through this API that creates a built-in template.
pub async fn create(pool: &PgPool, user_id: &str, body: CreateTemplateBody) -> AppResult<Template> {
    template::create(
        pool,
        user_id,
        NewTemplate {
            name: body.name,
            description: body.description,
            template_type: body.template_type,
            content: body.content,
            match_rules: body.match_rules,
            field_mappings: body.field_mappings,
            priority: body.priority,
            tags: body.tags,
        },
    )
    .await
}

/// Mirrors Node's `updateTemplate`
/// (`packages/api/src/templates/service.ts:46-68`) exactly: an unscoped
/// existence lookup, a built-in rejection (`ValidationError` — 400, the
/// message copied verbatim), then a user-scoped `UPDATE` whose `None`
/// result (id doesn't exist, or exists but isn't the caller's) also maps to
/// `NotFoundError` — 404. Removing the `is_built_in` check below still
/// leaves a real built-in template unmodifiable, because
/// `template::update`'s own `WHERE user_id = $2` can never match a
/// built-in row's `NULL` `user_id` — see
/// `tests/template.rs::update_rejects_built_in_template` for the
/// load-bearing proof (removing this check flips the response from 400 to
/// 404, not to a successful mutation).
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateTemplateBody,
) -> AppResult<Template> {
    let found = template::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Template".into()))?;

    if found.is_built_in {
        return Err(AppError::Validation(
            "Cannot edit built-in templates".into(),
        ));
    }

    template::update(
        pool,
        user_id,
        id,
        TemplatePatch {
            name: body.name,
            description: body.description,
            template_type: body.template_type,
            content: body.content,
            match_rules: body.match_rules,
            field_mappings: body.field_mappings,
            priority: body.priority,
            tags: body.tags,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Template".into()))
}

/// Mirrors Node's `deleteTemplate`
/// (`packages/api/src/templates/service.ts:70-79`) exactly: same shape as
/// `update` above, but the scoped `DELETE` also carries an explicit
/// `is_built_in = false` in SQL — see `template::delete`'s doc comment for
/// why that's independently load-bearing beyond the `user_id` scoping.
/// `tests/template.rs::delete_rejects_built_in_template` proves removing
/// *this* service-layer check still 404s (not deletes), same shape as
/// `update`'s proof.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    let found = template::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Template".into()))?;

    if found.is_built_in {
        return Err(AppError::Validation(
            "Cannot delete built-in templates".into(),
        ));
    }

    if template::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Template".into()))
    }
}
