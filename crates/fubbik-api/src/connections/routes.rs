use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use fubbik_db::repo::connection::Connection;

use super::dto::{CreateConnectionBody, MessageResponse};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(post, path = "/api/connections", request_body = CreateConnectionBody,
    responses((status = 201, body = Connection), (status = 400), (status = 404), (status = 409)))]
pub async fn create_connection(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateConnectionBody>,
) -> ApiResult<(StatusCode, Json<Connection>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(delete, path = "/api/connections/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_connection(
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
        .route("/api/connections", post(create_connection))
        .route(
            "/api/connections/{id}",
            axum::routing::delete(delete_connection),
        )
}
