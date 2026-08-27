use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use fubbik_db::repo::vocabulary::VocabularyEntry;

use super::dto::{
    BulkCreateBody, CreateEntryBody, ListVocabularyQuery, MessageResponse, ParseBody, SuggestBody,
    UpdateEntryBody,
};
use super::parser::ParseResult;
use super::service;
use super::suggest::SuggestedEntry;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
// Imported under its plain name so utoipa's `axum_extras` feature can infer
// this parameter is a query param by pattern-matching the literal
// `Query<T>` identifier — see the comment on `chunks::dto::ListChunksQuery`.
use crate::extract::Query;

/// `spaceId` absent -> `[]` without ever reaching the service layer,
/// matching Node's `if (!ctx.query.spaceId) return [];`
/// (`packages/api/src/vocabulary/routes.ts:30`) exactly — note this is
/// *not* a 404 the way every other endpoint in this domain treats a
/// missing/foreign space.
#[utoipa::path(get, path = "/api/vocabulary", params(ListVocabularyQuery),
    responses((status = 200, body = Vec<VocabularyEntry>)))]
pub async fn list_vocabulary(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListVocabularyQuery>,
) -> ApiResult<Json<Vec<VocabularyEntry>>> {
    let Some(space_id) = query.space_id else {
        return Ok(Json(Vec::new()));
    };
    Ok(Json(
        service::list_vocabulary(&state.pool, &user.id, &space_id).await?,
    ))
}

#[utoipa::path(post, path = "/api/vocabulary/suggest", request_body = SuggestBody,
    responses((status = 200, body = Vec<SuggestedEntry>), (status = 404)))]
pub async fn suggest_vocabulary(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<SuggestBody>,
) -> ApiResult<Json<Vec<SuggestedEntry>>> {
    Ok(Json(
        service::suggest_from_chunks(&state.pool, &state.ai, &user.id, &body.space_id).await?,
    ))
}

#[utoipa::path(post, path = "/api/vocabulary/bulk", request_body = BulkCreateBody,
    responses((status = 201, body = Vec<VocabularyEntry>), (status = 404)))]
pub async fn bulk_create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<BulkCreateBody>,
) -> ApiResult<(StatusCode, Json<Vec<VocabularyEntry>>)> {
    let entries = body
        .entries
        .into_iter()
        .map(|e| (e.word, e.category, e.expects))
        .collect();
    let created = service::create_entries(&state.pool, &user.id, entries, &body.space_id).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(post, path = "/api/vocabulary/parse", request_body = ParseBody,
    responses((status = 200, body = ParseResult), (status = 404)))]
pub async fn parse_step(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ParseBody>,
) -> ApiResult<Json<ParseResult>> {
    Ok(Json(
        service::parse_step(&state.pool, &user.id, &body.text, &body.space_id).await?,
    ))
}

#[utoipa::path(post, path = "/api/vocabulary", request_body = CreateEntryBody,
    responses((status = 201, body = VocabularyEntry), (status = 404)))]
pub async fn create_entry(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateEntryBody>,
) -> ApiResult<(StatusCode, Json<VocabularyEntry>)> {
    let created = service::create_entry(
        &state.pool,
        &user.id,
        &body.word,
        body.category,
        body.expects,
        &body.space_id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(patch, path = "/api/vocabulary/{id}", params(("id" = String, Path,)),
    request_body = UpdateEntryBody,
    responses((status = 200, body = VocabularyEntry), (status = 404)))]
pub async fn update_entry(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<UpdateEntryBody>,
) -> ApiResult<Json<VocabularyEntry>> {
    Ok(Json(
        service::update_entry(
            &state.pool,
            &user.id,
            &id,
            body.word,
            body.category,
            body.expects,
        )
        .await?,
    ))
}

#[utoipa::path(delete, path = "/api/vocabulary/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn delete_entry(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_entry(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vocabulary", get(list_vocabulary).post(create_entry))
        .route("/api/vocabulary/suggest", post(suggest_vocabulary))
        .route("/api/vocabulary/bulk", post(bulk_create))
        .route("/api/vocabulary/parse", post(parse_step))
        .route(
            "/api/vocabulary/{id}",
            patch(update_entry).delete(delete_entry),
        )
}
