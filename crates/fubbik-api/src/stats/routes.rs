use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::stats::Stats;

use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;

/// `GET /api/stats` returns a **bare object** — not an array, and not the
/// `{chunks,total,limit,offset}` envelope `GET /api/chunks` uses. Matches
/// Node's `statsRoutes` (`packages/api/src/stats/routes.ts`), which returns
/// `statsService.getUserStats(session.user.id)` directly as the response
/// body.
#[utoipa::path(get, path = "/api/stats", responses((status = 200, body = Stats)))]
pub async fn get_stats(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Stats>> {
    Ok(Json(service::get_stats(&state.pool, &user.id).await?))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/stats", get(get_stats))
}
