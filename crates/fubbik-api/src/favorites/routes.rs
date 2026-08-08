use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use fubbik_db::repo::favorite::Favorite;

use super::dto::{CreateFavoriteBody, MessageResponse, ReorderEntry};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

#[utoipa::path(get, path = "/api/favorites",
    responses((status = 200, body = Vec<Favorite>)))]
pub async fn list_favorites(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<Favorite>>> {
    Ok(Json(service::list(&state.pool, &user.id).await?))
}

/// Status is set to 201 unconditionally, even when the chunk was already
/// favorited and the body is `null` — matching Node's `Effect.tap` block,
/// which sets `ctx.set.status = 201` before ever inspecting the result
/// (`packages/api/src/favorites/routes.ts:11-29`, `_mutating.md`). This is
/// a deliberately preserved quirk, not a bug: do not "improve" it to
/// 200/409/an idempotent echo of the existing row.
#[utoipa::path(post, path = "/api/favorites", request_body = CreateFavoriteBody,
    responses((status = 201, body = Option<Favorite>), (status = 404)))]
pub async fn add_favorite(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateFavoriteBody>,
) -> ApiResult<(StatusCode, Json<Option<Favorite>>)> {
    let favorite = service::add(&state.pool, &user.id, &body.chunk_id).await?;
    Ok((StatusCode::CREATED, Json(favorite)))
}

/// Always 200 with `{"message":"Deleted"}`, even if the chunk was never
/// favorited by this caller — Node's `removeFavorite` has no 404 path at
/// all (`_mutating.md`: "no way to distinguish 'deleted something' from
/// 'there was nothing to delete'"). Reproduced faithfully, not "fixed".
#[utoipa::path(delete, path = "/api/favorites/{chunkId}", params(("chunkId" = String, Path,)),
    responses((status = 200, body = MessageResponse)))]
pub async fn remove_favorite(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(chunk_id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::remove(&state.pool, &user.id, &chunk_id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

#[utoipa::path(put, path = "/api/favorites/reorder", request_body = Vec<ReorderEntry>,
    responses((status = 200, body = MessageResponse)))]
pub async fn reorder_favorites(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<Vec<ReorderEntry>>,
) -> ApiResult<Json<MessageResponse>> {
    service::reorder(&state.pool, &user.id, body).await?;
    Ok(Json(MessageResponse {
        message: "Reordered".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/favorites", get(list_favorites).post(add_favorite))
        .route("/api/favorites/reorder", put(reorder_favorites))
        .route(
            "/api/favorites/{chunkId}",
            axum::routing::delete(remove_favorite),
        )
}
