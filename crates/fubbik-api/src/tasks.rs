//! Compatibility task queue backed by single-task plans.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_core::error::AppError;
use fubbik_db::repo::plan::{ListFilter, Plan, PlanListRow};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::extract::Query;
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

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct ListQueueTasksQuery {
    /// `in_progress` (default) or `completed`.
    pub status: Option<String>,
}

fn priority(value: Option<&str>) -> ApiResult<&'static str> {
    match value.unwrap_or("normal") {
        "low" => Ok("low"),
        "normal" => Ok("normal"),
        "high" => Ok("high"),
        "urgent" => Ok("urgent"),
        _ => {
            Err(AppError::Validation("Priority must be low, normal, high, or urgent".into()).into())
        }
    }
}

fn priority_rank(row: &PlanListRow) -> u8 {
    match row.metadata.0["priority"].as_str() {
        Some("urgent") => 3,
        Some("high") => 2,
        Some("low") => 0,
        _ => 1,
    }
}

#[utoipa::path(post, path = "/api/tasks", request_body = CreateQueueTaskBody,
    responses((status = 201, body = Plan), (status = 400)))]
pub async fn create_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateQueueTaskBody>,
) -> ApiResult<(StatusCode, Json<Plan>)> {
    let priority = priority(body.priority.as_deref())?;
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
            metadata: Some(serde_json::json!({ "taskQueue": true, "priority": priority })),
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(set_plan_status(&state, &user.id, &plan.id, "in_progress").await?),
    ))
}

#[utoipa::path(get, path = "/api/tasks", params(ListQueueTasksQuery),
    responses((status = 200, body = Vec<PlanListRow>), (status = 400)))]
pub async fn list_tasks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListQueueTasksQuery>,
) -> ApiResult<Json<Vec<PlanListRow>>> {
    let status = query.status.as_deref().unwrap_or("in_progress");
    if !matches!(status, "in_progress" | "completed") {
        return Err(AppError::Validation("Status must be in_progress or completed".into()).into());
    }
    let mut rows = service::list(
        &state.pool,
        &user.id,
        ListFilter {
            status: Some(status.into()),
            task_queue_only: true,
            ..Default::default()
        },
    )
    .await?;
    if status == "in_progress" {
        rows.sort_by_key(|row| std::cmp::Reverse(priority_rank(row)));
    } else {
        rows.sort_by_key(|row| std::cmp::Reverse(row.completed_at));
    }
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/api/tasks/{id}/claim", operation_id = "claim_queue_task",
    params(("id" = String, Path,)),
    responses((status = 200, body = PlanDetail), (status = 404)))]
pub async fn claim_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PlanDetail>> {
    let detail = queue_detail(&state, &user.id, &id).await?;
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
    let detail = queue_detail(&state, &user.id, &id).await?;
    if let Some(task) = detail.tasks.first() {
        service::update_task(&state.pool, &user.id, &id, &task.id, task_status("done")).await?;
    }
    Ok(Json(
        set_plan_status(&state, &user.id, &id, "completed").await?,
    ))
}

async fn queue_detail(state: &AppState, user_id: &str, id: &str) -> ApiResult<PlanDetail> {
    let detail = service::get_detail(&state.pool, user_id, id).await?;
    if detail.plan.metadata.0["taskQueue"] != true {
        return Err(AppError::NotFound("Task".into()).into());
    }
    if detail.plan.status != "in_progress" {
        return Err(AppError::Conflict("Task is not active".into()).into());
    }
    Ok(detail)
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
