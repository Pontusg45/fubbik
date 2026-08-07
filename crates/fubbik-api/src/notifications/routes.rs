use axum::extract::{Path, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use fubbik_db::repo::notification::Notification;

use super::dto::{CountResponse, ListNotificationsQuery, MessageResponse};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
// Imported under its plain name so utoipa's `axum_extras` feature can infer
// this parameter is a query param by pattern-matching the literal
// `Query<T>` identifier — see the comment on `chunks::dto::ListChunksQuery`.
use crate::extract::Query;

#[utoipa::path(
    get, path = "/api/notifications", params(ListNotificationsQuery),
    responses((status = 200, body = Vec<Notification>))
)]
pub async fn list_notifications(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListNotificationsQuery>,
) -> ApiResult<Json<Vec<Notification>>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.unread_only(), query.limit()).await?,
    ))
}

#[utoipa::path(get, path = "/api/notifications/count",
    responses((status = 200, body = CountResponse)))]
pub async fn unread_count(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<CountResponse>> {
    Ok(Json(CountResponse {
        count: service::count_unread(&state.pool, &user.id).await?,
    }))
}

#[utoipa::path(post, path = "/api/notifications/read-all",
    responses((status = 200, body = MessageResponse)))]
pub async fn mark_all_read(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<MessageResponse>> {
    service::mark_all_read(&state.pool, &user.id).await?;
    Ok(Json(MessageResponse {
        message: "All marked as read".to_string(),
    }))
}

#[utoipa::path(patch, path = "/api/notifications/{id}/read",
    params(("id" = String, Path,)),
    responses((status = 200, body = Notification), (status = 404)))]
pub async fn mark_notification_read(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Notification>> {
    Ok(Json(service::mark_read(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(delete, path = "/api/notifications/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_notification(
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
        .route("/api/notifications", get(list_notifications))
        .route("/api/notifications/count", get(unread_count))
        .route("/api/notifications/read-all", post(mark_all_read))
        .route(
            "/api/notifications/{id}/read",
            patch(mark_notification_read),
        )
        .route("/api/notifications/{id}", delete(delete_notification))
}
