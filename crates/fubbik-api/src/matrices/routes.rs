//! `/api/matrices` — 25 endpoints.
//!
//! Status codes follow Node: create-shaped POSTs answer **201**, everything
//! else 200. Deletes and unlinks answer `{ message }` rather than the removed
//! row.
//!
//! Every cell-surface handler passes both the matrix id from the path AND the
//! session, because in Node it passed neither — see the `fix(matrices)`
//! commit. The guard itself lives in SQL.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use fubbik_db::repo::behavior_matrix as repo;

use super::dto::*;
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::extract::Query;

fn ok() -> Json<MatrixMessage> {
    Json(MatrixMessage {
        message: "Deleted".into(),
    })
}

fn unlinked() -> Json<MatrixMessage> {
    Json(MatrixMessage {
        message: "Unlinked".into(),
    })
}

// --- Matrix ---

#[utoipa::path(get, path = "/api/matrices", params(ListMatricesQuery),
    responses((status = 200, body = Vec<repo::BehaviorMatrix>)))]
pub async fn list_matrices(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListMatricesQuery>,
) -> ApiResult<Json<Vec<repo::BehaviorMatrix>>> {
    Ok(Json(service::list(&state.pool, &user.id, query).await?))
}

#[utoipa::path(post, path = "/api/matrices", request_body = CreateMatrixBody,
    responses((status = 201, body = repo::BehaviorMatrix), (status = 400)))]
pub async fn create_matrix(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateMatrixBody>,
) -> ApiResult<(StatusCode, Json<repo::BehaviorMatrix>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
}

#[utoipa::path(get, path = "/api/matrices/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MatrixDetail), (status = 404)))]
pub async fn get_matrix(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MatrixDetail>> {
    Ok(Json(service::detail(&state.pool, &id, &user.id).await?))
}

#[utoipa::path(get, path = "/api/matrices/{id}/view", params(("id" = String, Path,)),
    responses((status = 200, body = MatrixView), (status = 404)))]
pub async fn get_matrix_view(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MatrixView>> {
    Ok(Json(service::view(&state.pool, &id, &user.id).await?))
}

#[utoipa::path(patch, path = "/api/matrices/{id}", request_body = UpdateMatrixBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = repo::BehaviorMatrix), (status = 404)))]
pub async fn update_matrix(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateMatrixBody>,
) -> ApiResult<Json<repo::BehaviorMatrix>> {
    Ok(Json(
        service::update(&state.pool, &id, &user.id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/matrices/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn delete_matrix(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MatrixMessage>> {
    service::delete(&state.pool, &id, &user.id).await?;
    Ok(ok())
}

// --- Dimensions ---

#[utoipa::path(post, path = "/api/matrices/{id}/dimensions", request_body = DimensionBody,
    params(("id" = String, Path,)),
    responses((status = 201, body = repo::BehaviorDimension), (status = 404)))]
pub async fn add_dimension(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<DimensionBody>,
) -> ApiResult<(StatusCode, Json<repo::BehaviorDimension>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::add_dimension(&state.pool, &id, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/matrices/{id}/dimensions/{dimId}", request_body = DimensionBody,
    params(("id" = String, Path,), ("dimId" = String, Path,)),
    responses((status = 200, body = repo::BehaviorDimension), (status = 404)))]
pub async fn rename_dimension(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, dim_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<DimensionBody>,
) -> ApiResult<Json<repo::BehaviorDimension>> {
    Ok(Json(
        service::rename_dimension(&state.pool, &id, &dim_id, &user.id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/matrices/{id}/dimensions/{dimId}",
    params(("id" = String, Path,), ("dimId" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn delete_dimension(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, dim_id)): Path<(String, String)>,
) -> ApiResult<Json<MatrixMessage>> {
    service::remove_dimension(&state.pool, &id, &dim_id, &user.id).await?;
    Ok(ok())
}

#[utoipa::path(post, path = "/api/matrices/{id}/dimensions/reorder",
    request_body = ReorderDimensionsBody, params(("id" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn reorder_dimensions(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<ReorderDimensionsBody>,
) -> ApiResult<Json<MatrixMessage>> {
    service::reorder_dimensions(&state.pool, &id, &user.id, body.dimension_ids).await?;
    Ok(Json(MatrixMessage {
        message: "Reordered".into(),
    }))
}

// --- Rules ---

#[utoipa::path(post, path = "/api/matrices/{id}/rules", request_body = CreateRuleBody,
    params(("id" = String, Path,)),
    responses((status = 201, body = repo::BehaviorRule), (status = 404)))]
pub async fn add_rule(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<CreateRuleBody>,
) -> ApiResult<(StatusCode, Json<repo::BehaviorRule>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::add_rule(&state.pool, &id, &user.id, body).await?),
    ))
}

#[utoipa::path(patch, path = "/api/matrices/{id}/rules/{ruleId}", request_body = UpdateRuleBody,
    params(("id" = String, Path,), ("ruleId" = String, Path,)),
    responses((status = 200, body = repo::BehaviorRule), (status = 404)))]
pub async fn update_rule(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, rule_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<UpdateRuleBody>,
) -> ApiResult<Json<repo::BehaviorRule>> {
    Ok(Json(
        service::update_rule(&state.pool, &id, &rule_id, &user.id, body).await?,
    ))
}

#[utoipa::path(get, path = "/api/matrices/{id}/rules/{ruleId}/history",
    params(("id" = String, Path,), ("ruleId" = String, Path,)),
    responses((status = 200, body = Vec<repo::BehaviorRuleVersion>), (status = 404)))]
pub async fn rule_history(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, rule_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<repo::BehaviorRuleVersion>>> {
    Ok(Json(
        service::rule_history(&state.pool, &id, &rule_id, &user.id).await?,
    ))
}

#[utoipa::path(delete, path = "/api/matrices/{id}/rules/{ruleId}",
    params(("id" = String, Path,), ("ruleId" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn delete_rule(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, rule_id)): Path<(String, String)>,
) -> ApiResult<Json<MatrixMessage>> {
    service::remove_rule(&state.pool, &id, &rule_id, &user.id).await?;
    Ok(ok())
}

#[utoipa::path(post, path = "/api/matrices/{id}/rules/reorder",
    request_body = ReorderRulesBody, params(("id" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn reorder_rules(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<ReorderRulesBody>,
) -> ApiResult<Json<MatrixMessage>> {
    service::reorder_rules(&state.pool, &id, &user.id, body.rule_ids).await?;
    Ok(Json(MatrixMessage {
        message: "Reordered".into(),
    }))
}

// --- Cells ---

#[utoipa::path(put, path = "/api/matrices/{id}/cells", request_body = ToggleCellBody,
    params(("id" = String, Path,)),
    responses((status = 200, body = ToggleCellResponse), (status = 400), (status = 404)))]
pub async fn toggle_cell(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<ToggleCellBody>,
) -> ApiResult<Json<ToggleCellResponse>> {
    Ok(Json(
        service::toggle_cell(&state.pool, &id, &user.id, body).await?,
    ))
}

#[utoipa::path(post, path = "/api/matrices/{id}/cells/{cellId}/requirements",
    request_body = LinkRequirementBody,
    params(("id" = String, Path,), ("cellId" = String, Path,)),
    responses((status = 201, body = repo::CellRequirementLink), (status = 404), (status = 409)))]
pub async fn link_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<LinkRequirementBody>,
) -> ApiResult<(StatusCode, Json<repo::CellRequirementLink>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::link_requirement(&state.pool, &id, &cell_id, &user.id, body).await?),
    ))
}

#[utoipa::path(delete, path = "/api/matrices/{id}/cells/{cellId}/requirements/{reqId}",
    params(("id" = String, Path,), ("cellId" = String, Path,), ("reqId" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn unlink_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id, req_id)): Path<(String, String, String)>,
) -> ApiResult<Json<MatrixMessage>> {
    service::unlink_requirement(&state.pool, &id, &cell_id, &user.id, &req_id).await?;
    Ok(unlinked())
}

#[utoipa::path(get, path = "/api/matrices/{id}/cells/{cellId}/requirements",
    params(("id" = String, Path,), ("cellId" = String, Path,)),
    responses((status = 200, body = Vec<repo::CellRequirement>), (status = 404)))]
pub async fn get_cell_requirements(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<repo::CellRequirement>>> {
    Ok(Json(
        service::requirements_for_cell(&state.pool, &id, &cell_id, &user.id).await?,
    ))
}

#[utoipa::path(post, path = "/api/matrices/{id}/cells/{cellId}/code",
    request_body = LinkCodeBody,
    params(("id" = String, Path,), ("cellId" = String, Path,)),
    responses((status = 201, body = repo::BehaviorCellCode), (status = 400), (status = 404), (status = 409)))]
pub async fn link_code(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<LinkCodeBody>,
) -> ApiResult<(StatusCode, Json<repo::BehaviorCellCode>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::link_code(&state.pool, &id, &cell_id, &user.id, body).await?),
    ))
}

#[utoipa::path(delete, path = "/api/matrices/{id}/cells/{cellId}/code/{codeId}",
    params(("id" = String, Path,), ("cellId" = String, Path,), ("codeId" = String, Path,)),
    responses((status = 200, body = MatrixMessage), (status = 404)))]
pub async fn unlink_code(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id, code_id)): Path<(String, String, String)>,
) -> ApiResult<Json<MatrixMessage>> {
    service::unlink_code(&state.pool, &id, &cell_id, &user.id, &code_id).await?;
    Ok(unlinked())
}

#[utoipa::path(get, path = "/api/matrices/{id}/cells/{cellId}/code",
    params(("id" = String, Path,), ("cellId" = String, Path,)),
    responses((status = 200, body = Vec<repo::BehaviorCellCode>), (status = 404)))]
pub async fn get_cell_code(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<repo::BehaviorCellCode>>> {
    Ok(Json(
        service::code_for_cell(&state.pool, &id, &cell_id, &user.id).await?,
    ))
}

#[utoipa::path(post, path = "/api/matrices/{id}/cells/{cellId}/test-results",
    request_body = TestResultBody,
    params(("id" = String, Path,), ("cellId" = String, Path,)),
    responses((status = 201, body = repo::BehaviorTestResult), (status = 400), (status = 404)))]
pub async fn record_test_result(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id)): Path<(String, String)>,
    ReqJson(body): ReqJson<TestResultBody>,
) -> ApiResult<(StatusCode, Json<repo::BehaviorTestResult>)> {
    Ok((
        StatusCode::CREATED,
        Json(service::record_test_result(&state.pool, &id, &cell_id, &user.id, body).await?),
    ))
}

#[utoipa::path(get, path = "/api/matrices/{id}/cells/{cellId}/test-results",
    params(("id" = String, Path,), ("cellId" = String, Path,)),
    responses((status = 200, body = Vec<repo::BehaviorTestResult>), (status = 404)))]
pub async fn get_cell_test_results(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path((id, cell_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<repo::BehaviorTestResult>>> {
    Ok(Json(
        service::test_results_for_cell(&state.pool, &id, &cell_id, &user.id).await?,
    ))
}

/// Registered BEFORE `/api/matrices/{id}` would otherwise capture it — axum
/// matches static segments ahead of dynamic ones, so `behaviors-for-file`
/// resolves correctly either way, but the ordering is kept explicit because
/// the two paths are one segment apart.
#[utoipa::path(get, path = "/api/matrices/behaviors-for-file", params(BehaviorsForFileQuery),
    responses((status = 200, body = Vec<repo::BehaviorForFile>)))]
pub async fn behaviors_for_file(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<BehaviorsForFileQuery>,
) -> ApiResult<Json<Vec<repo::BehaviorForFile>>> {
    Ok(Json(
        service::behaviors_for_path(&state.pool, &user.id, &query.path).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/matrices", get(list_matrices).post(create_matrix))
        .route("/api/matrices/behaviors-for-file", get(behaviors_for_file))
        .route(
            "/api/matrices/{id}",
            get(get_matrix).patch(update_matrix).delete(delete_matrix),
        )
        .route("/api/matrices/{id}/view", get(get_matrix_view))
        .route("/api/matrices/{id}/dimensions", post(add_dimension))
        .route(
            "/api/matrices/{id}/dimensions/reorder",
            post(reorder_dimensions),
        )
        .route(
            "/api/matrices/{id}/dimensions/{dimId}",
            axum::routing::patch(rename_dimension).delete(delete_dimension),
        )
        .route("/api/matrices/{id}/rules", post(add_rule))
        .route("/api/matrices/{id}/rules/reorder", post(reorder_rules))
        .route(
            "/api/matrices/{id}/rules/{ruleId}",
            axum::routing::patch(update_rule).delete(delete_rule),
        )
        .route(
            "/api/matrices/{id}/rules/{ruleId}/history",
            get(rule_history),
        )
        .route("/api/matrices/{id}/cells", put(toggle_cell))
        .route(
            "/api/matrices/{id}/cells/{cellId}/requirements",
            get(get_cell_requirements).post(link_requirement),
        )
        .route(
            "/api/matrices/{id}/cells/{cellId}/requirements/{reqId}",
            axum::routing::delete(unlink_requirement),
        )
        .route(
            "/api/matrices/{id}/cells/{cellId}/code",
            get(get_cell_code).post(link_code),
        )
        .route(
            "/api/matrices/{id}/cells/{cellId}/code/{codeId}",
            axum::routing::delete(unlink_code),
        )
        .route(
            "/api/matrices/{id}/cells/{cellId}/test-results",
            get(get_cell_test_results).post(record_test_result),
        )
}
