use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

use super::dto::{
    AiConnectionSuggestion, ChunkIdBody, GenerateBody, GeneratedChunk, StructureRequirementBody,
    StructuredRequirement, SummaryResponse,
};

#[utoipa::path(post, path = "/api/ai/summarize", request_body = ChunkIdBody,
    responses((status = 200, body = SummaryResponse), (status = 404)))]
pub async fn summarize(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ChunkIdBody>,
) -> ApiResult<Json<SummaryResponse>> {
    Ok(Json(
        super::service::summarize(&state.pool, &state.ai, &user.id, &body.chunk_id).await?,
    ))
}

#[utoipa::path(post, path = "/api/ai/suggest-connections", request_body = ChunkIdBody,
    responses((status = 200, body = Vec<AiConnectionSuggestion>), (status = 404)))]
pub async fn suggest_connections(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ChunkIdBody>,
) -> ApiResult<Json<Vec<AiConnectionSuggestion>>> {
    Ok(Json(
        super::service::suggest_connections(&state.pool, &state.ai, &user.id, &body.chunk_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/ai/generate", request_body = GenerateBody,
    responses((status = 200, body = GeneratedChunk)))]
pub async fn generate(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    ReqJson(body): ReqJson<GenerateBody>,
) -> ApiResult<Json<GeneratedChunk>> {
    Ok(Json(
        super::service::generate(&state.ai, &body.prompt).await?,
    ))
}

#[utoipa::path(post, path = "/api/ai/structure-requirement", request_body = StructureRequirementBody,
    responses((status = 200, body = StructuredRequirement), (status = 400)))]
pub async fn structure_requirement(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<StructureRequirementBody>,
) -> ApiResult<Json<StructuredRequirement>> {
    Ok(Json(
        super::service::structure_requirement(
            &state.pool,
            &state.ai,
            &user.id,
            &body.description,
            body.space_id.as_deref(),
        )
        .await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ai/summarize", post(summarize))
        .route("/api/ai/suggest-connections", post(suggest_connections))
        .route("/api/ai/generate", post(generate))
        .route("/api/ai/structure-requirement", post(structure_requirement))
}
