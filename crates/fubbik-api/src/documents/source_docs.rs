use axum::{Json, Router, extract::State, routing::post};
use fubbik_core::source_docs::SourceManifest;
use fubbik_db::repo::source_docs::ImportReport;

use crate::{AppState, auth::CurrentUser, error::ApiResult, extract::Json as ReqJson};

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportSourceDocs {
    pub space_id: String,
    pub manifest: SourceManifest,
}

#[utoipa::path(post, path = "/api/documents/import-source", request_body = ImportSourceDocs,
    responses((status = 200, body = ImportReport), (status = 400), (status = 404)))]
pub async fn import_source(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ImportSourceDocs>,
) -> ApiResult<Json<ImportReport>> {
    Ok(Json(
        fubbik_db::repo::source_docs::import(&state.pool, &user.id, &body.space_id, &body.manifest)
            .await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/documents/import-source", post(import_source))
}
