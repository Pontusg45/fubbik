//! `GET /api/context/for-plan`, `/api/context/about`, `/api/context/for-files`.
//!
//! Ports `packages/api/src/context/routes.ts`. All three handlers share one
//! shape — resolve candidate ids, enrich them into scored/health-annotated
//! chunks, budget them into a token limit, then format the result —
//! differing only in which resolver runs and which query parameters feed
//! it. `maxTokens` stays string-typed on the wire (parsed here rather than
//! declared as a number), matching Node's `t.Optional(t.String())` schema.

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use fubbik_core::format::format_structured;
use fubbik_core::score::budget_chunks;

use super::dto::{AboutQuery, ContextResponse, ForFilesQuery, ForPlanQuery, parse_max_tokens};
use super::resolvers::{resolve_for_concept, resolve_for_files, resolve_for_plan};
use super::service::enrich_chunks;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::Query;
use fubbik_core::error::AppError;

/// `GET /api/context/for-plan?planId=X&maxTokens=N&format=structured-md`.
///
/// `resolve_for_plan` returns `NotFound` for a plan the caller doesn't own
/// (a deliberate tightening over Node — see the resolver's own doc comment
/// for the full ruling); this handler just propagates it via `?`, it does
/// not add a second check.
#[utoipa::path(get, path = "/api/context/for-plan", params(ForPlanQuery),
    responses((status = 200, body = ContextResponse), (status = 404)))]
pub async fn for_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ForPlanQuery>,
) -> ApiResult<axum::Json<ContextResponse>> {
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let format = query.format.unwrap_or_default();

    let ids = resolve_for_plan(&state.pool, &user.id, &query.plan_id).await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// `GET /api/context/about?q=auth&maxTokens=N&spaceId=X&format=structured-md`.
#[utoipa::path(get, path = "/api/context/about", params(AboutQuery),
    responses((status = 200, body = ContextResponse)))]
pub async fn about(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<AboutQuery>,
) -> ApiResult<axum::Json<ContextResponse>> {
    // Matches Node's `if (!ctx.query.q)` exactly (`context/routes.ts:62-64`):
    // JS falsy-checks a string, so only an exactly-empty `q=` 400s. A
    // whitespace-only `q` (e.g. `q=%20`) is truthy in JS and reaches the
    // resolver unchanged — `.trim().is_empty()` here would reject a request
    // Node accepts, so this deliberately checks emptiness, not blankness.
    if query.q.is_empty() {
        return Err(ApiError::from(AppError::Validation("q is required".into())));
    }
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let format = query.format.unwrap_or_default();

    let ids = resolve_for_concept(
        &state.pool,
        &state.ai,
        &user.id,
        &query.q,
        query.space_id.as_deref(),
    )
    .await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// `GET /api/context/for-files?paths=a.ts,b.ts&maxTokens=N&spaceId=X&format=structured-md`.
#[utoipa::path(get, path = "/api/context/for-files", params(ForFilesQuery),
    responses((status = 200, body = ContextResponse), (status = 400)))]
pub async fn for_files(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ForFilesQuery>,
) -> ApiResult<axum::Json<ContextResponse>> {
    let paths: Vec<String> = query
        .paths
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if paths.is_empty() {
        return Err(ApiError::from(AppError::Validation(
            "paths must contain at least one path".into(),
        )));
    }
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let format = query.format.unwrap_or_default();

    let ids = resolve_for_files(&state.pool, &user.id, &paths, query.space_id.as_deref()).await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// Shared resolve->enrich tail: budgets the enriched chunks into
/// `max_tokens`, then groups the survivors into sections. `budget_chunks`
/// operates on bare `ScoredChunk`s and Node's own `budgetChunks` takes and
/// returns the full `ChunkWithMetadata[]` (`utils.ts:39`) — this port's
/// `budget_chunks` signature (Task 4b/5) only knows about `ScoredChunk`, so
/// budgeting here strips the metadata, budgets, then re-attaches it by id
/// rather than widening `fubbik_core::score::budget_chunks`'s signature for
/// this one caller.
fn budget_and_format(
    chunks: Vec<fubbik_core::format::ChunkWithMetadata>,
    max_tokens: usize,
) -> fubbik_core::format::StructuredContext {
    use std::collections::HashMap;

    let mut meta_by_id: HashMap<String, fubbik_core::format::ChunkWithMetadata> = chunks
        .into_iter()
        .map(|c| (c.chunk.id.clone(), c))
        .collect();

    let scored: Vec<_> = meta_by_id.values().map(|c| c.chunk.clone()).collect();

    let budgeted_ids: Vec<String> = budget_chunks(scored, max_tokens)
        .into_iter()
        .map(|c| c.id)
        .collect();

    let budgeted: Vec<fubbik_core::format::ChunkWithMetadata> = budgeted_ids
        .into_iter()
        .filter_map(|id| meta_by_id.remove(&id))
        .collect();

    format_structured(budgeted)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/context/for-plan", get(for_plan))
        .route("/api/context/about", get(about))
        .route("/api/context/for-files", get(for_files))
}
