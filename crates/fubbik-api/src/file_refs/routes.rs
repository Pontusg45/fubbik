//! `GET /api/file-refs` and `GET /api/file-refs/lookup` — the top-level
//! reverse lookups. The `chunks/{id}/file-refs` sub-resource lives in the
//! chunks module.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::insights::{self, FileRefLookup};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub path: String,
    pub space_id: Option<String>,
}

#[utoipa::path(get, path = "/api/file-refs",
    responses((status = 200, body = Vec<FileRefLookup>)))]
pub async fn list_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<FileRefLookup>>> {
    Ok(Json(
        insights::list_all_file_refs(&state.pool, &user.id).await?,
    ))
}

#[utoipa::path(get, path = "/api/file-refs/lookup", params(LookupQuery),
    responses((status = 200, body = Vec<FileRefLookup>)))]
pub async fn lookup_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<LookupQuery>,
) -> ApiResult<Json<Vec<FileRefLookup>>> {
    Ok(Json(
        insights::lookup_by_path(
            &state.pool,
            &user.id,
            &query.path,
            query.space_id.as_deref(),
        )
        .await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        // Static `lookup` before the bare list is not strictly required —
        // they are different paths, not a static/dynamic pair — but keeps
        // the two visibly adjacent.
        .route("/api/file-refs/lookup", get(lookup_file_refs))
        .route("/api/file-refs", get(list_file_refs))
}
