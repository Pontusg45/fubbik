//! `GET /api/chunks/export/context` and `GET /api/chunks/export/claude-md`.
//!
//! Ports `packages/api/src/context-export/routes.ts`. Both handlers parse
//! `maxTokens` the same lenient way `context::dto::parse_max_tokens` does
//! for the other `/api/context/*` routes (an absent or unparsable value
//! falls back to the endpoint's own default rather than erroring) — but
//! each endpoint has its own default (4000 here, 32000 for CLAUDE.md), so
//! that shared helper (which bakes in 4000) isn't reused verbatim.

use axum::Router;
use axum::extract::State;
use axum::routing::get;

use super::claude_md::{
    ClaudeMdParams, ClaudeMdResponse, DEFAULT_MAX_TOKENS as CLAUDE_MD_DEFAULT_MAX_TOKENS,
    generate_claude_md,
};
use super::service::{
    DEFAULT_MAX_TOKENS as EXPORT_CONTEXT_DEFAULT_MAX_TOKENS, ExportContextParams,
    ExportContextResponse, ExportFormat, export_context,
};
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

/// Parses `maxTokens`, matching Node's `ctx.query.maxTokens ? Number(...) :
/// undefined` fallback-to-default behaviour. An unparsable value falls back
/// to `default` rather than erroring, same rationale as
/// `context::dto::parse_max_tokens`: Node's `Number("bogus")` is `NaN`,
/// which would otherwise silently admit every chunk rather than erroring.
fn parse_max_tokens(raw: Option<&str>, default: usize) -> usize {
    raw.and_then(|s| s.parse().ok()).unwrap_or(default)
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ExportContextQuery {
    pub space_id: Option<String>,
    pub max_tokens: Option<String>,
    pub format: Option<ExportFormat>,
    pub for_path: Option<String>,
}

/// `GET /api/chunks/export/context?spaceId=&maxTokens=&format=markdown|json&forPath=`.
#[utoipa::path(get, path = "/api/chunks/export/context", params(ExportContextQuery),
    responses((status = 200, body = ExportContextResponse)))]
pub async fn export_context_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ExportContextQuery>,
) -> ApiResult<axum::Json<ExportContextResponse>> {
    let max_tokens = parse_max_tokens(
        query.max_tokens.as_deref(),
        EXPORT_CONTEXT_DEFAULT_MAX_TOKENS,
    );
    let format = query.format.unwrap_or_default();

    let result = export_context(
        &state.pool,
        &state.ai,
        &user.id,
        ExportContextParams {
            space_id: query.space_id.as_deref(),
            max_tokens,
            format,
            for_path: query.for_path.as_deref(),
        },
    )
    .await?;

    Ok(axum::Json(result))
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMdQuery {
    pub space_id: Option<String>,
    pub tag: Option<String>,
    pub max_tokens: Option<String>,
}

/// `GET /api/chunks/export/claude-md?spaceId=&tag=&maxTokens=`.
#[utoipa::path(get, path = "/api/chunks/export/claude-md", params(ClaudeMdQuery),
    responses((status = 200, body = ClaudeMdResponse)))]
pub async fn export_claude_md_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ClaudeMdQuery>,
) -> ApiResult<axum::Json<ClaudeMdResponse>> {
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref(), CLAUDE_MD_DEFAULT_MAX_TOKENS);

    let result = generate_claude_md(
        &state.pool,
        &user.id,
        ClaudeMdParams {
            space_id: query.space_id.as_deref(),
            tag: query.tag.as_deref(),
            max_tokens,
        },
    )
    .await?;

    Ok(axum::Json(result))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks/export/context", get(export_context_route))
        .route("/api/chunks/export/claude-md", get(export_claude_md_route))
}
