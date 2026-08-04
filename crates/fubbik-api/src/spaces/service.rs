use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::space::{
    self, CodeInput, CodeUpdate, NewSpace, ResetResult, Space, SpaceDetail, SpacePatch,
};
use sqlx::PgPool;

use super::dto::{CreateSpaceBody, DetectQuery, UpdateSpaceBody};
use super::normalize_url::normalize_git_url;

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Space>> {
    space::list(pool, user_id).await
}

/// `{ space, code }` — the nested detail shape. See the doc comment on
/// `fubbik_db::repo::space::SpaceDetail`.
pub async fn get(pool: &PgPool, user_id: &str, id: &str) -> AppResult<SpaceDetail> {
    space::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))
}

pub async fn create(pool: &PgPool, user_id: &str, body: CreateSpaceBody) -> AppResult<Space> {
    let kind = body.kind.unwrap_or_else(|| "code".to_string());
    // `body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : undefined` — only
    // a truthy (non-empty) string gets normalized; an absent one stays
    // `None`, never an empty string.
    let remote_url = body
        .remote_url
        .filter(|s| !s.is_empty())
        .map(|s| normalize_git_url(&s));

    if kind == "code"
        && let Some(remote_url) = &remote_url
        && space::find_by_remote_url(pool, user_id, remote_url)
            .await?
            .is_some()
    {
        return Err(AppError::Validation(
            "A space with this remote URL already exists".into(),
        ));
    }

    // Node keys `code` off `kind` alone (`kind === "code" ? {...} :
    // undefined`) — a code-kind space always gets a `space_code_metadata`
    // row, even with neither `remoteUrl` nor `localPaths` given.
    let code = (kind == "code").then(|| CodeInput {
        remote_url: remote_url.clone(),
        local_paths: body.local_paths.unwrap_or_default(),
    });

    space::create(
        pool,
        user_id,
        NewSpace {
            name: body.name,
            kind,
            description: body.description,
        },
        code,
    )
    .await
}

/// **Note the surprising Node behavior replicated here**: `code` is
/// constructed unconditionally whenever the *existing* space is
/// `kind == "code"` — regardless of whether the request body mentioned
/// `remoteUrl`/`localPaths` at all (`packages/api/src/spaces/service.ts:76`,
/// `packages/db/src/repository/space.ts:123`'s `if (params.code)` is always
/// truthy in that case). So a `PATCH` that only sends `{"name": "..."}`
/// against a code-kind space silently clears its `remoteUrl` to `null` and
/// `localPaths` to `[]`. This is replicated verbatim as a parity port, not
/// treated as a bug to fix — see `tests/spaces.rs::patching_name_only_on_a_code_space_clears_its_remote_url_and_local_paths`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateSpaceBody,
) -> AppResult<Space> {
    let found = space::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;

    // `body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : body.remoteUrl` —
    // a truthy string gets normalized; undefined/null/"" pass through as-is.
    let remote_url: Option<Option<String>> = match body.remote_url {
        Some(Some(url)) if !url.is_empty() => Some(Some(normalize_git_url(&url))),
        other => other,
    };

    let code = (found.space.kind == "code").then(|| CodeUpdate {
        remote_url: remote_url.flatten(),
        local_paths: body.local_paths.unwrap_or_default(),
    });

    space::update(
        pool,
        user_id,
        id,
        SpacePatch {
            name: body.name,
            description: body.description,
        },
        code,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Space".into()))
}

/// `resetSpace` (`packages/api/src/spaces/service.ts:82-87`): 404 if not
/// found/not owned, otherwise wipe the space's content. See
/// `fubbik_db::repo::space::reset` for exactly what "content" means.
pub async fn reset(pool: &PgPool, user_id: &str, id: &str) -> AppResult<ResetResult> {
    space::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;
    space::reset(pool, user_id, id).await
}

/// `deleteSpace` (`packages/api/src/spaces/service.ts:89-95`): 404 if not
/// found/not owned, otherwise wipe the content (same as `reset`) and then
/// delete the `space` row itself.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    space::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;
    space::reset(pool, user_id, id).await?;
    if space::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Space".into()))
    }
}

/// `detectSpace` (`packages/api/src/spaces/service.ts:97-102`): `remoteUrl`
/// takes priority over `localPath` when both are given; a truthy
/// `remoteUrl` that normalizes to an empty string falls through to
/// `localPath` (JS-truthiness edge case, replicated via the `filter`
/// chain). Neither given, or neither matches: `None`, which the route
/// layer turns into an empty 200 body, not `null`/`{}`/404.
pub async fn detect(pool: &PgPool, user_id: &str, query: DetectQuery) -> AppResult<Option<Space>> {
    let normalized_url = query
        .remote_url
        .filter(|s| !s.is_empty())
        .map(|s| normalize_git_url(&s))
        .filter(|s| !s.is_empty());

    if let Some(url) = normalized_url {
        return space::find_by_remote_url(pool, user_id, &url).await;
    }

    if let Some(path) = query.local_path.filter(|s| !s.is_empty()) {
        return space::find_by_local_path(pool, user_id, &path).await;
    }

    Ok(None)
}
