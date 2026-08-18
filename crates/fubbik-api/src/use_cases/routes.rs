use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::use_case::{UseCase, UseCaseListItem, UseCaseRequirement};

use super::dto::{CreateUseCaseBody, ListUseCasesQuery, MessageResponse, UpdateUseCaseBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
// Imported under its plain name so utoipa's `axum_extras` feature can infer
// this parameter is a query param by pattern-matching the literal
// `Query<T>` identifier — see the comment on `chunks::dto::ListChunksQuery`.
use crate::extract::Query;

/// Bare array, matching Node's `listUseCases` return
/// (`packages/api/src/use-cases/routes.ts:9-16`) — not the `{chunks,...}`
/// envelope, same shape convention as `notifications`/`workspaces`.
#[utoipa::path(get, path = "/api/use-cases", params(ListUseCasesQuery),
    responses((status = 200, body = Vec<UseCaseListItem>)))]
pub async fn list_use_cases(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListUseCasesQuery>,
) -> ApiResult<Json<Vec<UseCaseListItem>>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

#[utoipa::path(post, path = "/api/use-cases", request_body = CreateUseCaseBody,
    responses((status = 201, body = UseCase), (status = 404), (status = 400)))]
pub async fn create_use_case(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateUseCaseBody>,
) -> ApiResult<(StatusCode, Json<UseCase>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

/// Bare array of full `requirement` rows — see
/// `fubbik_db::repo::use_case::UseCaseRequirement`'s doc comment for why
/// this projection lives in the `use_case` repo module rather than a
/// `requirement` one.
#[utoipa::path(get, path = "/api/use-cases/{id}/requirements",
    params(("id" = String, Path,)),
    responses((status = 200, body = Vec<UseCaseRequirement>), (status = 404)))]
pub async fn use_case_requirements(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<UseCaseRequirement>>> {
    Ok(Json(
        service::get_requirements(&state.pool, &user.id, &id).await?,
    ))
}

#[utoipa::path(patch, path = "/api/use-cases/{id}", request_body = UpdateUseCaseBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = UseCase), (status = 404), (status = 400)))]
pub async fn update_use_case(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateUseCaseBody>,
) -> ApiResult<Json<UseCase>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/use-cases/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_use_case(
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
        .route("/api/use-cases", get(list_use_cases).post(create_use_case))
        .route(
            "/api/use-cases/{id}/requirements",
            get(use_case_requirements),
        )
        .route(
            "/api/use-cases/{id}",
            axum::routing::patch(update_use_case).delete(delete_use_case),
        )
}
