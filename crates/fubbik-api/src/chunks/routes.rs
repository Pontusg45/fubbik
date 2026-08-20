use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::chunk::Chunk;
use fubbik_db::repo::chunk_meta::{self, AppliesTo, FileRef};

use super::dto::{
    ChunkDetail, ChunkListResponse, CreateChunkBody, ListChunksQuery, UpdateChunkBody,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
// Imported under its plain name (not e.g. `ReqQuery`) because utoipa's
// `axum_extras` feature infers a handler param's `parameter_in` (path vs
// query) by pattern-matching the literal `Query<T>` identifier used in the
// function signature below — see the comment on `ListChunksQuery`.
use crate::extract::Json as ReqJson;
use crate::extract::Query;

#[utoipa::path(
    get, path = "/api/chunks", params(ListChunksQuery),
    responses((status = 200, body = ChunkListResponse))
)]
pub async fn list_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListChunksQuery>,
) -> ApiResult<Json<ChunkListResponse>> {
    Ok(Json(
        service::list(&state.pool, &user.id, query.into_params()).await?,
    ))
}

#[utoipa::path(post, path = "/api/chunks", request_body = CreateChunkBody,
    responses((status = 200, body = Chunk)))]
pub async fn create_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateChunkBody>,
) -> ApiResult<Json<Chunk>> {
    Ok(Json(service::create(&state.pool, &user.id, body).await?))
}

/// Returns the **enriched** detail shape ([`ChunkDetail`]), not the bare
/// `chunk` row — matching Node. The active feature ids are loaded here
/// rather than in a global middleware: Node resolves them for every request
/// (`packages/api/src/index.ts:207-218`), but this is the only Rust route
/// that reads them implicitly, so a per-route load avoids paying for a
/// query on all ~200 other endpoints.
#[utoipa::path(get, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = ChunkDetail), (status = 404)))]
pub async fn get_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ChunkDetail>> {
    let active = fubbik_db::repo::feature::active_feature_ids(&state.pool, &user.id).await?;
    Ok(Json(
        service::get_detail(&state.pool, &user.id, &id, &active).await?,
    ))
}

#[utoipa::path(patch, path = "/api/chunks/{id}", request_body = UpdateChunkBody,
    params(("id" = String, Path,)), responses((status = 200, body = Chunk), (status = 404)))]
pub async fn update_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateChunkBody>,
) -> ApiResult<Json<Chunk>> {
    Ok(Json(
        service::update(&state.pool, &user.id, &id, body).await?,
    ))
}

#[utoipa::path(delete, path = "/api/chunks/{id}", params(("id" = String, Path,)),
    responses((status = 200), (status = 404)))]
pub async fn delete_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    service::delete(&state.pool, &user.id, &id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

#[utoipa::path(get, path = "/api/chunks/{id}/history", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<fubbik_db::repo::chunk_version::ChunkVersion>)))]
pub async fn chunk_history(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<fubbik_db::repo::chunk_version::ChunkVersion>>> {
    Ok(Json(service::history(&state.pool, &user.id, &id).await?))
}

/// One entry of `PUT /api/chunks/{id}/applies-to`'s body.
///
/// The body is a **bare array** of these, not `{patterns: [...]}` — that is
/// what Node's route schema declares
/// (`packages/api/src/applies-to/routes.ts:19-27`) and what the web edit
/// page has always sent. Rust previously published `{patterns: string[]}`,
/// so every real request 400'd; the edit page swallowed it in a
/// `catch { /* non-critical */ }` and silently never saved patterns at all.
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct AppliesToEntry {
    pub pattern: String,
    /// Optional and explicitly nullable, matching Node's
    /// `t.Optional(t.Union([t.String(), t.Null()]))`.
    #[serde(default)]
    pub note: Option<String>,
}

/// One entry of `PUT /api/chunks/{id}/file-refs`'s body — a bare array, for
/// the same reason as [`AppliesToEntry`]
/// (`packages/api/src/file-refs/routes.ts:17-26`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct FileRefEntry {
    pub path: String,
    #[serde(default)]
    pub anchor: Option<String>,
    /// Node constrains this to `documents | configures | tests |
    /// implements` on the route schema. Kept a `String` here and validated
    /// in the service layer rather than modelled as a serde enum: the
    /// column itself is free `text NOT NULL DEFAULT 'documents'` with no
    /// CHECK, and a DTO enum would reject with serde's parse error instead
    /// of a message naming the field.
    pub relation: String,
}

#[utoipa::path(get, path = "/api/chunks/{id}/applies-to", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<AppliesTo>)))]
pub async fn get_applies_to(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<AppliesTo>>> {
    service::get(&state.pool, &user.id, &id).await?;
    Ok(Json(
        chunk_meta::get_applies_to(&state.pool, &id, &user.id).await?,
    ))
}

#[utoipa::path(put, path = "/api/chunks/{id}/applies-to", request_body = Vec<AppliesToEntry>,
    params(("id" = String, Path,)), responses((status = 200, body = Vec<AppliesTo>)))]
pub async fn put_applies_to(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<Vec<AppliesToEntry>>,
) -> ApiResult<Json<Vec<AppliesTo>>> {
    service::get(&state.pool, &user.id, &id).await?;
    let entries = service::validate_applies_to(body)?;
    chunk_meta::replace_applies_to(&state.pool, &id, &user.id, &entries).await?;
    Ok(Json(
        chunk_meta::get_applies_to(&state.pool, &id, &user.id).await?,
    ))
}

#[utoipa::path(get, path = "/api/chunks/{id}/file-refs", params(("id" = String, Path,)),
    responses((status = 200, body = Vec<FileRef>)))]
pub async fn get_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<FileRef>>> {
    service::get(&state.pool, &user.id, &id).await?;
    Ok(Json(
        chunk_meta::get_file_refs(&state.pool, &id, &user.id).await?,
    ))
}

#[utoipa::path(put, path = "/api/chunks/{id}/file-refs", request_body = Vec<FileRefEntry>,
    params(("id" = String, Path,)), responses((status = 200, body = Vec<FileRef>)))]
pub async fn put_file_refs(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<Vec<FileRefEntry>>,
) -> ApiResult<Json<Vec<FileRef>>> {
    service::get(&state.pool, &user.id, &id).await?;
    let entries = service::validate_file_refs(body)?;
    chunk_meta::replace_file_refs(&state.pool, &id, &user.id, &entries).await?;
    Ok(Json(
        chunk_meta::get_file_refs(&state.pool, &id, &user.id).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks", get(list_chunks).post(create_chunk))
        .route(
            "/api/chunks/{id}",
            get(get_chunk).patch(update_chunk).delete(delete_chunk),
        )
        .route("/api/chunks/{id}/history", get(chunk_history))
        .route(
            "/api/chunks/{id}/applies-to",
            get(get_applies_to).put(put_applies_to),
        )
        .route(
            "/api/chunks/{id}/file-refs",
            get(get_file_refs).put(put_file_refs),
        )
}
