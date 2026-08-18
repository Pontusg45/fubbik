//! See `fubbik_db::repo::proposal`'s module doc comment for the ownership
//! model this whole domain follows: create/read are global (no `user_id`
//! filter anywhere), only `approve` is scoped — and only by derivation
//! through the parent chunk, via `chunks::service::update`'s own
//! `WHERE user_id = ..` — while `reject` is not scoped at all. Both
//! asymmetries are faithful ports of Node, not bugs introduced here.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::proposal::{
    self, ChunkProposal, ListProposalsFilter, NewProposal, ProposalWithChunk, ProposedChanges,
};
use sqlx::PgPool;

use super::dto::{BulkAction, BulkActionBody};
use crate::chunks::dto::UpdateChunkBody;
use crate::chunks::service as chunk_service;

/// Mirrors Node's `createProposal`
/// (`packages/api/src/proposals/service.ts:15-29`): the only validation is
/// "changes is not empty"; there is deliberately no check that `chunk_id`
/// exists or belongs to `proposed_by` — see
/// `fubbik_db::repo::proposal::create`'s doc comment for what happens to an
/// unknown `chunk_id` instead.
pub async fn create_proposal(
    pool: &PgPool,
    chunk_id: &str,
    proposed_by: &str,
    changes: ProposedChanges,
    reason: Option<String>,
) -> AppResult<ChunkProposal> {
    if changes.is_empty() {
        return Err(AppError::Validation("changes must not be empty".into()));
    }
    proposal::create(
        pool,
        NewProposal {
            chunk_id,
            proposed_by,
            changes: &changes,
            reason: reason.as_deref(),
        },
    )
    .await
}

pub async fn get_proposal(pool: &PgPool, id: &str) -> AppResult<ChunkProposal> {
    proposal::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proposal".into()))
}

const VALID_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

/// Mirrors Node's `listProposals`
/// (`packages/api/src/proposals/service.ts:37-50`): an absent `status`
/// defaults to `"pending"` — this is the global queue, and Node deliberately
/// does not expose "every status" as the default — while a present-but-
/// invalid `status` is a 400, matching Node's explicit
/// `validStatuses.includes` check. Contrast with `list_for_chunk`, which
/// validates nothing.
pub async fn list_proposals(
    pool: &PgPool,
    chunk_id: Option<&str>,
    status: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<ProposalWithChunk>> {
    let status = status.unwrap_or("pending");
    if !VALID_STATUSES.contains(&status) {
        return Err(AppError::Validation(format!(
            "status must be one of: {}",
            VALID_STATUSES.join(", ")
        )));
    }
    proposal::list(
        pool,
        ListProposalsFilter {
            chunk_id,
            status,
            limit: limit.unwrap_or(50),
            offset: offset.unwrap_or(0),
        },
    )
    .await
}

/// Mirrors Node's `listProposalsForChunk`
/// (`packages/api/src/proposals/service.ts:52-54`): a thin pass-through,
/// `status` unvalidated and, when absent, meaning "every status" — not
/// defaulted to `"pending"` the way the global queue's `list` is.
pub async fn list_proposals_for_chunk(
    pool: &PgPool,
    chunk_id: &str,
    status: Option<&str>,
) -> AppResult<Vec<ChunkProposal>> {
    proposal::list_for_chunk(pool, chunk_id, status).await
}

/// Fetches the proposal and rejects it if not `pending` — the shared first
/// half of both `approve_proposal` and `reject_proposal`, matching the
/// identical `getProposalById(...).flatMap(status check)` prefix Node
/// repeats in both `approveProposal` and `rejectProposal`
/// (`packages/api/src/proposals/service.ts:57-63,81-87`).
async fn pending_proposal_or_error(pool: &PgPool, proposal_id: &str) -> AppResult<ChunkProposal> {
    let found = get_proposal(pool, proposal_id).await?;
    if found.status != "pending" {
        return Err(AppError::Validation(format!(
            "Proposal is already {}",
            found.status
        )));
    }
    Ok(found)
}

/// Mirrors Node's `approveProposal`
/// (`packages/api/src/proposals/service.ts:56-78`) exactly, including its
/// shape as **two sequential, non-atomic writes**: this applies the
/// proposal's changes to the chunk first (delegating to
/// `chunks::service::update`, which is where chunk ownership is actually
/// enforced — a non-owner `reviewer_id` 404s there and the proposal row is
/// never touched), and only then flips the proposal to `approved`. There is
/// no transaction wrapping the pair, matching Node; see this module's and
/// `fubbik_db::repo::proposal`'s doc comments for why that is flagged, not
/// silently changed.
///
/// Only `title`/`content`/`type`/`rationale`/`consequences` are actually
/// applied — `tags`/`alternatives`/`scope` on the proposal are accepted at
/// create time but dropped here, because `chunks::service::update`'s
/// `UpdateChunkBody` has no fields for them yet. See
/// `fubbik_db::repo::proposal::ProposedChanges`'s doc comment.
pub async fn approve_proposal(
    pool: &PgPool,
    proposal_id: &str,
    reviewer_id: &str,
    note: Option<String>,
) -> AppResult<ChunkProposal> {
    let found = pending_proposal_or_error(pool, proposal_id).await?;
    let changes = found.changes.0.clone();

    chunk_service::update(
        pool,
        reviewer_id,
        &found.chunk_id,
        UpdateChunkBody {
            title: changes.title,
            content: changes.content,
            chunk_type: changes.proposed_type,
            rationale: changes.rationale,
            consequences: changes.consequences,
        },
    )
    .await?;

    proposal::update_status(pool, proposal_id, "approved", reviewer_id, note.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound("Proposal".into()))
}

/// Mirrors Node's `rejectProposal`
/// (`packages/api/src/proposals/service.ts:80-90`) exactly: **no chunk
/// ownership check of any kind** — unlike `approve_proposal`, this never
/// calls into `chunks::service`, so any authenticated user can reject any
/// pending proposal regardless of who owns the underlying chunk. Not a bug
/// introduced by this port; see `fubbik_db::repo::proposal`'s module doc
/// comment.
pub async fn reject_proposal(
    pool: &PgPool,
    proposal_id: &str,
    reviewer_id: &str,
    note: Option<String>,
) -> AppResult<ChunkProposal> {
    pending_proposal_or_error(pool, proposal_id).await?;

    proposal::update_status(pool, proposal_id, "rejected", reviewer_id, note.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound("Proposal".into()))
}

/// Mirrors Node's `bulkAction`
/// (`packages/api/src/proposals/service.ts:92-99`): sequential, fail-fast —
/// the first `approve`/`reject` error aborts the whole request, and any
/// actions already applied before that point stay applied (no transaction
/// wraps the loop, matching Node's `Effect.forEach(..., { concurrency: 1 })`
/// semantics, which does not retroactively undo already-completed effects
/// in the array just because a later one fails).
pub async fn bulk_action(
    pool: &PgPool,
    reviewer_id: &str,
    body: BulkActionBody,
) -> AppResult<Vec<ChunkProposal>> {
    let mut results = Vec::with_capacity(body.actions.len());
    for item in body.actions {
        let result = match item.action {
            BulkAction::Approve => {
                approve_proposal(pool, &item.proposal_id, reviewer_id, item.note).await?
            }
            BulkAction::Reject => {
                reject_proposal(pool, &item.proposal_id, reviewer_id, item.note).await?
            }
        };
        results.push(result);
    }
    Ok(results)
}

/// Global pending count — see `fubbik_db::repo::proposal::count_pending`'s
/// doc comment for why this has no `user_id` filter.
pub async fn pending_count(pool: &PgPool) -> AppResult<i64> {
    proposal::count_pending(pool).await
}
