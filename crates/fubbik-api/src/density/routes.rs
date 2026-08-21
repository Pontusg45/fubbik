//! `GET /api/density` — chunk coverage folded into a directory tree.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::insights;

use super::service::{self, DensityResponse};
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DensityQuery {
    /// Named `codebaseId` on the wire — the `codebase → space` rename never
    /// reached this query param, and the web still sends the old name.
    pub codebase_id: Option<String>,
}

#[utoipa::path(get, path = "/api/density", params(DensityQuery),
    responses((status = 200, body = DensityResponse)))]
pub async fn get_density(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<DensityQuery>,
) -> ApiResult<Json<DensityResponse>> {
    let paths =
        insights::density_paths(&state.pool, &user.id, query.codebase_id.as_deref()).await?;
    Ok(Json(service::build(&paths)))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/density", get(get_density))
}
