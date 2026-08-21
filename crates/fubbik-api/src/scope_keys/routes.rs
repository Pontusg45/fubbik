//! `/api/scope-keys` — the optional registry of expected chunk `scope` keys.
//!
//! Opt-in by design: `chunk.scope` stays free-form JSONB whether or not a key
//! is registered here. The registry drives autocomplete, not validation.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::insights::{self, NewScopeKey, ScopeKey};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

const VALUE_TYPES: [&str; 4] = ["string", "number", "boolean", "enum"];

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateScopeKeyBody {
    pub key: String,
    pub description: Option<String>,
    /// `string | number | boolean | enum`. Defaults to `string`, matching the
    /// column default.
    pub value_type: Option<String>,
    pub allowed_values: Option<Vec<String>>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ScopeKeyMessage {
    pub message: String,
}

#[utoipa::path(get, path = "/api/scope-keys",
    responses((status = 200, body = Vec<ScopeKey>)))]
pub async fn list_scope_keys(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<ScopeKey>>> {
    Ok(Json(
        insights::list_scope_keys(&state.pool, &user.id).await?,
    ))
}

fn validate(body: &CreateScopeKeyBody) -> AppResult<String> {
    let key = body.key.trim();
    if key.is_empty() {
        return Err(AppError::Validation("key is required".into()));
    }
    let value_type = body.value_type.as_deref().unwrap_or("string");
    if !VALUE_TYPES.contains(&value_type) {
        return Err(AppError::Validation(format!(
            "valueType must be one of {}",
            VALUE_TYPES.join(", ")
        )));
    }
    // An `enum` key with nothing to choose from would make the autocomplete
    // offer an empty list forever. Node does not check this; rejecting it is
    // a deliberate divergence, and cheap — the registry is opt-in, so the
    // only cost of being strict is a clearer error.
    if value_type == "enum" && body.allowed_values.as_ref().is_none_or(|v| v.is_empty()) {
        return Err(AppError::Validation(
            "allowedValues is required when valueType is enum".into(),
        ));
    }
    Ok(key.to_string())
}

#[utoipa::path(post, path = "/api/scope-keys", request_body = CreateScopeKeyBody,
    responses((status = 201, body = ScopeKey), (status = 400), (status = 409)))]
pub async fn create_scope_key(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateScopeKeyBody>,
) -> ApiResult<(StatusCode, Json<ScopeKey>)> {
    let key = validate(&body)?;
    let value_type = body.value_type.unwrap_or_else(|| "string".into());
    let created = insights::create_scope_key(
        &state.pool,
        &user.id,
        NewScopeKey {
            key,
            description: body.description,
            value_type,
            allowed_values: body.allowed_values,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(delete, path = "/api/scope-keys/{id}", params(("id" = String, Path,)),
    responses((status = 200, body = ScopeKeyMessage), (status = 404)))]
pub async fn delete_scope_key(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ScopeKeyMessage>> {
    if !insights::delete_scope_key(&state.pool, &user.id, &id).await? {
        return Err(AppError::NotFound("scope key".into()).into());
    }
    Ok(Json(ScopeKeyMessage {
        message: "Deleted".into(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/scope-keys",
            get(list_scope_keys).post(create_scope_key),
        )
        .route(
            "/api/scope-keys/{id}",
            axum::routing::delete(delete_scope_key),
        )
}
