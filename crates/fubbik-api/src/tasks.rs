//! Compatibility task queue backed by single-task plans.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::plan::{ListFilter, Plan, PlanListRow};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::plans::dto::{
    CreatePlanBody, CreateTaskInput, PlanDetail, UpdatePlanBody, UpdateTaskBody,
};
use crate::plans::service;

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateQueueTaskBody {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub space_id: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CompleteQueueTaskBody {
    pub note: Option<String>,
}

#[utoipa::path(post, path = "/api/tasks", request_body = CreateQueueTaskBody,
    responses((status = 201, body = Plan), (status = 400)))]
pub async fn create_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateQueueTaskBody>,
) -> ApiResult<(StatusCode, Json<Plan>)> {
    let plan = service::create(
        &state.pool,
        &user.id,
        CreatePlanBody {
            title: body.title.clone(),
            description: body.description,
            space_id: body.space_id,
            requirement_ids: None,
            tasks: Some(vec![CreateTaskInput {
                title: body.title,
                description: None,
                acceptance_criteria: None,
            }]),
            metadata: None,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(set_plan_status(&state, &user.id, &plan.id, "in_progress").await?),
    ))
}

#[utoipa::path(get, path = "/api/tasks",
    responses((status = 200, body = Vec<PlanListRow>)))]
pub async fn list_tasks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<PlanListRow>>> {
    Ok(Json(
        service::list(
            &state.pool,
            &user.id,
            ListFilter {
                status: Some("in_progress".into()),
                ..Default::default()
            },
        )
        .await?,
    ))
}

#[utoipa::path(post, path = "/api/tasks/{id}/claim", params(("id" = String, Path,)),
    responses((status = 200, body = PlanDetail), (status = 404)))]
pub async fn claim_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PlanDetail>> {
    let detail = service::get_detail(&state.pool, &user.id, &id).await?;
    if let Some(task) = detail.tasks.first() {
        service::update_task(
            &state.pool,
            &user.id,
            &id,
            &task.id,
            task_status("in_progress"),
        )
        .await?;
    }
    set_plan_status(&state, &user.id, &id, "in_progress").await?;
    Ok(Json(service::get_detail(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(post, path = "/api/tasks/{id}/complete", request_body = CompleteQueueTaskBody,
    params(("id" = String, Path,)), responses((status = 200, body = Plan), (status = 404)))]
pub async fn complete_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<CompleteQueueTaskBody>,
) -> ApiResult<Json<Plan>> {
    let _note = body.note;
    let detail = service::get_detail(&state.pool, &user.id, &id).await?;
    if let Some(task) = detail.tasks.first() {
        service::update_task(&state.pool, &user.id, &id, &task.id, task_status("done")).await?;
    }
    Ok(Json(
        set_plan_status(&state, &user.id, &id, "completed").await?,
    ))
}

async fn set_plan_status(
    state: &AppState,
    user_id: &str,
    id: &str,
    status: &str,
) -> ApiResult<Plan> {
    Ok(service::update(
        &state.pool,
        user_id,
        id,
        UpdatePlanBody {
            title: None,
            description: None,
            status: Some(status.into()),
            space_id: None,
            metadata: None,
        },
    )
    .await?)
}

fn task_status(status: &str) -> UpdateTaskBody {
    UpdateTaskBody {
        title: None,
        description: None,
        acceptance_criteria: None,
        status: Some(status.into()),
        metadata: None,
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}/claim", post(claim_task))
        .route("/api/tasks/{id}/complete", post(complete_task))
}
