use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::saved_graph::{self, NewSavedGraph, SavedGraph, SavedGraphPatch};
use sqlx::PgPool;

use super::dto::{CreateSavedGraphBody, UpdateSavedGraphBody};

/// Mirrors Node's `listSavedGraphs`
/// (`packages/api/src/saved-graphs/service.ts:12-14`): a thin pass-through,
/// no ownership pre-check beyond what the repository's `WHERE user_id = $1`
/// already does.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<SavedGraph>> {
    saved_graph::list(pool, user_id, space_id).await
}

/// Mirrors Node's `getSavedGraphDetail`
/// (`packages/api/src/saved-graphs/service.ts:16-22`).
pub async fn get_detail(pool: &PgPool, user_id: &str, id: &str) -> AppResult<SavedGraph> {
    saved_graph::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("SavedGraph".into()))
}

/// Mirrors Node's `createSavedGraph`
/// (`packages/api/src/saved-graphs/service.ts:24-52`): rejects a
/// whitespace-only name with the exact message `"Saved graph name is
/// required"`, trims the stored name, defaults `layoutAlgorithm` to
/// `"force"` when omitted, and otherwise passes every field through
/// untouched (no trim on `description`, matching Node).
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    body: CreateSavedGraphBody,
) -> AppResult<SavedGraph> {
    let trimmed_name = body.name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::Validation(
            "Saved graph name is required".to_string(),
        ));
    }

    saved_graph::create(
        pool,
        user_id,
        NewSavedGraph {
            name: trimmed_name.to_string(),
            description: body.description,
            chunk_ids: body.chunk_ids,
            positions: body.positions,
            layout_algorithm: body.layout_algorithm.unwrap_or_else(|| "force".to_string()),
            space_id: body.space_id,
        },
    )
    .await
}

/// Mirrors Node's `updateSavedGraph`
/// (`packages/api/src/saved-graphs/service.ts:54-83`): 404s if the caller
/// doesn't own the row, then rejects an explicitly-provided but
/// whitespace-only `name` with `"Saved graph name cannot be empty"` — a
/// distinct message from `create`'s. Node re-checks the `UPDATE`'s own
/// result for `null` and 404s again (belt-and-suspenders, same shape as
/// `collections::service::update`); the final `ok_or_else` here plays that
/// role.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateSavedGraphBody,
) -> AppResult<SavedGraph> {
    saved_graph::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("SavedGraph".into()))?;

    if let Some(name) = &body.name
        && name.trim().is_empty()
    {
        return Err(AppError::Validation(
            "Saved graph name cannot be empty".to_string(),
        ));
    }

    saved_graph::update(
        pool,
        user_id,
        id,
        SavedGraphPatch {
            name: body.name.map(|n| n.trim().to_string()),
            description: body.description,
            chunk_ids: body.chunk_ids,
            positions: body.positions,
            layout_algorithm: body.layout_algorithm,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("SavedGraph".into()))
}

/// Mirrors Node's `deleteSavedGraph`
/// (`packages/api/src/saved-graphs/service.ts:85-89`).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if saved_graph::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("SavedGraph".into()))
    }
}
