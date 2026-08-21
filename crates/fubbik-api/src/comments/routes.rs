//! `/api/chunks/{id}/comments` and `/api/comments/{id}`.
//!
//! Two different owners guard this domain — reading a thread is gated on
//! owning the chunk, editing a comment on having written it. See
//! `fubbik_db::repo::comment`'s module doc; the guards are in SQL.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::comment::{self, ChunkComment};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CommentBody {
    pub content: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CommentMessage {
    pub message: String,
}

/// Node caps comment length at 5000 on its route schema; enforced here
/// because Elysia's check has no service-layer equivalent to port.
fn check_content(content: &str) -> AppResult<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("content is required".into()));
    }
    if trimmed.chars().count() > 5000 {
        return Err(AppError::Validation(
            "content must be at most 5000 characters".into(),
        ));
    }
    Ok(trimmed.to_string())
}

#[utoipa::path(get, path = "/api/chunks/{id}/comments", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<ChunkComment>)))]
pub async fn list_comments(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<ChunkComment>>> {
    Ok(Json(comment::list(&state.pool, &id, &user.id).await?))
}

#[utoipa::path(post, path = "/api/chunks/{id}/comments", request_body = CommentBody,
    params(("id" = String, Path,)),
    responses((status = 201, body = ChunkComment), (status = 400), (status = 404)))]
pub async fn create_comment(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<CommentBody>,
) -> ApiResult<(StatusCode, Json<ChunkComment>)> {
    let content = check_content(&body.content)?;
    let created = comment::create(&state.pool, &id, &user.id, &content)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(patch, path = "/api/comments/{id}", request_body = CommentBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = ChunkComment), (status = 400), (status = 404)))]
pub async fn update_comment(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<CommentBody>,
) -> ApiResult<Json<ChunkComment>> {
    let content = check_content(&body.content)?;
    Ok(Json(
        comment::update(&state.pool, &id, &user.id, &content)
            .await?
            .ok_or_else(|| AppError::NotFound("comment".into()))?,
    ))
}

#[utoipa::path(delete, path = "/api/comments/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = CommentMessage), (status = 404)))]
pub async fn delete_comment(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<CommentMessage>> {
    comment::delete(&state.pool, &id, &user.id)
        .await?
        .ok_or_else(|| AppError::NotFound("comment".into()))?;
    Ok(Json(CommentMessage {
        message: "Deleted".into(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/chunks/{id}/comments",
            get(list_comments).post(create_comment),
        )
        .route(
            "/api/comments/{id}",
            axum::routing::patch(update_comment).delete(delete_comment),
        )
}
