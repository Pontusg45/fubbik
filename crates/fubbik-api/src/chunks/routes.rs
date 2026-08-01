use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::AppResult;
use fubbik_db::repo::chunk::Chunk;

use super::dto::{CreateChunkBody, ListChunksQuery, UpdateChunkBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;

#[utoipa::path(
    get, path = "/api/chunks", params(ListChunksQuery),
    responses((status = 200, body = Vec<Chunk>))
)]
async fn list_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListChunksQuery>,
) -> AppResult<Json<Vec<Chunk>>> {
    Ok(Json(service::list(&state.pool, &user.id, query.into_params()).await?))
}

#[utoipa::path(post, path = "/api/chunks", request_body = CreateChunkBody,
    responses((status = 200, body = Chunk)))]
async fn create_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateChunkBody>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::create(&state.pool, &user.id, body).await?))
}

#[utoipa::path(get, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = Chunk), (status = 404)))]
async fn get_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::get(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/chunks/{id}", request_body = UpdateChunkBody,
    params(("id" = String, Path,)), responses((status = 200, body = Chunk), (status = 404)))]
async fn update_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateChunkBody>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::update(&state.pool, &user.id, &id, body).await?))
}

#[utoipa::path(delete, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200), (status = 404)))]
async fn delete_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks", get(list_chunks).post(create_chunk))
        .route(
            "/api/chunks/{id}",
            get(get_chunk).patch(update_chunk).delete(delete_chunk),
        )
}
