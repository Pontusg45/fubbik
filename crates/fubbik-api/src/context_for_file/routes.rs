//! `GET /api/context/for-file`. Ports
//! `packages/api/src/context-for-file/routes.ts`.
//!
//! Three `format` values, two different pipelines:
//!
//! - `json-legacy` calls [`service::get_context_for_file`] directly and
//!   returns its raw `{chunks, requirements}` shape — scores carry the
//!   five strategies' additive bonuses and each chunk's `matchReason`.
//! - `structured-md`/`structured-json` (default) instead go through
//!   `resolve_for_files` -> `enrich_chunks` -> budget -> format, the same
//!   pipeline every other `/api/context/*` route uses. This means the
//!   "structured" formats do NOT expose strategy bonuses or match reasons
//!   at all — a chunk's score there is purely `context::service`'s
//!   health-based `score_chunk`, matching Node's `context-for-file/
//!   routes.ts:39-46` exactly (only `json-legacy` reaches
//!   `getContextForFile`'s own `score`/`matchReason` fields).

use axum::Router;
use axum::extract::State;
use axum::routing::get;

use super::dto::{ForFileFormat, ForFileQuery, ForFileResponse, parse_deps};
use super::service::get_context_for_file;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::context::dto::{ContextFormat, ContextResponse, parse_max_tokens};
use crate::context::resolvers::resolve_for_files;
use crate::context::routes::budget_and_format;
use crate::context::service::enrich_chunks;
use crate::error::ApiResult;
use crate::extract::Query;

/// `GET /api/context/for-file?path=X&spaceId=&deps=&format=&maxTokens=`.
#[utoipa::path(get, path = "/api/context/for-file", params(ForFileQuery),
    responses((status = 200, body = ForFileResponse)))]
pub async fn for_file(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ForFileQuery>,
) -> ApiResult<axum::Json<ForFileResponse>> {
    let format = query.format.unwrap_or_default();

    if format == ForFileFormat::JsonLegacy {
        let deps = parse_deps(query.deps.as_deref());
        let file_context = get_context_for_file(
            &state.pool,
            &state.ai,
            &user.id,
            &query.path,
            query.space_id.as_deref(),
            deps.as_deref(),
        )
        .await?;
        return Ok(axum::Json(ForFileResponse::JsonLegacy(file_context)));
    }

    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let paths = vec![query.path.clone()];
    let ids = resolve_for_files(
        &state.pool,
        &state.ai,
        &user.id,
        &paths,
        query.space_id.as_deref(),
    )
    .await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);

    let context_format = match format {
        ForFileFormat::StructuredJson => ContextFormat::StructuredJson,
        // `StructuredMd` is the default arm; `JsonLegacy` already returned above.
        _ => ContextFormat::StructuredMd,
    };
    Ok(axum::Json(ForFileResponse::Structured(
        ContextResponse::from_structured(structured, context_format),
    )))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/context/for-file", get(for_file))
}
