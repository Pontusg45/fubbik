use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use fubbik_db::repo::chunk_type::ChunkType;
use fubbik_db::repo::connection_relation::ConnectionRelation;

use super::dto::{
    ChunkTypesQuery, ConnectionRelationsQuery, CreateChunkTypeBody, CreateConnectionRelationBody,
    MessageResponse, UpdateChunkTypeBody, UpdateConnectionRelationBody,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
// Imported under its plain name so utoipa's `axum_extras` feature can infer
// this parameter is a query param by pattern-matching the literal
// `Query<T>` identifier — see the comment on `chunks::dto::ListChunksQuery`.
use crate::extract::Query;

// --- chunk types ------------------------------------------------------------

#[utoipa::path(get, path = "/api/chunk-types", params(ChunkTypesQuery),
    responses((status = 200, body = Vec<ChunkType>)))]
pub async fn list_chunk_types(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ChunkTypesQuery>,
) -> ApiResult<Json<Vec<ChunkType>>> {
    Ok(Json(
        service::list_chunk_types(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

#[utoipa::path(post, path = "/api/chunk-types", request_body = CreateChunkTypeBody,
    responses((status = 201, body = ChunkType), (status = 400)))]
pub async fn create_chunk_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateChunkTypeBody>,
) -> ApiResult<(StatusCode, Json<ChunkType>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create_chunk_type(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/chunk-types/{id}", request_body = UpdateChunkTypeBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = ChunkType), (status = 400), (status = 404)))]
pub async fn update_chunk_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateChunkTypeBody>,
) -> ApiResult<Json<ChunkType>> {
    Ok(Json(
        service::update_chunk_type(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/chunk-types/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 400), (status = 404)))]
pub async fn delete_chunk_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_chunk_type(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

// --- connection relations ---------------------------------------------------

#[utoipa::path(get, path = "/api/connection-relations", params(ConnectionRelationsQuery),
    responses((status = 200, body = Vec<ConnectionRelation>)))]
pub async fn list_connection_relations(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ConnectionRelationsQuery>,
) -> ApiResult<Json<Vec<ConnectionRelation>>> {
    Ok(Json(
        service::list_connection_relations(&state.pool, &user.id, query.space_id.as_deref())
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/connection-relations",
    request_body = CreateConnectionRelationBody,
    responses((status = 201, body = ConnectionRelation), (status = 400)))]
pub async fn create_connection_relation(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateConnectionRelationBody>,
) -> ApiResult<(StatusCode, Json<ConnectionRelation>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create_connection_relation(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/connection-relations/{id}",
    request_body = UpdateConnectionRelationBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = ConnectionRelation), (status = 400), (status = 404)))]
pub async fn update_connection_relation(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateConnectionRelationBody>,
) -> ApiResult<Json<ConnectionRelation>> {
    Ok(Json(
        service::update_connection_relation(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/connection-relations/{id}",
    params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 400), (status = 404)))]
pub async fn delete_connection_relation(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_connection_relation(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/chunk-types",
            axum::routing::get(list_chunk_types).post(create_chunk_type),
        )
        .route(
            "/api/chunk-types/{id}",
            axum::routing::patch(update_chunk_type).delete(delete_chunk_type),
        )
        .route(
            "/api/connection-relations",
            axum::routing::get(list_connection_relations).post(create_connection_relation),
        )
        .route(
            "/api/connection-relations/{id}",
            axum::routing::patch(update_connection_relation).delete(delete_connection_relation),
        )
}
