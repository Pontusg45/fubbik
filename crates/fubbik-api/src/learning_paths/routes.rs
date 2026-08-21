//! `/api/learning-paths` — ordered reading lists of chunks.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::learning_path::{self, LearningPath, LearningPathPatch, NewLearningPath};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLearningPathBody {
    pub title: String,
    pub description: Option<String>,
    pub chunk_ids: Vec<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLearningPathBody {
    pub title: Option<String>,
    /// Two-state, not tri-state — Node's body has no null variant, so a
    /// description cannot be cleared once set. Reproduced.
    pub description: Option<String>,
    /// `[]` empties the path; omitting the key leaves it alone.
    pub chunk_ids: Option<Vec<String>>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct LearningPathMessage {
    pub message: String,
}

fn check_title(title: &str) -> AppResult<String> {
    let t = title.trim();
    if t.is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if t.chars().count() > 200 {
        return Err(AppError::Validation(
            "title must be at most 200 characters".into(),
        ));
    }
    Ok(t.to_string())
}

#[utoipa::path(get, path = "/api/learning-paths",
    responses((status = 200, body = Vec<LearningPath>)))]
pub async fn list_learning_paths(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<LearningPath>>> {
    Ok(Json(learning_path::list(&state.pool, &user.id).await?))
}

#[utoipa::path(get, path = "/api/learning-paths/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = LearningPath), (status = 404)))]
pub async fn get_learning_path(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<LearningPath>> {
    Ok(Json(
        learning_path::find_by_id(&state.pool, &id, &user.id)
            .await?
            .ok_or_else(|| AppError::NotFound("learning path".into()))?,
    ))
}

/// A `chunkIds` list containing an id the caller does not own is rejected
/// whole — reported as a validation error rather than a 404, because what
/// was not found is a chunk named in the body, not the path being created.
#[utoipa::path(post, path = "/api/learning-paths", request_body = CreateLearningPathBody,
    responses((status = 201, body = LearningPath), (status = 400)))]
pub async fn create_learning_path(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateLearningPathBody>,
) -> ApiResult<(StatusCode, Json<LearningPath>)> {
    let title = check_title(&body.title)?;
    let created = learning_path::create(
        &state.pool,
        &user.id,
        NewLearningPath {
            title,
            description: body.description,
            chunk_ids: body.chunk_ids,
        },
    )
    .await?
    .ok_or_else(|| AppError::Validation("chunkIds contains a chunk that is not yours".into()))?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(patch, path = "/api/learning-paths/{id}",
    request_body = UpdateLearningPathBody, params(("id" = String, Path,)),
    responses((status = 200, body = LearningPath), (status = 400), (status = 404)))]
pub async fn update_learning_path(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateLearningPathBody>,
) -> ApiResult<Json<LearningPath>> {
    let title = body.title.as_deref().map(check_title).transpose()?;
    // The UPDATE returns no row for two different reasons — the path is not
    // the caller's, or `chunkIds` named a chunk that is not. Re-reading the
    // path tells them apart, so a bad id list is a 400 rather than a
    // confusing 404. Same shape as the matrices cell surface.
    let touched_ids = body.chunk_ids.is_some();
    match learning_path::update(
        &state.pool,
        &id,
        &user.id,
        LearningPathPatch {
            title,
            description: body.description,
            chunk_ids: body.chunk_ids,
        },
    )
    .await?
    {
        Some(updated) => Ok(Json(updated)),
        None => {
            let exists = learning_path::find_by_id(&state.pool, &id, &user.id)
                .await?
                .is_some();
            if exists && touched_ids {
                Err(
                    AppError::Validation("chunkIds contains a chunk that is not yours".into())
                        .into(),
                )
            } else {
                Err(AppError::NotFound("learning path".into()).into())
            }
        }
    }
}

#[utoipa::path(delete, path = "/api/learning-paths/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = LearningPathMessage), (status = 404)))]
pub async fn delete_learning_path(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<LearningPathMessage>> {
    if !learning_path::delete(&state.pool, &id, &user.id).await? {
        return Err(AppError::NotFound("learning path".into()).into());
    }
    Ok(Json(LearningPathMessage {
        message: "Deleted".into(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/learning-paths",
            get(list_learning_paths).post(create_learning_path),
        )
        .route(
            "/api/learning-paths/{id}",
            get(get_learning_path)
                .patch(update_learning_path)
                .delete(delete_learning_path),
        )
}
