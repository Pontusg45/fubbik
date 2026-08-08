use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::collection::Collection;

use super::dto::{CreateCollectionBody, MessageResponse, UpdateCollectionBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::chunks::dto::ChunkListResponse;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(get, path = "/api/collections",
    responses((status = 200, body = Vec<Collection>)))]
pub async fn list_collections(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<Collection>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

#[utoipa::path(post, path = "/api/collections", request_body = CreateCollectionBody,
    responses((status = 201, body = Collection), (status = 404)))]
pub async fn create_collection(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateCollectionBody>,
) -> ApiResult<(StatusCode, Json<Collection>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/collections/{id}", request_body = UpdateCollectionBody,
    params(("id" = String, Path,)), responses((status = 200, body = Collection), (status = 404)))]
pub async fn update_collection(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateCollectionBody>,
) -> ApiResult<Json<Collection>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/collections/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_collection(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

/// Returns the chunks `{chunks, total, limit, offset}` envelope, NOT a bare
/// array — the one endpoint in this slice that inherits the chunks
/// envelope by delegating to the same `chunks::service::list` that backs
/// `GET /api/chunks`. See `service::get_chunks`'s doc comment.
#[utoipa::path(get, path = "/api/collections/{id}/chunks", params(("id" = String, Path,)),
    responses((status = 200, body = ChunkListResponse), (status = 404)))]
pub async fn get_collection_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ChunkListResponse>> {
    Ok(Json(service::get_chunks(&state.pool, &user.id, &id).await?))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/collections",
            get(list_collections).post(create_collection),
        )
        .route(
            "/api/collections/{id}",
            axum::routing::patch(update_collection).delete(delete_collection),
        )
        .route("/api/collections/{id}/chunks", get(get_collection_chunks))
}
