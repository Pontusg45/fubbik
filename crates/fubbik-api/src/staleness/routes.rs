use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::staleness::{RawUpdateResult, StaleFlag};

use super::dto::{
    CountQuery, ListStaleQuery, ScanAgeBody, ScanImpactBody, ScanResult, SuppressDuplicateBody,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::{Json as ReqJson, Query};

#[utoipa::path(get, path = "/api/chunks/stale", params(ListStaleQuery),
    responses((status = 200, body = Vec<StaleFlag>)))]
pub async fn list_stale(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListStaleQuery>,
) -> ApiResult<Json<Vec<StaleFlag>>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.into_params()).await?,
    ))
}

/// Returns a **bare number**, not `{"count": N}` — Node's `getStaleCount`
/// resolves the route handler to a plain `number`, and Elysia serialises a
/// primitive response as `text/plain`, not JSON
/// (`tests/fixtures/node-contract-2c/chunks-stale-count.json`: literally
/// `0`). `String`'s `IntoResponse` impl is what axum uses to produce a
/// `text/plain; charset=utf-8` body here, matching that content type —
/// `Json<i64>` would emit the right bytes but the wrong content-type.
#[utoipa::path(get, path = "/api/chunks/stale/count", params(CountQuery),
    responses((status = 200, body = i64, content_type = "text/plain")))]
pub async fn stale_count(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<CountQuery>,
) -> ApiResult<String> {
    let n = service::count(&state.pool, &user.id, query.space_id.as_deref()).await?;
    Ok(n.to_string())
}

#[utoipa::path(post, path = "/api/chunks/{id}/dismiss-staleness",
    params(("id" = String, Path,)),
    responses((status = 200, body = RawUpdateResult), (status = 404)))]
pub async fn dismiss_staleness(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<RawUpdateResult>> {
    Ok(Json(service::dismiss(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(post, path = "/api/chunks/suppress-duplicate", request_body = SuppressDuplicateBody,
    responses((status = 200, body = RawUpdateResult), (status = 404)))]
pub async fn suppress_duplicate(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<SuppressDuplicateBody>,
) -> ApiResult<Json<RawUpdateResult>> {
    Ok(Json(
        service::suppress_duplicate(&state.pool, &user.id, &body.chunk_id_a, &body.chunk_id_b)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/chunks/stale/scan-age", request_body = ScanAgeBody,
    responses((status = 200, body = ScanResult)))]
pub async fn scan_age(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ScanAgeBody>,
) -> ApiResult<Json<ScanResult>> {
    let flagged = service::scan_age(
        &state.pool,
        &user.id,
        body.space_id.as_deref(),
        body.threshold_days,
    )
    .await?;
    Ok(Json(ScanResult { flagged }))
}

/// `title` falls back to `"Unknown"` when absent — matches Node's
/// `ctx.body.title ?? "Unknown"` (`packages/api/src/staleness/routes.ts:98`).
#[utoipa::path(post, path = "/api/chunks/{id}/scan-impact",
    params(("id" = String, Path,)), request_body = ScanImpactBody,
    responses((status = 200, body = ScanResult), (status = 404)))]
pub async fn scan_impact(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<ScanImpactBody>,
) -> ApiResult<Json<ScanResult>> {
    let title = body.title.as_deref().unwrap_or("Unknown");
    let flagged = service::scan_impact(&state.pool, &user.id, &id, title).await?;
    Ok(Json(ScanResult { flagged }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks/stale", get(list_stale))
        .route("/api/chunks/stale/count", get(stale_count))
        .route("/api/chunks/stale/scan-age", post(scan_age))
        .route(
            "/api/chunks/{id}/dismiss-staleness",
            post(dismiss_staleness),
        )
        .route("/api/chunks/suppress-duplicate", post(suppress_duplicate))
        .route("/api/chunks/{id}/scan-impact", post(scan_impact))
}
