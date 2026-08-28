//! `GET /api/spaces/{id}/generate-instructions?format=claude|agents|cursor`.
//! See the module doc comment on `generate_instructions` for the two
//! deliberate divergences from Node (route path, ownership check).

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use super::service::{self, GenerateInstructionsResponse, InstructionFormat};
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct GenerateInstructionsQuery {
    pub format: Option<InstructionFormat>,
}

/// `unknown_format_is_rejected_or_defaults`: an unrecognised `format`
/// value fails to deserialize against `InstructionFormat`'s three
/// literals, so `extract::Query` rejects the request with
/// `AppError::Validation` -> `400`, the same "reject" behaviour Node's
/// Elysia `t.Union([t.Literal(...), ...])` query schema produces (Elysia
/// responds non-2xx for a query value outside the declared union before
/// the handler body ever runs) — not a silent default to `claude`.
#[utoipa::path(get, path = "/api/spaces/{id}/generate-instructions",
    params(("id" = String, Path,), GenerateInstructionsQuery),
    responses((status = 200, body = GenerateInstructionsResponse), (status = 404)))]
pub async fn generate_instructions_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Query(query): Query<GenerateInstructionsQuery>,
) -> ApiResult<Json<GenerateInstructionsResponse>> {
    let result = service::generate_instructions(&state.pool, &user.id, &id, query.format).await?;
    Ok(Json(result))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/spaces/{id}/generate-instructions",
        get(generate_instructions_route),
    )
}
