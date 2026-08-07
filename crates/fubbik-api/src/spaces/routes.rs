use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::space::{ResetResult, Space, SpaceDetail};

use super::dto::{CreateSpaceBody, DetectQuery, MessageResponse, UpdateSpaceBody};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::extract::Query;

/// `GET /api/spaces/detect` returns a bare [`Space`] on a match, but on no
/// match Node returns a genuinely empty HTTP body — `Content-Length: 0`, no
/// `content-type` header at all (not `null`, not `{}`, not 404). See
/// `tests/fixtures/node-contract/spaces-detect-nomatch.json` and
/// `_questions.md` ("Null vs. empty-array / empty-object conventions").
/// `()`'s `IntoResponse` impl in axum produces exactly that shape, so
/// `NotFound` delegates to it rather than emitting `Json(null)` or
/// `StatusCode::NOT_FOUND`, either of which would diverge from the captured
/// contract.
pub enum DetectResponse {
    Found(Space),
    NotFound,
}

impl IntoResponse for DetectResponse {
    fn into_response(self) -> Response {
        match self {
            DetectResponse::Found(space) => Json(space).into_response(),
            DetectResponse::NotFound => ().into_response(),
        }
    }
}

#[utoipa::path(get, path = "/api/spaces/detect", params(DetectQuery),
    responses((status = 200, body = Space), (status = 200, description = "no match: empty body")))]
pub async fn detect_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<DetectQuery>,
) -> ApiResult<DetectResponse> {
    Ok(match service::detect(&state.pool, &user.id, query).await? {
        Some(space) => DetectResponse::Found(space),
        None => DetectResponse::NotFound,
    })
}

#[utoipa::path(get, path = "/api/spaces",
    responses((status = 200, body = Vec<Space>)))]
pub async fn list_spaces(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<Space>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

#[utoipa::path(post, path = "/api/spaces", request_body = CreateSpaceBody,
    responses((status = 201, body = Space), (status = 400)))]
pub async fn create_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateSpaceBody>,
) -> ApiResult<(StatusCode, Json<Space>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(get, path = "/api/spaces/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = SpaceDetail), (status = 404)))]
pub async fn get_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<SpaceDetail>> {
    Ok(Json(service::get(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/spaces/{id}", request_body = UpdateSpaceBody,
    params(("id" = String, Path,)), responses((status = 200, body = Space), (status = 404)))]
pub async fn update_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateSpaceBody>,
) -> ApiResult<Json<Space>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(post, path = "/api/spaces/{id}/reset", params(("id" = String, Path,)),
    responses((status = 200, body = ResetResult), (status = 404)))]
pub async fn reset_space(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ResetResult>> {
    Ok(Json(service::reset(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(delete, path = "/api/spaces/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_space(
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
        .route("/api/spaces/detect", get(detect_space))
        .route("/api/spaces", get(list_spaces).post(create_space))
        .route(
            "/api/spaces/{id}",
            get(get_space).patch(update_space).delete(delete_space),
        )
        .route("/api/spaces/{id}/reset", post(reset_space))
}
