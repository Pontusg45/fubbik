use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use fubbik_db::repo::saved_query::SavedQuery;

use super::dto::{
    AutocompleteQuery, CreateSavedQueryBody, ListSavedQuery, MessageResponse, ParseQuery,
    ParseResponse, SearchQueryBody, SearchResult,
};
use super::parser::parse_query_string;
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::{Json as ReqJson, Query};

/// `GET /api/search/parse` — `{clauses: parse_query_string(q)}`. No error
/// path beyond the session guard: `parse_query_string` never fails
/// (`search::parser`'s module doc), so the only non-200 this route can
/// produce is a 401.
#[utoipa::path(get, path = "/api/search/parse", params(ParseQuery),
    responses((status = 200, body = ParseResponse)))]
pub async fn parse(
    _user: CurrentUser,
    Query(query): Query<ParseQuery>,
) -> ApiResult<Json<ParseResponse>> {
    Ok(Json(ParseResponse {
        clauses: parse_query_string(&query.q),
    }))
}

/// `POST /api/search/query` — always 200, even on a database error; see
/// `search::service`'s module doc. `body.join` is accepted and ignored —
/// see `SearchQueryBody`'s doc comment.
#[utoipa::path(post, path = "/api/search/query", request_body = SearchQueryBody,
    responses((status = 200, body = SearchResult)))]
pub async fn query(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<SearchQueryBody>,
) -> ApiResult<Json<SearchResult>> {
    Ok(Json(
        service::execute_search(&state.pool, &user.id, &body).await,
    ))
}

/// `GET /api/search/autocomplete` — bare `string[]`. No error path beyond
/// the session guard; see `search::service::autocomplete`'s doc comment.
#[utoipa::path(get, path = "/api/search/autocomplete", params(AutocompleteQuery),
    responses((status = 200, body = Vec<String>)))]
pub async fn autocomplete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<AutocompleteQuery>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(
        service::autocomplete(&state.pool, &user.id, &query.field, &query.prefix).await,
    ))
}

/// `GET /api/search/saved` — bare array, user-scoped, optionally narrowed
/// to one space.
#[utoipa::path(get, path = "/api/search/saved", params(ListSavedQuery),
    responses((status = 200, body = Vec<SavedQuery>)))]
pub async fn list_saved(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListSavedQuery>,
) -> ApiResult<Json<Vec<SavedQuery>>> {
    Ok(Json(
        service::list_saved(&state.pool, &user.id, query.space_id.as_deref()).await?,
    ))
}

/// `POST /api/search/saved` — bare object, the created row.
#[utoipa::path(post, path = "/api/search/saved", request_body = CreateSavedQueryBody,
    responses((status = 200, body = SavedQuery)))]
pub async fn create_saved(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateSavedQueryBody>,
) -> ApiResult<Json<SavedQuery>> {
    Ok(Json(
        service::create_saved(&state.pool, &user.id, body).await?,
    ))
}

/// `DELETE /api/search/saved/{id}` — **never 404s**, even cross-user or for
/// a nonexistent id. The delete is still user-scoped in SQL
/// (`fubbik_db::repo::saved_query::delete`'s doc comment); the response
/// alone can never reveal that, only a surviving-row assertion can — see
/// `tests/search.rs`.
#[utoipa::path(delete, path = "/api/search/saved/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = MessageResponse)))]
pub async fn delete_saved(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MessageResponse>> {
    service::delete_saved(&state.pool, &user.id, &id).await?;
    Ok(Json(MessageResponse {
        message: "Deleted".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/search/parse", get(parse))
        .route("/api/search/query", post(query))
        .route("/api/search/autocomplete", get(autocomplete))
        .route("/api/search/saved", get(list_saved).post(create_saved))
        .route("/api/search/saved/{id}", delete(delete_saved))
}
