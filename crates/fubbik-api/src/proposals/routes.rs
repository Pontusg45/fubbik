//! Route wiring for the proposals domain
//! (`packages/api/src/proposals/routes.ts`). Every handler here returns the
//! default 200 — Node's `proposalRoutes` never calls `ctx.set.status`
//! anywhere in this file (unlike, say, `collections`/`tags`/`workspaces`'
//! create routes, which explicitly set 201), so none of these — including
//! `create_proposal` and `bulk_action_proposals`, both `POST`s that create
//! or mutate rows — return `StatusCode::CREATED`.
//!
//! Route ordering doesn't matter for axum's matchit-based router the way it
//! does for Elysia (static segments always win over path params regardless
//! of registration order), but `/api/proposals/count` and
//! `/api/proposals/bulk` are still registered as their own literal routes
//! ahead of `/api/proposals/{proposalId}` here, mirroring Node's explicit
//! "MUST be before" comments (`packages/api/src/proposals/routes.ts:56,65`)
//! for readability.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_db::repo::proposal::ChunkProposal;

use super::dto::{
    BulkActionBody, CreateProposalBody, ListChunkProposalsQuery, ListProposalsQuery,
    PendingCountResponse, ReviewBody,
};
use super::service;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::{Json as ReqJson, Query};

#[utoipa::path(post, path = "/api/chunks/{id}/proposals", request_body = CreateProposalBody,
    params(("id" = String, Path,)), responses((status = 200, body = ChunkProposal), (status = 400)))]
pub async fn create_proposal(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ReqJson(body): ReqJson<CreateProposalBody>,
) -> ApiResult<Json<ChunkProposal>> {
    Ok(Json(
        service::create_proposal(&state.pool, &id, &user.id, body.changes, body.reason).await?,
    ))
}

#[utoipa::path(get, path = "/api/chunks/{id}/proposals",
    params(("id" = String, Path,), ListChunkProposalsQuery),
    responses((status = 200, body = Vec<ChunkProposal>)))]
pub async fn list_chunk_proposals(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    Path(id): Path<String>,
    Query(query): Query<ListChunkProposalsQuery>,
) -> ApiResult<Json<Vec<ChunkProposal>>> {
    Ok(Json(
        service::list_proposals_for_chunk(&state.pool, &id, query.status.as_deref()).await?,
    ))
}

/// `{ pending: N }` — see `dto::PendingCountResponse`'s doc comment for why
/// this is the shape, and why it matters (`stats-bar.tsx` reads `.pending`).
#[utoipa::path(get, path = "/api/proposals/count",
    responses((status = 200, body = PendingCountResponse)))]
pub async fn proposal_count(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
) -> ApiResult<Json<PendingCountResponse>> {
    Ok(Json(PendingCountResponse {
        pending: service::pending_count(&state.pool).await?,
    }))
}

#[utoipa::path(post, path = "/api/proposals/bulk", request_body = BulkActionBody,
    responses((status = 200, body = Vec<ChunkProposal>), (status = 400), (status = 404)))]
pub async fn bulk_action_proposals(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<BulkActionBody>,
) -> ApiResult<Json<Vec<ChunkProposal>>> {
    Ok(Json(
        service::bulk_action(&state.pool, &user.id, body).await?,
    ))
}

/// The global proposal queue — `status` defaults to `"pending"`, not "every
/// status", and (Phase 2e wave 1) is scoped to the caller: only proposals on
/// chunks the caller owns appear. See `service::list_proposals`'s doc
/// comment.
#[utoipa::path(get, path = "/api/proposals", params(ListProposalsQuery),
    responses((status = 200, body = Vec<fubbik_db::repo::proposal::ProposalWithChunk>), (status = 400)))]
pub async fn list_proposals(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListProposalsQuery>,
) -> ApiResult<Json<Vec<fubbik_db::repo::proposal::ProposalWithChunk>>> {
    Ok(Json(
        service::list_proposals(
            &state.pool,
            &user.id,
            query.chunk_id.as_deref(),
            query.status.as_deref(),
            query.limit,
            query.offset,
        )
        .await?,
    ))
}

/// Scoped to the caller (Phase 2e wave 1) — a proposal on a chunk the
/// caller doesn't own 404s, matching Node no longer. See
/// `service::get_proposal`'s doc comment.
#[utoipa::path(get, path = "/api/proposals/{proposalId}",
    params(("proposalId" = String, Path,)),
    responses((status = 200, body = ChunkProposal), (status = 404)))]
pub async fn get_proposal(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(proposal_id): Path<String>,
) -> ApiResult<Json<ChunkProposal>> {
    Ok(Json(
        service::get_proposal(&state.pool, &user.id, &proposal_id).await?,
    ))
}

/// Applies the proposal's changes to the underlying chunk and flips the
/// proposal to `approved` in one atomic transaction (Phase 2e wave 1 — see
/// `service::approve_proposal`'s doc comment). A caller who does not own the
/// chunk 404s here, and the proposal row is left untouched.
#[utoipa::path(post, path = "/api/proposals/{proposalId}/approve", request_body = ReviewBody,
    params(("proposalId" = String, Path,)),
    responses((status = 200, body = ChunkProposal), (status = 400), (status = 404)))]
pub async fn approve_proposal(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(proposal_id): Path<String>,
    ReqJson(body): ReqJson<ReviewBody>,
) -> ApiResult<Json<ChunkProposal>> {
    Ok(Json(
        service::approve_proposal(&state.pool, &proposal_id, &user.id, body.note).await?,
    ))
}

/// Scoped through the parent chunk (Phase 2e wave 1) — a caller who does
/// not own the chunk 404s here, closing the asymmetry with `approve` Node
/// itself has. See `service::reject_proposal`'s doc comment.
#[utoipa::path(post, path = "/api/proposals/{proposalId}/reject", request_body = ReviewBody,
    params(("proposalId" = String, Path,)),
    responses((status = 200, body = ChunkProposal), (status = 400), (status = 404)))]
pub async fn reject_proposal(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(proposal_id): Path<String>,
    ReqJson(body): ReqJson<ReviewBody>,
) -> ApiResult<Json<ChunkProposal>> {
    Ok(Json(
        service::reject_proposal(&state.pool, &proposal_id, &user.id, body.note).await?,
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/chunks/{id}/proposals",
            post(create_proposal).get(list_chunk_proposals),
        )
        .route("/api/proposals/count", get(proposal_count))
        .route("/api/proposals/bulk", post(bulk_action_proposals))
        .route("/api/proposals", get(list_proposals))
        .route("/api/proposals/{proposalId}", get(get_proposal))
        .route(
            "/api/proposals/{proposalId}/approve",
            post(approve_proposal),
        )
        .route("/api/proposals/{proposalId}/reject", post(reject_proposal))
}
