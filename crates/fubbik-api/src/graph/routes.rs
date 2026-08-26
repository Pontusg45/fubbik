//! `GET /api/graph` — everything the graph page renders, in one payload.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::dto::GraphResponse;
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GraphQuery {
    /// `spaceId`, NOT `codebaseId`. Node still declares the pre-rename name
    /// (`packages/api/src/graph/routes.ts:18`) while the web sends `spaceId`,
    /// so Elysia strips it and the filter silently does nothing. Task 8 fixes
    /// Node to match this.
    pub space_id: Option<String>,
    /// Takes precedence over `space_id` when both are present.
    pub workspace_id: Option<String>,
}

#[utoipa::path(get, path = "/api/graph", params(GraphQuery),
    responses((status = 200, body = GraphResponse)))]
pub async fn get_graph(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<GraphQuery>,
) -> ApiResult<Json<GraphResponse>> {
    let response = service::build(
        &state.pool,
        &user.id,
        query.space_id.as_deref(),
        query.workspace_id.as_deref(),
    )
    .await?;
    Ok(Json(response))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/graph", get(get_graph))
}
