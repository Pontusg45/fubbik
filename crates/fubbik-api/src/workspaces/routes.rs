use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::workspace::{Workspace, WorkspaceSpaceLink};

use super::dto::{
    AddSpaceBody, CreateWorkspaceBody, MessageResponse, UpdateWorkspaceBody, WorkspaceDetail,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(get, path = "/api/workspaces",
    responses((status = 200, body = Vec<Workspace>)))]
pub async fn list_workspaces(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<Workspace>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

#[utoipa::path(post, path = "/api/workspaces", request_body = CreateWorkspaceBody,
    responses((status = 201, body = Workspace), (status = 400)))]
pub async fn create_workspace(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateWorkspaceBody>,
) -> ApiResult<(StatusCode, Json<Workspace>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(get, path = "/api/workspaces/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = WorkspaceDetail), (status = 404)))]
pub async fn get_workspace(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<WorkspaceDetail>> {
    Ok(Json(service::get_detail(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/workspaces/{id}", request_body = UpdateWorkspaceBody,
    params(("id" = String, Path,)), responses((status = 200, body = Workspace), (status = 400), (status = 404)))]
pub async fn update_workspace(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateWorkspaceBody>,
) -> ApiResult<Json<Workspace>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/workspaces/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_workspace(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

#[utoipa::path(post, path = "/api/workspaces/{id}/spaces", request_body = AddSpaceBody,
    params(("id" = String, Path,)), responses((status = 201, body = WorkspaceSpaceLink), (status = 404)))]
pub async fn add_workspace_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<AddSpaceBody>,
) -> ApiResult<(StatusCode, Json<WorkspaceSpaceLink>)> {
    let link = service::add_space_to_workspace(&state.pool, &user.id, &id, &body.space_id).await?;
    Ok((StatusCode::CREATED, Json(link)))
}

#[utoipa::path(delete, path = "/api/workspaces/{id}/spaces/{spaceId}",
    params(("id" = String, Path,), ("spaceId" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn remove_workspace_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, space_id)): Path<(String, String)>,
) -> ApiResult<Json<MessageResponse>> {
    service::remove_space_from_workspace(&state.pool, &user.id, &id, &space_id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/api/workspaces/{id}",
            get(get_workspace)
                .patch(update_workspace)
                .delete(delete_workspace),
        )
        .route("/api/workspaces/{id}/spaces", post(add_workspace_space))
        .route(
            "/api/workspaces/{id}/spaces/{spaceId}",
            axum::routing::delete(remove_workspace_space),
        )
}
