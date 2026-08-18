use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::space;
use fubbik_db::repo::use_case::{
    self, NewUseCase, UseCase, UseCaseListItem, UseCasePatch, UseCaseRequirement,
};
use sqlx::PgPool;

use super::dto::{CreateUseCaseBody, UpdateUseCaseBody};

pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<UseCaseListItem>> {
    use_case::list(pool, user_id, space_id).await
}

/// Mirrors Node's `createUseCase`
/// (`packages/api/src/use-cases/service.ts:24-45`): when `parentId` is
/// given, the parent must exist (scoped to the caller — `getUseCaseById`
/// always receives `userId`) and must not itself already have a parent
/// (Node enforces exactly one level of nesting).
///
/// **`spaceId` ownership is now checked (Phase 2e wave 1)** — a deliberate
/// divergence from Node, whose `createUseCaseRepo` is a bare insert with no
/// such guard, same shape as accepted divergences #4/#9/#10/#13/#14/#15/#17/
/// #19 and matching `collections::service::create`'s own pre-check for the
/// analogous field. This service-level `space::find_by_id` pre-check gives
/// the precise 404, but the *real* guard is the `EXISTS` in
/// `use_case::create`'s own SQL — see that function's doc comment and
/// `tests/use_case.rs` for why the SQL guard, not this pre-check, is what's
/// proven load-bearing.
pub async fn create(pool: &PgPool, user_id: &str, body: CreateUseCaseBody) -> AppResult<UseCase> {
    if let Some(parent_id) = &body.parent_id {
        let parent = use_case::find_by_id(pool, user_id, parent_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Parent use case".into()))?;
        if parent.parent_id.is_some() {
            return Err(AppError::Validation(
                "Cannot nest more than one level deep".into(),
            ));
        }
    }
    if let Some(space_id) = &body.space_id {
        space::find_by_id(pool, user_id, space_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Space".into()))?;
    }

    use_case::create(
        pool,
        user_id,
        NewUseCase {
            name: body.name,
            description: body.description,
            space_id: body.space_id,
            parent_id: body.parent_id,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Space".into()))
}

/// Mirrors Node's `updateUseCase`
/// (`packages/api/src/use-cases/service.ts:47-77`): 404 up front if the use
/// case isn't the caller's. The re-parenting checks only run when `parentId`
/// is present *and* non-null (Node: `body.parentId !== undefined &&
/// body.parentId !== null`) — an omitted `parentId` skips them entirely,
/// and so does an explicit `null` (detaching from a parent needs no
/// validation, only attaching to one does). In order: self-parent rejected
/// first, then the parent must exist (scoped to the caller), then the
/// parent must not itself have a parent (one level of nesting).
///
/// Node has one further check here — `if (parent.parentId === id)` — that
/// is dead code: it can only run when `parent.parentId` is falsy (the
/// branch above already returned on any truthy `parent.parentId`), so
/// `null === id` is always false for a non-empty `id`. Not reproduced here;
/// it can never fire in Node either.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateUseCaseBody,
) -> AppResult<UseCase> {
    use_case::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("UseCase".into()))?;

    if let Some(Some(parent_id)) = &body.parent_id {
        if parent_id == id {
            return Err(AppError::Validation(
                "Cannot set use case as its own parent".into(),
            ));
        }
        let parent = use_case::find_by_id(pool, user_id, parent_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Parent use case".into()))?;
        if parent.parent_id.is_some() {
            return Err(AppError::Validation(
                "Cannot nest more than one level deep".into(),
            ));
        }
    }

    use_case::update(
        pool,
        user_id,
        id,
        UseCasePatch {
            name: body.name,
            description: body.description,
            order: body.order,
            parent_id: body.parent_id,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("UseCase".into()))
}

/// `deleteUseCase` (`packages/api/src/use-cases/service.ts:79-82`): 404 if
/// not found/not owned. Dependent `requirement.use_case_id` rows are nulled
/// by the database's own `ON DELETE SET NULL`, not application code — see
/// `fubbik_db::repo::use_case`'s module doc comment for the full writeup,
/// including the one behavioural divergence from Node (a rejected
/// cross-user delete no longer has the side effect of unlinking the real
/// owner's requirements).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if use_case::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("UseCase".into()))
    }
}

/// Mirrors Node's `getUseCaseRequirements`
/// (`packages/api/src/use-cases/service.ts:84-87`): 404 if the use case
/// isn't the caller's, otherwise every `requirement` row pointing at it
/// (also scoped to the caller — see `use_case::list_requirements`'s doc
/// comment).
pub async fn get_requirements(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<Vec<UseCaseRequirement>> {
    use_case::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("UseCase".into()))?;
    use_case::list_requirements(pool, user_id, id).await
}
