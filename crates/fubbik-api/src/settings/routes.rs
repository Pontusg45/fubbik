use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::dto::{
    CodebaseQuery, FeatureFlags, MessageResponse, SetCodebaseSettingBody, SetSettingBody,
    SettingsMap,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use crate::extract::Query;

/// Unauthenticated, matching Node exactly
/// (`packages/api/src/settings/routes.ts:8` — `GET /settings/features` has
/// no `requireSession`). See `get_instance_settings` for the full note on
/// why this crate has two deliberately open `GET`s in this domain.
#[utoipa::path(get, path = "/api/settings/features",
    responses((status = 200, body = FeatureFlags)))]
pub async fn get_feature_flags(State(state): State<AppState>) -> ApiResult<Json<FeatureFlags>> {
    Ok(Json(service::get_feature_flags(&state.pool).await?))
}

#[utoipa::path(get, path = "/api/settings/user",
    responses((status = 200, body = serde_json::Value)))]
pub async fn get_user_settings(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<SettingsMap>> {
    Ok(Json(
        service::get_all_user_settings(&state.pool, &user.id).await?,
    ))
}

#[utoipa::path(patch, path = "/api/settings/user", request_body = SetSettingBody,
    responses((status = 200, body = MessageResponse)))]
pub async fn set_user_setting(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<SetSettingBody>,
) -> ApiResult<Json<MessageResponse>> {
    service::set_user_setting(&state.pool, &user.id, &body.key, body.value).await?;
    Ok(Json(MessageResponse {
        message: "Updated".to_string(),
    }))
}

#[utoipa::path(get, path = "/api/settings/codebase", params(CodebaseQuery),
    responses((status = 200, body = serde_json::Value), (status = 404)))]
pub async fn get_codebase_settings(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<CodebaseQuery>,
) -> ApiResult<Json<SettingsMap>> {
    Ok(Json(
        service::get_all_codebase_settings(&state.pool, &user.id, &query.codebase_id).await?,
    ))
}

#[utoipa::path(patch, path = "/api/settings/codebase", request_body = SetCodebaseSettingBody,
    responses((status = 200, body = MessageResponse), (status = 404)))]
pub async fn set_codebase_setting(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<SetCodebaseSettingBody>,
) -> ApiResult<Json<MessageResponse>> {
    service::set_codebase_setting(
        &state.pool,
        &user.id,
        &body.codebase_id,
        &body.key,
        body.value,
    )
    .await?;
    Ok(Json(MessageResponse {
        message: "Updated".to_string(),
    }))
}

/// **Deliberately unauthenticated** — matches Node exactly
/// (`packages/api/src/settings/routes.ts:55`: `GET /settings/instance` has
/// no `requireSession`, and there is no global auth middleware in
/// `packages/api/src/index.ts` to backstop it). This was escalated to and
/// decided by the human partner explicitly (captured, then corrected on
/// review, in `tests/fixtures/node-contract-2b/_questions.md` Q2): faithful
/// port, `GET` open, `PATCH` session-gated (see `set_instance_setting`
/// below), exactly as Node. `instance_settings` holds zero rows today, the
/// server binds `127.0.0.1` by default, and the web app may legitimately
/// need to read feature flags before login — do NOT add a session guard
/// here believing it an oversight; that would silently diverge from an
/// already-made decision and could break pre-login flag reads.
///
/// The real property this leaves: any caller reachable on localhost can
/// read the raw instance-settings map with no session at all.
#[utoipa::path(get, path = "/api/settings/instance",
    responses((status = 200, body = serde_json::Value)))]
pub async fn get_instance_settings(State(state): State<AppState>) -> ApiResult<Json<SettingsMap>> {
    Ok(Json(service::get_all_instance_settings(&state.pool).await?))
}

/// Session-gated (unlike its `GET` sibling above), but with **no
/// role/admin check** — any authenticated user can flip instance-wide
/// flags like `aiEnabled`, matching Node's `requireSession`-only guard
/// (`packages/api/src/settings/routes.ts:56-60`). This is also a faithful
/// port, not an oversight.
#[utoipa::path(patch, path = "/api/settings/instance", request_body = SetSettingBody,
    responses((status = 200, body = MessageResponse)))]
pub async fn set_instance_setting(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    ReqJson(body): ReqJson<SetSettingBody>,
) -> ApiResult<Json<MessageResponse>> {
    service::set_instance_setting(&state.pool, &body.key, body.value).await?;
    Ok(Json(MessageResponse {
        message: "Updated".to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/settings/features", get(get_feature_flags))
        .route(
            "/api/settings/user",
            get(get_user_settings).patch(set_user_setting),
        )
        .route(
            "/api/settings/codebase",
            get(get_codebase_settings).patch(set_codebase_setting),
        )
        .route(
            "/api/settings/instance",
            get(get_instance_settings).patch(set_instance_setting),
        )
}
