use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

use super::dependency_service;
use super::dto::{AddDependencyBody, DependencyGraph, DependencySides, MessageResponse};
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(post, path = "/api/requirements/{id}/dependencies", request_body = AddDependencyBody,
    params(("id" = String, Path,)),
    responses((status = 201, body = MessageResponse), (status = 400), (status = 404)))]
pub async fn add_dependency(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<AddDependencyBody>,
) -> ApiResult<(StatusCode, Json<MessageResponse>)> {
    dependency_service::add_dependency(&state.pool, &user.id, &id, &body.depends_on_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(MessageResponse {
            message: "Dependency added".to_string(),
        }),
    ))
}

#[utoipa::path(delete, path = "/api/requirements/{id}/dependencies/{dependsOnId}",
    params(("id" = String, Path,), ("dependsOnId" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn remove_dependency(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, depends_on_id)): Path<(String, String)>,
) -> ApiResult<Json<MessageResponse>> {
    dependency_service::remove_dependency(&state.pool, &user.id, &id, &depends_on_id).await?;
    Ok(Json(MessageResponse {
        message: "Dependency removed".to_string(),
    }))
}

#[utoipa::path(get, path = "/api/requirements/{id}/dependencies", params(("id" = String, Path,)),
    responses((status = 200, body = DependencySides), (status = 404)))]
pub async fn get_dependencies(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<DependencySides>> {
    Ok(Json(
        dependency_service::get_dependencies(&state.pool, &user.id, &id).await?,
    ))
}

#[utoipa::path(get, path = "/api/requirements/{id}/dependencies/graph", params(("id" = String, Path,)),
    responses((status = 200, body = DependencyGraph), (status = 404)))]
pub async fn get_dependency_graph(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<DependencyGraph>> {
    Ok(Json(
        dependency_service::get_dependency_graph(&state.pool, &user.id, &id).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/requirements/{id}/dependencies",
            get(get_dependencies).post(add_dependency),
        )
        .route(
            "/api/requirements/{id}/dependencies/graph",
            get(get_dependency_graph),
        )
        .route(
            "/api/requirements/{id}/dependencies/{dependsOnId}",
            axum::routing::delete(remove_dependency),
        )
}
