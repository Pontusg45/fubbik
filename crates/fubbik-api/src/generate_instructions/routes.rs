//! `GET /api/spaces/{id}/generate-instructions?format=claude|agents|cursor`
//! and its deprecated `/api/codebases/{id}/...` alias. See the module doc
//! comment on `generate_instructions` for the ownership-check divergence
//! from Node; see [`generate_instructions_codebases_alias_route`]'s doc
//! comment for why both paths are served.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use super::service::{self, GenerateInstructionsResponse, InstructionFormat};
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct GenerateInstructionsQuery {
    pub format: Option<InstructionFormat>,
}

/// `unknown_format_is_rejected_or_defaults`: an unrecognised `format`
/// value fails to deserialize against `InstructionFormat`'s three
/// literals, so `extract::Query` rejects the request with
/// `AppError::Validation` -> `400`, the same "reject" behaviour Node's
/// Elysia `t.Union([t.Literal(...), ...])` query schema produces (Elysia
/// responds non-2xx for a query value outside the declared union before
/// the handler body ever runs) — not a silent default to `claude`.
#[utoipa::path(get, path = "/api/spaces/{id}/generate-instructions",
    params(("id" = String, Path,), GenerateInstructionsQuery),
    responses((status = 200, body = GenerateInstructionsResponse), (status = 404)))]
pub async fn generate_instructions_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Query(query): Query<GenerateInstructionsQuery>,
) -> ApiResult<Json<GenerateInstructionsResponse>> {
    let result = service::generate_instructions(&state.pool, &user.id, &id, query.format).await?;
    Ok(Json(result))
}

/// Deprecated alias: `GET /api/codebases/{id}/generate-instructions`.
///
/// **This is Node's *actual* (and only) registration.** Node's
/// `generateInstructionsRoutes` (`packages/api/src/generate-instructions/
/// routes.ts:8`) registers exactly one path, `/codebases/:id/generate-
/// instructions` — it never registers `/spaces/:id/generate-instructions`
/// at all, and `codebaseRoutes` (the `/api/codebases` -> `/api/spaces`
/// forwarding layer used for every other space endpoint) does not cover
/// this route either, since it forwards the opposite direction and this
/// handler isn't one of the ones it re-exports.
///
/// Yet Node's own CLI (`apps/cli/src/commands/generate.ts:35,58,81`) calls
/// `/api/spaces/${spaceId}/generate-instructions` at all three call sites
/// — matching how every *other* CLI call in that codebase addresses spaces
/// (`api/spaces`, never `api/codebases`). So in a live Node deployment,
/// `fubbik generate claude.md`/`agents.md`/`.cursorrules` all 404 today.
/// This is fallout from the project's `codebases` -> `spaces` rename: the
/// CLI's call sites and the rest of the API surface were renamed, but this
/// one route registration was missed.
///
/// This port serves **both** paths, pointed at the same handler
/// ([`generate_instructions_route`]), rather than picking one:
/// `/api/spaces/{id}/generate-instructions` is primary — it matches the
/// CLI and the project's own rename, and fixes the three broken commands.
/// This `/api/codebases/{id}/...` path is kept as a deprecated
/// compatibility alias matching the *real* shape of Node's API today, the
/// same pattern already used for `/api/codebases` elsewhere in this
/// codebase (see CLAUDE.md: "`/api/codebases` is a deprecated alias kept
/// for backward compatibility with older VS Code extension builds and
/// external consumers."). Serving only `/spaces` would silently drop a
/// path that genuinely exists in Node today, breaking any external
/// consumer or older client still calling it at cutover.
#[utoipa::path(get, path = "/api/codebases/{id}/generate-instructions",
    params(("id" = String, Path,), GenerateInstructionsQuery),
    responses((status = 200, body = GenerateInstructionsResponse), (status = 404)))]
pub async fn generate_instructions_codebases_alias_route(
    state: State<AppState>,
    user: CurrentUser,
    id: Path<String>,
    query: Query<GenerateInstructionsQuery>,
) -> ApiResult<Json<GenerateInstructionsResponse>> {
    generate_instructions_route(state, user, id, query).await
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/spaces/{id}/generate-instructions",
            get(generate_instructions_route),
        )
        .route(
            "/api/codebases/{id}/generate-instructions",
            get(generate_instructions_codebases_alias_route),
        )
}
