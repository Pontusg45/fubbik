use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use fubbik_db::repo::tag_type::TagType;

use super::dto::{CreateTagTypeBody, MessageResponse, UpdateTagTypeBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(get, path = "/api/tag-types",
    responses((status = 200, body = Vec<TagType>)))]
pub async fn list_tag_types(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<TagType>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

#[utoipa::path(post, path = "/api/tag-types", request_body = CreateTagTypeBody,
    responses((status = 201, body = TagType)))]
pub async fn create_tag_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateTagTypeBody>,
) -> ApiResult<(StatusCode, Json<TagType>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/tag-types/{id}", request_body = UpdateTagTypeBody,
    params(("id" = String, Path,)), responses((status = 200, body = TagType), (status = 404)))]
pub async fn update_tag_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateTagTypeBody>,
) -> ApiResult<Json<TagType>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/tag-types/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_tag_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tag-types", get(list_tag_types).post(create_tag_type))
        .route(
            "/api/tag-types/{id}",
            patch(update_tag_type).delete(delete_tag_type),
        )
}
