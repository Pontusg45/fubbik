use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::dto::{CoverageQuery, CoverageResponse, TraceabilityQuery, TraceabilityRow};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

/// Both paths live under `/api/requirements/...` even though this is its own
/// domain module — that is Node's URL layout
/// (`packages/api/src/coverage/routes.ts:9,28`), mounted from a separate
/// `coverageRoutes` Elysia instance the same way. They are static segments,
/// so axum's router prefers them over `requirements::routes`' `/{id}`
/// (same arrangement `/api/requirements/stats` already relies on).
///
/// **One handler, two response shapes.** `?detail=true` adds a `matrix`
/// field; anything else — including `?detail=1` — omits the key entirely.
/// See `dto::CoverageResponse` and `dto::CoverageQuery::detail`.
#[utoipa::path(get, path = "/api/requirements/coverage", params(CoverageQuery),
    responses((status = 200, body = CoverageResponse)))]
pub async fn get_coverage(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<CoverageQuery>,
) -> ApiResult<Json<CoverageResponse>> {
    let codebase_id = query.codebase_id.as_deref();
    let response = if service::wants_detail(query.detail.as_deref()) {
        service::get_coverage_matrix(&state.pool, &user.id, codebase_id).await?
    } else {
        service::get_coverage(&state.pool, &user.id, codebase_id).await?
    };
    Ok(Json(response))
}

/// Returns a **bare array**, not an envelope — see `dto::TraceabilityRow`.
#[utoipa::path(get, path = "/api/requirements/traceability", params(TraceabilityQuery),
    responses((status = 200, body = Vec<TraceabilityRow>)))]
pub async fn get_traceability(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<TraceabilityQuery>,
) -> ApiResult<Json<Vec<TraceabilityRow>>> {
    Ok(Json(
        service::get_traceability(&state.pool, &user.id, query.codebase_id.as_deref()).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/requirements/coverage", get(get_coverage))
        .route("/api/requirements/traceability", get(get_traceability))
}
