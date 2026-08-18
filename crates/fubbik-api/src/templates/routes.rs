use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use fubbik_db::repo::template::Template;

use super::dto::{CreateTemplateBody, MessageResponse, UpdateTemplateBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(get, path = "/api/templates",
    responses((status = 200, body = Vec<Template>)))]
pub async fn list_templates(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<Template>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

#[utoipa::path(post, path = "/api/templates", request_body = CreateTemplateBody,
    responses((status = 201, body = Template)))]
pub async fn create_template(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateTemplateBody>,
) -> ApiResult<(StatusCode, Json<Template>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/templates/{id}", request_body = UpdateTemplateBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = Template), (status = 400), (status = 404)))]
pub async fn update_template(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateTemplateBody>,
) -> ApiResult<Json<Template>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/templates/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 400), (status = 404)))]
pub async fn delete_template(
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
        .route(
            "/api/templates",
            axum::routing::get(list_templates).post(create_template),
        )
        .route(
            "/api/templates/{id}",
            axum::routing::patch(update_template).delete(delete_template),
        )
}
