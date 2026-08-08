use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::activity::Activity;

use super::dto::ListActivityQuery;
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
// Imported under its plain name so utoipa's `axum_extras` feature can infer
// this parameter is a query param by pattern-matching the literal
// `Query<T>` identifier — see the comment on `chunks::dto::ListChunksQuery`.
use crate::extract::Query;

/// Bare array, no `total` field — Node's `listActivityRepo` applies
/// `LIMIT`/`OFFSET` in the query itself but never returns a count
/// (`_questions.md` Q3), unlike `chunks`' `{chunks, total, limit, offset}`
/// envelope. A foreign `spaceId` 404s — see `service::list`'s doc comment
/// for why that's a deliberate divergence from Node's 200-with-`[]`.
#[utoipa::path(get, path = "/api/activity", params(ListActivityQuery),
    responses((status = 200, body = Vec<Activity>), (status = 404)))]
pub async fn list_activity(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListActivityQuery>,
) -> ApiResult<Json<Vec<Activity>>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.into_params()).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/activity", get(list_activity))
}
