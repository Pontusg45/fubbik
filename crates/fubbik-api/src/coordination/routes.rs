use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::coordination::{AgentRun, CoordinationEntry};

use super::dto::{
    AckRunBody, BoardQuery, BoardSnapshot, ClaimBody, ClaimResponse, CreateEntryBody, JoinRunBody,
    TransitionTaskBody, TransitionTaskResponse,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::{Json as ReqJson, Query};

#[utoipa::path(post, path = "/api/plans/{planId}/board/runs", request_body = JoinRunBody,
    params(("planId" = String, Path,)), responses((status = 200, body = AgentRun), (status = 404), (status = 409)))]
pub async fn join_board(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(plan_id): Path<String>,
    ReqJson(body): ReqJson<JoinRunBody>,
) -> ApiResult<Json<AgentRun>> {
    Ok(Json(
        service::join(&state.pool, &user.id, &plan_id, body).await?,
    ))
}

#[utoipa::path(get, path = "/api/plans/{planId}/board", params(("planId" = String, Path,), BoardQuery),
    responses((status = 200, body = BoardSnapshot), (status = 400), (status = 404)))]
pub async fn read_board(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(plan_id): Path<String>,
    Query(query): Query<BoardQuery>,
) -> ApiResult<Json<BoardSnapshot>> {
    Ok(Json(
        service::board(&state.pool, &user.id, &plan_id, query).await?,
    ))
}

#[utoipa::path(post, path = "/api/plans/{planId}/board/tasks/{taskId}/claim", request_body = ClaimBody,
    params(("planId" = String, Path,), ("taskId" = String, Path,)), responses((status = 200, body = ClaimResponse), (status = 400), (status = 404), (status = 409)))]
pub async fn claim_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((plan_id, task_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<ClaimBody>,
) -> ApiResult<Json<ClaimResponse>> {
    Ok(Json(
        service::mutate_claim(&state.pool, &user.id, &plan_id, &task_id, body).await?,
    ))
}

#[utoipa::path(post, path = "/api/plans/{planId}/board/tasks/{taskId}/transition", request_body = TransitionTaskBody,
    params(("planId" = String, Path,), ("taskId" = String, Path,)), responses((status = 200, body = TransitionTaskResponse), (status = 400), (status = 404), (status = 409)))]
pub async fn transition_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((plan_id, task_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<TransitionTaskBody>,
) -> ApiResult<Json<TransitionTaskResponse>> {
    Ok(Json(
        service::transition(&state.pool, &user.id, &plan_id, &task_id, body).await?,
    ))
}

#[utoipa::path(post, path = "/api/plans/{planId}/board/entries", request_body = CreateEntryBody,
    params(("planId" = String, Path,)), responses((status = 200, body = CoordinationEntry), (status = 400), (status = 404), (status = 409)))]
pub async fn write_entry(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(plan_id): Path<String>,
    ReqJson(body): ReqJson<CreateEntryBody>,
) -> ApiResult<Json<CoordinationEntry>> {
    Ok(Json(
        service::write_entry(&state.pool, &user.id, &plan_id, body).await?,
    ))
}

#[utoipa::path(post, path = "/api/plans/{planId}/board/runs/{runId}/ack", request_body = AckRunBody,
    params(("planId" = String, Path,), ("runId" = String, Path,)), responses((status = 200, body = AgentRun), (status = 400), (status = 404)))]
pub async fn ack_board(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((plan_id, run_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<AckRunBody>,
) -> ApiResult<Json<AgentRun>> {
    Ok(Json(
        service::ack(&state.pool, &user.id, &plan_id, &run_id, body).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/plans/{planId}/board", get(read_board))
        .route("/api/plans/{planId}/board/runs", post(join_board))
        .route(
            "/api/plans/{planId}/board/tasks/{taskId}/claim",
            post(claim_task),
        )
        .route(
            "/api/plans/{planId}/board/tasks/{taskId}/transition",
            post(transition_task),
        )
        .route("/api/plans/{planId}/board/entries", post(write_entry))
        .route(
            "/api/plans/{planId}/board/runs/{runId}/ack",
            post(ack_board),
        )
}
