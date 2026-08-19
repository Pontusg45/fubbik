use axum::extract::{Path, State};
use axum::{Json, Router};
use fubbik_core::error::AppError;
use fubbik_db::repo::document::{Document, DocumentSearchResult, DocumentWithTagsItem};

use super::dto::{
    DocumentDetail, ImportDirBody, ImportDocumentBody, ImportResult, ListDocumentsQuery,
    RenderResult, SearchDocumentsQuery, SyncDocumentBody, SyncResult,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::{Json as ReqJson, Query};

#[utoipa::path(get, path = "/api/documents", params(ListDocumentsQuery),
    responses((status = 200, body = Vec<DocumentWithTagsItem>)))]
pub async fn list_documents(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListDocumentsQuery>,
) -> ApiResult<Json<Vec<DocumentWithTagsItem>>> {
    Ok(Json(
        service::list_documents_with_tags(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

/// `q` must be at least 2 characters, matching Node's `t.String({ minLength:
/// 2 })` (`packages/api/src/documents/routes.ts:32`) — enforced here rather
/// than left unchecked, unlike this port's usual "Elysia-only length caps
/// go unenforced" precedent (see `ImportDocumentBody`'s doc comment):
/// `minLength` here shapes actual query behaviour (an unbounded `ILIKE
/// '%_%'` scan for a 0-1 char query), not just a request-size guard.
#[utoipa::path(get, path = "/api/documents/search", params(SearchDocumentsQuery),
    responses((status = 200, body = Vec<DocumentSearchResult>), (status = 400)))]
pub async fn search_documents(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<SearchDocumentsQuery>,
) -> ApiResult<Json<Vec<DocumentSearchResult>>> {
    if query.q.chars().count() < 2 {
        return Err(AppError::Validation("q must be at least 2 characters".into()).into());
    }
    Ok(Json(
        service::search_documents(&state.pool, &user.id, &query.q, query.space_id.as_deref())
            .await?,
    ))
}

#[utoipa::path(get, path = "/api/documents/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = DocumentDetail), (status = 404)))]
pub async fn get_document(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<DocumentDetail>> {
    Ok(Json(
        service::get_document(&state.pool, &user.id, &id).await?,
    ))
}

#[utoipa::path(post, path = "/api/documents/import", request_body = ImportDocumentBody,
    responses((status = 200, body = ImportResult)))]
pub async fn import_document(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ImportDocumentBody>,
) -> ApiResult<Json<ImportResult>> {
    Ok(Json(
        service::import_document(
            &state.pool,
            &user.id,
            &body.source_path,
            &body.content,
            body.space_id.as_deref(),
        )
        .await?,
    ))
}

#[utoipa::path(post, path = "/api/documents/import-dir", request_body = ImportDirBody,
    responses((status = 200, body = Vec<ImportResult>)))]
pub async fn import_documents_dir(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ImportDirBody>,
) -> ApiResult<Json<Vec<ImportResult>>> {
    let mut results = Vec::with_capacity(body.files.len());
    for file in body.files {
        results.push(
            service::import_document(
                &state.pool,
                &user.id,
                &file.source_path,
                &file.content,
                body.space_id.as_deref(),
            )
            .await?,
        );
    }
    Ok(Json(results))
}

#[utoipa::path(post, path = "/api/documents/{id}/sync", params(("id" = String, Path,)),
    request_body = SyncDocumentBody,
    responses((status = 200, body = SyncResult), (status = 404)))]
pub async fn sync_document(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<SyncDocumentBody>,
) -> ApiResult<Json<SyncResult>> {
    Ok(Json(
        service::sync_document(
            &state.pool,
            &user.id,
            &id,
            &body.content,
            body.space_id.as_deref(),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/documents/{id}/render", params(("id" = String, Path,)),
    responses((status = 200, body = RenderResult), (status = 404)))]
pub async fn render_document(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<RenderResult>> {
    Ok(Json(
        service::render_document(&state.pool, &user.id, &id).await?,
    ))
}

/// Returns the deleted row directly (not a `{message: "Deleted"}` wrapper)
/// — matching Node's route, whose handler resolves to whatever
/// `removeDocument` returns (`deleteDocumentRepo`'s deleted row), unlike
/// most other domains' delete endpoints in this port.
#[utoipa::path(delete, path = "/api/documents/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = Document), (status = 404)))]
pub async fn delete_document(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Document>> {
    Ok(Json(
        service::remove_document(&state.pool, &user.id, &id).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/documents", axum::routing::get(list_documents))
        .route(
            "/api/documents/search",
            axum::routing::get(search_documents),
        )
        .route(
            "/api/documents/import",
            axum::routing::post(import_document),
        )
        .route(
            "/api/documents/import-dir",
            axum::routing::post(import_documents_dir),
        )
        .route(
            "/api/documents/{id}",
            axum::routing::get(get_document).delete(delete_document),
        )
        .route(
            "/api/documents/{id}/sync",
            axum::routing::post(sync_document),
        )
        .route(
            "/api/documents/{id}/render",
            axum::routing::get(render_document),
        )
}
