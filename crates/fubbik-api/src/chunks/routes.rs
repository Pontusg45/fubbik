use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::AppResult;
use fubbik_db::repo::chunk::Chunk;
use fubbik_db::repo::chunk_meta::{self, AppliesTo, FileRef};

use super::dto::{CreateChunkBody, ListChunksQuery, UpdateChunkBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;

#[utoipa::path(
    get, path = "/api/chunks", params(ListChunksQuery),
    responses((status = 200, body = Vec<Chunk>))
)]
pub async fn list_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListChunksQuery>,
) -> AppResult<Json<Vec<Chunk>>> {
    Ok(Json(service::list(&state.pool, &user.id, query.into_params()).await?))
}

#[utoipa::path(post, path = "/api/chunks", request_body = CreateChunkBody,
    responses((status = 200, body = Chunk)))]
pub async fn create_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateChunkBody>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::create(&state.pool, &user.id, body).await?))
}

#[utoipa::path(get, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = Chunk), (status = 404)))]
pub async fn get_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::get(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/chunks/{id}", request_body = UpdateChunkBody,
    params(("id" = String, Path,)), responses((status = 200, body = Chunk), (status = 404)))]
pub async fn update_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateChunkBody>,
) -> AppResult<Json<Chunk>> {
    Ok(Json(service::update(&state.pool, &user.id, &id, body).await?))
}

#[utoipa::path(delete, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200), (status = 404)))]
pub async fn delete_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

#[utoipa::path(get, path = "/api/chunks/{id}/history", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<fubbik_db::repo::chunk_version::ChunkVersion>)))]
pub async fn chunk_history(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<fubbik_db::repo::chunk_version::ChunkVersion>>> {
    Ok(Json(service::history(&state.pool, &user.id, &id).await?))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct PatternsBody {
    pub patterns: Vec<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct PathsBody {
    pub paths: Vec<String>,
}

#[utoipa::path(get, path = "/api/chunks/{id}/applies-to", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<AppliesTo>)))]
pub async fn get_applies_to(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<AppliesTo>>> {
    service::get(&state.pool, &user.id, &id).await?;
    Ok(Json(chunk_meta::get_applies_to(&state.pool, &id).await?))
}

#[utoipa::path(put, path = "/api/chunks/{id}/applies-to", request_body = PatternsBody,
    params(("id" = String, Path,)), responses((status = 200, body = Vec<AppliesTo>)))]
pub async fn put_applies_to(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<PatternsBody>,
) -> AppResult<Json<Vec<AppliesTo>>> {
    service::get(&state.pool, &user.id, &id).await?;
    chunk_meta::replace_applies_to(&state.pool, &id, &body.patterns).await?;
    Ok(Json(chunk_meta::get_applies_to(&state.pool, &id).await?))
}

#[utoipa::path(get, path = "/api/chunks/{id}/file-refs", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<FileRef>)))]
pub async fn get_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<FileRef>>> {
    service::get(&state.pool, &user.id, &id).await?;
    Ok(Json(chunk_meta::get_file_refs(&state.pool, &id).await?))
}

#[utoipa::path(put, path = "/api/chunks/{id}/file-refs", request_body = PathsBody,
    params(("id" = String, Path,)), responses((status = 200, body = Vec<FileRef>)))]
pub async fn put_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<PathsBody>,
) -> AppResult<Json<Vec<FileRef>>> {
    service::get(&state.pool, &user.id, &id).await?;
    chunk_meta::replace_file_refs(&state.pool, &id, &body.paths).await?;
    Ok(Json(chunk_meta::get_file_refs(&state.pool, &id).await?))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks", get(list_chunks).post(create_chunk))
        .route(
            "/api/chunks/{id}",
            get(get_chunk).patch(update_chunk).delete(delete_chunk),
        )
        .route("/api/chunks/{id}/history", get(chunk_history))
        .route("/api/chunks/{id}/applies-to", get(get_applies_to).put(put_applies_to))
        .route("/api/chunks/{id}/file-refs", get(get_file_refs).put(put_file_refs))
}
