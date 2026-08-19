//! HTTP surface for the `features` domain — the 13 endpoints of
//! `packages/api/src/features/routes.ts`.
//!
//! Three of them hang off `/api/chunks/...` rather than `/api/features/...`
//! (the delta CRUD), which is why this router owns routes in another
//! domain's path space; Node's `featureRoutes` does the same.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use fubbik_db::repo::feature::{
    ChunkFeatureDelta, DeltaWithChunk, DeltaWithFeature, Feature, FeatureListItem,
};

use super::dto::{
    CreateFeatureBody, FeatureDetail, ListFeaturesQuery, MessageResponse, ReorderFeatureBody,
    SetActiveFeaturesBody, UpdateFeatureBody, UpsertDeltaBody,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::extract::Query;

#[utoipa::path(get, path = "/api/features", params(ListFeaturesQuery),
    responses((status = 200, body = Vec<FeatureListItem>)))]
pub async fn list_features(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListFeaturesQuery>,
) -> ApiResult<Json<Vec<FeatureListItem>>> {
    Ok(Json(
        service::list_features(&state.pool, &user.id, query).await?,
    ))
}

/// 201, matching Node's explicit `ctx.set.status = 201`
/// (`packages/api/src/features/routes.ts:36-40`) — the only endpoint in
/// this domain that is not 200.
#[utoipa::path(post, path = "/api/features", request_body = CreateFeatureBody,
    responses((status = 201, body = Feature)))]
pub async fn create_feature(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateFeatureBody>,
) -> ApiResult<(StatusCode, Json<Feature>)> {
    let created = service::create_feature(&state.pool, &user.id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// A bare array of feature ids — not objects. Node maps
/// `rows.map(r => r.featureId)` before responding
/// (`packages/api/src/features/service.ts:145-147`).
#[utoipa::path(get, path = "/api/features/active",
    responses((status = 200, body = Vec<String>)))]
pub async fn get_active_features(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(
        service::get_active_features(&state.pool, &user.id).await?,
    ))
}

#[utoipa::path(put, path = "/api/features/active", request_body = SetActiveFeaturesBody,
    responses((status = 200, body = MessageResponse), (status = 400)))]
pub async fn set_active_features(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<SetActiveFeaturesBody>,
) -> ApiResult<Json<MessageResponse>> {
    service::set_active_features(&state.pool, &user.id, body.feature_ids).await?;
    Ok(Json(MessageResponse {
        message: "Active features updated".to_string(),
    }))
}

#[utoipa::path(get, path = "/api/features/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = FeatureDetail), (status = 404)))]
pub async fn get_feature(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<FeatureDetail>> {
    Ok(Json(
        service::get_feature_detail(&state.pool, &id, &user.id).await?,
    ))
}

#[utoipa::path(patch, path = "/api/features/{id}", params(("id" = String, Path,)),
    request_body = UpdateFeatureBody,
    responses((status = 200, body = Feature), (status = 400), (status = 404)))]
pub async fn update_feature(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateFeatureBody>,
) -> ApiResult<Json<Feature>> {
    Ok(Json(
        service::update_feature(&state.pool, &id, &user.id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/features/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_feature(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_feature(&state.pool, &id, &user.id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

/// `{ message: "Feature merged" }` — the merged chunk ids are deliberately
/// not surfaced; Node's route discards the service's return value with
/// `Effect.map(() => ({ message: "Feature merged" }))`.
#[utoipa::path(post, path = "/api/features/{id}/merge", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 400), (status = 404)))]
pub async fn merge_feature(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::merge_feature(&state.pool, &id, &user.id).await?;
    Ok(Json(MessageResponse {
        message: "Feature merged".to_string(),
    }))
}

/// Returns the reordered feature row itself, not a message.
#[utoipa::path(post, path = "/api/features/{id}/reorder", params(("id" = String, Path,)),
    request_body = ReorderFeatureBody,
    responses((status = 200, body = Feature), (status = 404)))]
pub async fn reorder_feature(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<ReorderFeatureBody>,
) -> ApiResult<Json<Feature>> {
    Ok(Json(
        service::reorder_feature(&state.pool, &id, &user.id, body.priority).await?,
    ))
}

#[utoipa::path(get, path = "/api/features/{id}/deltas", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<DeltaWithChunk>), (status = 404)))]
pub async fn feature_deltas(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<DeltaWithChunk>>> {
    Ok(Json(
        service::feature_deltas(&state.pool, &id, &user.id).await?,
    ))
}

/// Node's handler drops the session entirely
/// (`featureService.getDeltasForChunk(ctx.params.id)`), so any signed-in
/// caller can read any chunk's overlays there. This port passes the
/// caller's id down to the SQL, which returns an empty list for a chunk
/// the caller does not own — a deliberate, flagged divergence.
#[utoipa::path(get, path = "/api/chunks/{id}/deltas", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<DeltaWithFeature>)))]
pub async fn chunk_deltas(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<DeltaWithFeature>>> {
    Ok(Json(
        service::deltas_for_chunk(&state.pool, &id, &user.id).await?,
    ))
}

#[utoipa::path(put, path = "/api/chunks/{id}/deltas/{featureId}",
    params(("id" = String, Path,), ("featureId" = String, Path,)),
    request_body = UpsertDeltaBody,
    responses((status = 200, body = ChunkFeatureDelta), (status = 400), (status = 404)))]
pub async fn upsert_delta(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((chunk_id, feature_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<UpsertDeltaBody>,
) -> ApiResult<Json<ChunkFeatureDelta>> {
    Ok(Json(
        service::upsert_delta(&state.pool, &chunk_id, &feature_id, &user.id, body.delta).await?,
    ))
}

#[utoipa::path(delete, path = "/api/chunks/{id}/deltas/{featureId}",
    params(("id" = String, Path,), ("featureId" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_delta(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((chunk_id, feature_id)): Path<(String, String)>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_delta(&state.pool, &chunk_id, &feature_id, &user.id).await?;
    Ok(Json(MessageResponse {
        message: "Delta deleted".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/features", get(list_features).post(create_feature))
        // Registered before `/{id}` for readability only — axum 0.8 matches
        // a static segment ahead of a dynamic one regardless of order.
        .route(
            "/api/features/active",
            get(get_active_features).put(set_active_features),
        )
        .route(
            "/api/features/{id}",
            get(get_feature)
                .patch(update_feature)
                .delete(delete_feature),
        )
        .route("/api/features/{id}/merge", post(merge_feature))
        .route("/api/features/{id}/reorder", post(reorder_feature))
        .route("/api/features/{id}/deltas", get(feature_deltas))
        .route("/api/chunks/{id}/deltas", get(chunk_deltas))
        .route(
            "/api/chunks/{id}/deltas/{featureId}",
            put(upsert_delta).delete(delete_delta),
        )
}
