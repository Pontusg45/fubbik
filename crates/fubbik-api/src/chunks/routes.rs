use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::chunk::Chunk;
use fubbik_db::repo::chunk_meta::{self, AppliesTo, FileRef};
use fubbik_db::repo::semantic::SemanticHit;

use super::ai;
use super::dto::{
    ChunkDetail, ChunkListResponse, CreateChunkBody, ListChunksQuery, SemanticSearchQuery,
    UpdateChunkBody,
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

/// **201**, not 200 — Node sets `ctx.set.status = 201` in an `Effect.tap`
/// after `createChunk` (`packages/api/src/chunks/routes.ts`), and this port
/// had been answering 200. Aligned here rather than left as a quiet
/// divergence; `POST /api/features` and `POST /api/requirements` are
/// already 201 in this crate, so 200 was also inconsistent internally.
#[utoipa::path(post, path = "/api/chunks", request_body = CreateChunkBody,
    responses((status = 201, body = Chunk)))]
pub async fn create_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateChunkBody>,
) -> ApiResult<(axum::http::StatusCode, Json<Chunk>)> {
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service::create(&state.pool, &user.id, body).await?),
    ))
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

/// Node's limit for this path (`chunks/routes.ts:197`) — 30 per 60s, keyed
/// per user, same shape as `enrich`'s but a distinct key prefix so the two
/// endpoints' budgets don't share a bucket.
const SEMANTIC_SEARCH_MAX: u32 = 30;
const SEMANTIC_SEARCH_WINDOW: Duration = Duration::from_secs(60);

/// No availability probe on this path — see `chunks::ai::semantic_search`'s
/// doc comment. An unreachable Ollama surfaces as a 502, not an empty 200.
#[utoipa::path(get, path = "/api/chunks/search/semantic", params(SemanticSearchQuery),
    responses((status = 200, body = Vec<SemanticHit>), (status = 429), (status = 502)))]
pub async fn search_semantic(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<SemanticSearchQuery>,
) -> ApiResult<axum::response::Response> {
    let decision = state.rate_limiter.check(
        &format!("semantic-search:{}", user.id),
        SEMANTIC_SEARCH_MAX,
        SEMANTIC_SEARCH_WINDOW,
    );
    if !decision.allowed {
        return Ok((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "retryAfter": decision.retry_after_secs,
            })),
        )
            .into_response());
    }

    let limit = query.limit.as_deref().and_then(|s| s.parse::<i64>().ok());
    let hits = ai::semantic_search(
        &state.pool,
        &state.ai,
        &user.id,
        &query.q,
        limit,
        query.exclude.as_deref(),
        query.scope.as_deref(),
    )
    .await?;
    Ok(Json(hits).into_response())
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

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ChunkMessage {
    pub message: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BulkUpdated {
    pub updated: u64,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BulkDeleted {
    pub deleted: u64,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct BulkUpdateBody {
    pub ids: Vec<String>,
    /// One of `add_tags | remove_tags | set_type | set_codebase |
    /// set_review_status | archive | delete`. Validated in the service.
    pub action: String,
    /// Meaning depends on `action`: a comma-separated tag list, a type, a
    /// space id, or a review status. Explicitly nullable — a null `value`
    /// with `set_codebase` clears the chunk's spaces.
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct BulkIdsBody {
    pub ids: Vec<String>,
}

/// `#[schema(as = ChunkMergeBody)]` because `tags::dto::MergeBody` already
/// claims the bare name. utoipa registers schemas in one flat namespace, so
/// without a rename one silently overwrites the other — `tests/schema_names.rs`
/// caught this.
///
/// The two are in fact **structurally identical** (`sourceId`/`targetId` in
/// both), so allowlisting the duplicate would also have been safe. Renamed
/// rather than allowlisted because the ids mean different things — these are
/// chunk ids, those are tag ids — and a generated client that shows one
/// `MergeBody` for two unrelated endpoints invites passing the wrong pair.
/// The allowlist is for names whose collision is *meaningless*, not merely
/// currently harmless.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ChunkMergeBody)]
pub struct MergeBody {
    pub source_id: String,
    pub target_id: String,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedQuery {
    pub space_id: Option<String>,
}

#[utoipa::path(post, path = "/api/chunks/{id}/archive", params(("id" = String, Path,)),
    responses((status = 200, body = ChunkMessage), (status = 404)))]
pub async fn archive_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ChunkMessage>> {
    service::archive(&state.pool, &user.id, &id).await?;
    Ok(Json(ChunkMessage {
        message: "Archived".into(),
    }))
}

#[utoipa::path(post, path = "/api/chunks/{id}/restore", params(("id" = String, Path,)),
    responses((status = 200, body = ChunkMessage), (status = 404)))]
pub async fn restore_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ChunkMessage>> {
    service::restore(&state.pool, &user.id, &id).await?;
    Ok(Json(ChunkMessage {
        message: "Restored".into(),
    }))
}

#[utoipa::path(get, path = "/api/chunks/archived", params(ArchivedQuery),
    responses((status = 200, body = Vec<Chunk>)))]
pub async fn list_archived(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ArchivedQuery>,
) -> ApiResult<Json<Vec<Chunk>>> {
    Ok(Json(
        service::list_archived(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

#[utoipa::path(post, path = "/api/chunks/bulk-update", request_body = BulkUpdateBody,
    responses((status = 200, body = BulkUpdated), (status = 400), (status = 404)))]
pub async fn bulk_update(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<BulkUpdateBody>,
) -> ApiResult<Json<BulkUpdated>> {
    let updated = service::bulk_update(
        &state.pool,
        &user.id,
        body.ids,
        &body.action,
        body.value.as_deref(),
    )
    .await?;
    Ok(Json(BulkUpdated { updated }))
}

#[utoipa::path(delete, path = "/api/chunks/bulk", request_body = BulkIdsBody,
    responses((status = 200, body = BulkDeleted), (status = 400)))]
pub async fn bulk_delete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<BulkIdsBody>,
) -> ApiResult<Json<BulkDeleted>> {
    let deleted = service::bulk_delete(&state.pool, &user.id, body.ids).await?;
    Ok(Json(BulkDeleted { deleted }))
}

#[utoipa::path(post, path = "/api/chunks/merge", request_body = MergeBody,
    responses((status = 200, body = Chunk), (status = 400), (status = 404)))]
pub async fn merge_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<MergeBody>,
) -> ApiResult<Json<Chunk>> {
    Ok(Json(
        service::merge(&state.pool, &user.id, &body.source_id, &body.target_id).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks", get(list_chunks).post(create_chunk))
        // Static segments first: axum resolves these ahead of `/{id}`
        // regardless, but `archived`, `bulk` and `merge` are one segment
        // deep and the ordering is kept explicit.
        .route("/api/chunks/search/semantic", get(search_semantic))
        .route("/api/chunks/archived", get(list_archived))
        .route("/api/chunks/bulk-update", post(bulk_update))
        .route("/api/chunks/bulk", axum::routing::delete(bulk_delete))
        .route("/api/chunks/merge", post(merge_chunks))
        .route("/api/chunks/{id}/archive", post(archive_chunk))
        .route("/api/chunks/{id}/restore", post(restore_chunk))
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
