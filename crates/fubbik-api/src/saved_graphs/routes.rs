use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::saved_graph::SavedGraph;

use super::dto::{
    CreateSavedGraphBody, ListSavedGraphsQuery, MessageResponse, UpdateSavedGraphBody,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
// Imported under its plain name so utoipa's `axum_extras` feature can infer
// this parameter is a query param by pattern-matching the literal
// `Query<T>` identifier — see the comment on `chunks::dto::ListChunksQuery`.
use crate::extract::Query;

#[utoipa::path(
    get, path = "/api/saved-graphs", params(ListSavedGraphsQuery),
    responses((status = 200, body = Vec<SavedGraph>))
)]
pub async fn list_saved_graphs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListSavedGraphsQuery>,
) -> ApiResult<Json<Vec<SavedGraph>>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

#[utoipa::path(post, path = "/api/saved-graphs", request_body = CreateSavedGraphBody,
    responses((status = 201, body = SavedGraph), (status = 400)))]
pub async fn create_saved_graph(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateSavedGraphBody>,
) -> ApiResult<(StatusCode, Json<SavedGraph>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(get, path = "/api/saved-graphs/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = SavedGraph), (status = 404)))]
pub async fn get_saved_graph(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<SavedGraph>> {
    Ok(Json(service::get_detail(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/saved-graphs/{id}", request_body = UpdateSavedGraphBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = SavedGraph), (status = 400), (status = 404)))]
pub async fn update_saved_graph(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateSavedGraphBody>,
) -> ApiResult<Json<SavedGraph>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/saved-graphs/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_saved_graph(
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
            "/api/saved-graphs",
            get(list_saved_graphs).post(create_saved_graph),
        )
        .route(
            "/api/saved-graphs/{id}",
            get(get_saved_graph)
                .patch(update_saved_graph)
                .delete(delete_saved_graph),
        )
}
