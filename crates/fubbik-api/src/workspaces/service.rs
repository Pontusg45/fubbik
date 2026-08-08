use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::space;
use fubbik_db::repo::workspace::{
    self, NewWorkspace, Workspace, WorkspacePatch, WorkspaceSpaceLink,
};
use sqlx::PgPool;

use super::dto::{CreateWorkspaceBody, UpdateWorkspaceBody, WorkspaceDetail};

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Workspace>> {
    workspace::list(pool, user_id).await
}

/// `{ ...found, spaces }` — the flattened detail shape. See the doc comment
/// on `dto::WorkspaceDetail`.
pub async fn get_detail(pool: &PgPool, user_id: &str, id: &str) -> AppResult<WorkspaceDetail> {
    let found = workspace::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace".into()))?;
    let spaces = workspace::spaces_for_workspace(pool, user_id, id).await?;
    Ok(WorkspaceDetail::new(found, spaces))
}

/// Mirrors Node's `createWorkspace`
/// (`packages/api/src/workspaces/service.ts:31-51`): rejects a
/// whitespace-only name — which passes Elysia's `t.String` schema
/// unchanged — with a 400, and trims the name before insert. Node has no
/// application-level uniqueness check on `(userId, name)`; a duplicate
/// throws the DB's own unique-index violation straight through as a 500,
/// reproduced here by *not* pre-checking either (see `_mutating.md`).
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    body: CreateWorkspaceBody,
) -> AppResult<Workspace> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Workspace name is required".into()));
    }
    workspace::create(
        pool,
        user_id,
        NewWorkspace {
            name: name.to_string(),
            description: body.description,
        },
    )
    .await
}

/// Mirrors Node's `updateWorkspace`
/// (`packages/api/src/workspaces/service.ts:53-76`): 404 up front if the
/// workspace isn't the caller's, then 400 if a *provided* `name` trims to
/// empty — an omitted `name` never trips this check, only one that's
/// present but blank does.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateWorkspaceBody,
) -> AppResult<Workspace> {
    workspace::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace".into()))?;

    let name = match body.name {
        Some(n) => {
            let trimmed = n.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::Validation(
                    "Workspace name cannot be empty".into(),
                ));
            }
            Some(trimmed)
        }
        None => None,
    };

    workspace::update(
        pool,
        user_id,
        id,
        WorkspacePatch {
            name,
            description: body.description,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Workspace".into()))
}

/// `deleteWorkspace` (`packages/api/src/workspaces/service.ts:78-82`): 404
/// if not found/not owned. `workspace_space` rows cascade via the FK, not
/// application code — see `workspace::delete`'s doc comment.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if workspace::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Workspace".into()))
    }
}

/// Mirrors Node's `addSpaceToWorkspace`
/// (`packages/api/src/workspaces/service.ts:84-94`): two independent
/// 404s — `Workspace` then `Space` — checked in application code *before*
/// the insert. This is required to reproduce Node's distinct error per
/// missing resource: the repo's own SQL-level ownership guard
/// (`workspace::add_space`) can't tell "workspace not owned" apart from
/// "space not owned" from a bare zero-rows result (see its doc comment),
/// so the disambiguation has to happen here, the same way
/// `favorites::service::add` pre-checks chunk ownership ahead of
/// `favorite::add`'s own SQL-level guard.
///
/// Falls back to the same `{ workspaceId, spaceId }` literal Node does on
/// an already-linked conflict (`workspace::add_space` returning `None`
/// here can only mean "already linked" — both ownership checks above
/// already passed).
pub async fn add_space_to_workspace(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    space_id: &str,
) -> AppResult<WorkspaceSpaceLink> {
    workspace::find_by_id(pool, user_id, workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace".into()))?;
    space::find_by_id(pool, user_id, space_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;

    let link = workspace::add_space(pool, user_id, workspace_id, space_id).await?;
    Ok(link.unwrap_or_else(|| WorkspaceSpaceLink {
        workspace_id: workspace_id.to_string(),
        space_id: space_id.to_string(),
    }))
}

/// Mirrors Node's `removeSpaceFromWorkspace`
/// (`packages/api/src/workspaces/service.ts:96-105`): 404 `Workspace` if
/// the workspace isn't the caller's, otherwise 404 `WorkspaceSpace` if the
/// join row didn't exist (never linked, or already removed).
pub async fn remove_space_from_workspace(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    space_id: &str,
) -> AppResult<()> {
    workspace::find_by_id(pool, user_id, workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace".into()))?;

    if workspace::remove_space(pool, user_id, workspace_id, space_id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("WorkspaceSpace".into()))
    }
}
