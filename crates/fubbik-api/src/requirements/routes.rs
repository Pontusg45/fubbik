use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use fubbik_db::repo::requirement::{Requirement, RequirementChunkLink, RequirementStats};

use super::dto::{
    BatchCreateBody, BatchCreateResponse, BulkActionBody, CreateRequirementBody, ExportAllQuery,
    ExportOneQuery, ListRequirementsQuery, ListRequirementsResponse, MessageResponse, ReorderBody,
    ReorderResponse, RequirementDetail, RequirementWithWarnings, SetChunksBody, StatsQuery,
    UpdateRequirementBody, UpdateStatusBody,
};
use super::error::RequirementResult;
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::extract::Query;

#[utoipa::path(get, path = "/api/requirements/stats", params(StatsQuery),
    responses((status = 200, body = RequirementStats)))]
pub async fn stats(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<StatsQuery>,
) -> ApiResult<Json<RequirementStats>> {
    Ok(Json(
        service::get_stats(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

#[utoipa::path(patch, path = "/api/requirements/bulk", request_body = BulkActionBody,
    responses((status = 200, body = i64)))]
pub async fn bulk_action(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<BulkActionBody>,
) -> ApiResult<Json<i64>> {
    Ok(Json(
        service::bulk_action(&state.pool, &user.id, body).await?,
    ))
}

#[utoipa::path(patch, path = "/api/requirements/reorder", request_body = ReorderBody,
    responses((status = 200, body = ReorderResponse), (status = 400)))]
pub async fn reorder(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ReorderBody>,
) -> ApiResult<Json<ReorderResponse>> {
    let updated =
        service::reorder_requirements(&state.pool, &user.id, body.requirement_ids).await?;
    Ok(Json(ReorderResponse { updated }))
}

/// Bare text, not JSON — Node's `exportAll`/`exportRequirement` resolve to
/// a plain string and Elysia serialises a primitive response as
/// `text/plain`, same shape convention as `staleness::routes::stale_count`.
#[utoipa::path(get, path = "/api/requirements/export", params(ExportAllQuery),
    responses((status = 200, body = String)))]
pub async fn export_all(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ExportAllQuery>,
) -> ApiResult<String> {
    Ok(service::export_all(&state.pool, &user.id, query).await?)
}

#[utoipa::path(get, path = "/api/requirements", params(ListRequirementsQuery),
    responses((status = 200, body = ListRequirementsResponse)))]
pub async fn list_requirements(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListRequirementsQuery>,
) -> ApiResult<Json<ListRequirementsResponse>> {
    Ok(Json(
        service::list_requirements(&state.pool, &user.id, query).await?,
    ))
}

#[utoipa::path(post, path = "/api/requirements", request_body = CreateRequirementBody,
    responses((status = 201, body = RequirementWithWarnings), (status = 400), (status = 404)))]
pub async fn create_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateRequirementBody>,
) -> RequirementResult<(StatusCode, Json<RequirementWithWarnings>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create_requirement(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(post, path = "/api/requirements/batch", request_body = BatchCreateBody,
    responses((status = 201, body = BatchCreateResponse), (status = 400), (status = 404)))]
pub async fn batch_create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<BatchCreateBody>,
) -> RequirementResult<(StatusCode, Json<BatchCreateResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::batch_create_requirements(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(get, path = "/api/requirements/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = RequirementDetail), (status = 404)))]
pub async fn get_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<RequirementDetail>> {
    Ok(Json(
        service::get_requirement(&state.pool, &user.id, &id).await?,
    ))
}

#[utoipa::path(patch, path = "/api/requirements/{id}", request_body = UpdateRequirementBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = RequirementWithWarnings), (status = 400), (status = 404)))]
pub async fn update_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateRequirementBody>,
) -> RequirementResult<Json<RequirementWithWarnings>> {
    Ok(Json(
        service::update_requirement(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/requirements/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_requirement(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

#[utoipa::path(patch, path = "/api/requirements/{id}/status", request_body = UpdateStatusBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = Requirement), (status = 404)))]
pub async fn update_status(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateStatusBody>,
) -> ApiResult<Json<Requirement>> {
    Ok(Json(
        service::update_status(&state.pool, &user.id, &id, body.status).await?,
    ))
}

#[utoipa::path(put, path = "/api/requirements/{id}/chunks", request_body = SetChunksBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = Vec<RequirementChunkLink>), (status = 404)))]
pub async fn set_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<SetChunksBody>,
) -> ApiResult<Json<Vec<RequirementChunkLink>>> {
    Ok(Json(
        service::set_chunks(&state.pool, &user.id, &id, body.chunk_ids).await?,
    ))
}

/// Bare text, not JSON — see [`export_all`]'s doc comment.
#[utoipa::path(get, path = "/api/requirements/{id}/export", params(("id" = String, Path,), ExportOneQuery),
    responses((status = 200, body = String), (status = 404)))]
pub async fn export_one(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Query(query): Query<ExportOneQuery>,
) -> ApiResult<String> {
    Ok(service::export_requirement(&state.pool, &user.id, &id, query.format).await?)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/requirements/stats", get(stats))
        .route("/api/requirements/bulk", patch(bulk_action))
        .route("/api/requirements/reorder", patch(reorder))
        .route("/api/requirements/export", get(export_all))
        .route("/api/requirements/batch", post(batch_create))
        .route(
            "/api/requirements",
            get(list_requirements).post(create_requirement),
        )
        .route(
            "/api/requirements/{id}",
            get(get_requirement)
                .patch(update_requirement)
                .delete(delete_requirement),
        )
        .route("/api/requirements/{id}/status", patch(update_status))
        .route("/api/requirements/{id}/chunks", put(set_chunks))
        .route("/api/requirements/{id}/export", get(export_one))
}
