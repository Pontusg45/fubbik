use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::tag::{MergeResult, Tag, TagListItem};

use super::dto::{CreateTagBody, MergeBody, MessageResponse, UpdateTagBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(get, path = "/api/tags",
    responses((status = 200, body = Vec<TagListItem>)))]
pub async fn list_tags(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<TagListItem>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

#[utoipa::path(post, path = "/api/tags", request_body = CreateTagBody,
    responses((status = 201, body = Tag)))]
pub async fn create_tag(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateTagBody>,
) -> ApiResult<(StatusCode, Json<Tag>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/tags/{id}", request_body = UpdateTagBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = Tag), (status = 400), (status = 404)))]
pub async fn update_tag(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateTagBody>,
) -> ApiResult<Json<Tag>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/tags/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_tag(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

#[utoipa::path(post, path = "/api/tags/merge", request_body = MergeBody,
    responses((status = 200, body = MergeResult), (status = 400), (status = 404)))]
pub async fn merge_tags(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<MergeBody>,
) -> ApiResult<Json<MergeResult>> {
    Ok(Json(service::merge(&state.pool, &user.id, body).await?))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tags", get(list_tags).post(create_tag))
        .route(
            "/api/tags/{id}",
            axum::routing::patch(update_tag).delete(delete_tag),
        )
        .route("/api/tags/merge", post(merge_tags))
}
