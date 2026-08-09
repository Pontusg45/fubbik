use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::activity::Activity;
use fubbik_db::repo::plan::Plan;

use super::db::{PlanExternalLink, PlanListRow};
use super::dto::{
    CreateLinkBody, CreatePlanBody, ListPlansQuery, OkResponse, PlanDetail, UpdatePlanBody,
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

/// `PlanListRow`, not the bare `Plan` — see that struct's doc comment.
/// Confirmed against `tests/fixtures/node-contract-2c/plans-list.json`,
/// which carries the rollup fields even though the task brief's own
/// endpoint table just says "bare array".
#[utoipa::path(get, path = "/api/plans", params(ListPlansQuery),
    responses((status = 200, body = Vec<PlanListRow>), (status = 400)))]
pub async fn list_plans(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListPlansQuery>,
) -> ApiResult<Json<Vec<PlanListRow>>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.into_filter()).await?,
    ))
}

/// Every plans POST/PATCH/DELETE returns 200, never 201 — confirmed by grep
/// against Node's `plans/routes.ts` (`_mutating.md`): zero `set.status =
/// 201` calls anywhere in this slice. `Json<T>`'s default status is already
/// 200, so none of these handlers wrap their response in an explicit
/// `StatusCode`.
#[utoipa::path(post, path = "/api/plans", request_body = CreatePlanBody,
    responses((status = 200, body = Plan), (status = 400)))]
pub async fn create_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreatePlanBody>,
) -> ApiResult<Json<Plan>> {
    Ok(Json(service::create(&state.pool, &user.id, body).await?))
}

/// Enveloped `{plan, requirements, analyze, tasks, dependencies}` — the one
/// plan GET that isn't a bare array/object, confirmed against
/// `tests/fixtures/node-contract-2c/plans-detail.json`.
#[utoipa::path(get, path = "/api/plans/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = PlanDetail), (status = 404)))]
pub async fn get_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PlanDetail>> {
    Ok(Json(service::get_detail(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(patch, path = "/api/plans/{id}", request_body = UpdatePlanBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = Plan), (status = 400), (status = 404)))]
pub async fn update_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdatePlanBody>,
) -> ApiResult<Json<Plan>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

/// `{ ok: true }`, not `{ message: "Deleted" }` — Node's plans routes
/// discard the delete Effect's own result (`_mutating.md`).
#[utoipa::path(delete, path = "/api/plans/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = OkResponse), (status = 404)))]
pub async fn delete_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<OkResponse>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(OkResponse::default()))
}

/// Response is the new `Plan` row only — not its copied children, matching
/// the plain create/update endpoints' shape (`_mutating.md`).
#[utoipa::path(post, path = "/api/plans/{id}/duplicate", params(("id" = String, Path,)),
    responses((status = 200, body = Plan), (status = 404)))]
pub async fn duplicate_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Plan>> {
    Ok(Json(service::duplicate(&state.pool, &user.id, &id).await?))
}

/// Bare array, merged plan+task events sorted `createdAt` desc and sliced
/// to 100 — see `service::get_activity`'s doc comment.
#[utoipa::path(get, path = "/api/plans/{id}/activity", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<Activity>), (status = 404)))]
pub async fn plan_activity(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Activity>>> {
    Ok(Json(
        service::get_activity(&state.pool, &user.id, &id).await?,
    ))
}

#[utoipa::path(get, path = "/api/plans/{id}/links", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<PlanExternalLink>), (status = 404)))]
pub async fn list_plan_links(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<PlanExternalLink>>> {
    Ok(Json(service::list_links(&state.pool, &user.id, &id).await?))
}

/// `system` defaults to `"url"`, `label` to `null` when omitted — applied
/// in `service::add_link`.
#[utoipa::path(post, path = "/api/plans/{id}/links", request_body = CreateLinkBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = PlanExternalLink), (status = 404)))]
pub async fn add_plan_link(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<CreateLinkBody>,
) -> ApiResult<Json<PlanExternalLink>> {
    Ok(Json(
        service::add_link(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/plans/{id}/links/{linkId}",
    params(("id" = String, Path,), ("linkId" = String, Path,)),
    responses((status = 200, body = OkResponse), (status = 404)))]
pub async fn remove_plan_link(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, link_id)): Path<(String, String)>,
) -> ApiResult<Json<OkResponse>> {
    service::remove_link(&state.pool, &user.id, &id, &link_id).await?;
    Ok(Json(OkResponse::default()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/plans", get(list_plans).post(create_plan))
        .route(
            "/api/plans/{id}",
            get(get_plan).patch(update_plan).delete(delete_plan),
        )
        .route("/api/plans/{id}/duplicate", post(duplicate_plan))
        .route("/api/plans/{id}/activity", get(plan_activity))
        .route(
            "/api/plans/{id}/links",
            get(list_plan_links).post(add_plan_link),
        )
        .route(
            "/api/plans/{id}/links/{linkId}",
            axum::routing::delete(remove_plan_link),
        )
}
