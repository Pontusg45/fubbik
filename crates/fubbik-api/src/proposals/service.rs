//! See `fubbik_db::repo::proposal`'s module doc comment for the ownership
//! model this whole domain follows: `create`/`list_for_chunk` stay global
//! (no `user_id` filter, faithfully matching Node), while `list`/`get`
//! (Phase 2e wave 1) and `reject` (also wave 1) are now scoped to the
//! caller, and `approve` runs as one atomic transaction instead of two
//! sequential writes.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::proposal::{
    self, ApproveChunkChanges, ChunkProposal, ListProposalsFilter, NewProposal, ProposalWithChunk,
    ProposedChanges,
};
use sqlx::PgPool;

use super::dto::{BulkAction, BulkActionBody};

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

/// Backs `GET /proposals/{id}` — scoped to the caller (Phase 2e wave 1: any
/// authenticated user could previously fetch any other user's proposal by
/// id, matching Node's unscoped `getProposal`; this is a deliberate
/// divergence now, same shape as accepted divergences #4/#9/#10/#13/#14/
/// #15/#17/#19). Uses `proposal::find_by_id_for_owner`, **not** the plain
/// `proposal::find_by_id` `pending_proposal_or_error` uses internally for
/// `approve`/`reject` — see `fubbik_db::repo::proposal`'s module doc
/// comment for why those two intentionally stay on the unscoped lookup.
pub async fn get_proposal(pool: &PgPool, user_id: &str, id: &str) -> AppResult<ChunkProposal> {
    proposal::find_by_id_for_owner(pool, user_id, id)
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
///
/// Now also scoped to the caller (Phase 2e wave 1 — see `get_proposal`'s doc
/// comment for the same divergence, applied here to the global queue).
pub async fn list_proposals(
    pool: &PgPool,
    user_id: &str,
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
        user_id,
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
///
/// Deliberately uses the **unscoped** `proposal::find_by_id`, not
/// `get_proposal`/`find_by_id_for_owner` above — ownership for both
/// `approve` and `reject` is enforced independently, at the write layer
/// (`proposal::approve`'s chunk-owned `UPDATE`, `proposal::reject`'s
/// `EXISTS`-through-chunk guard), not by this pre-check. See
/// `fubbik_db::repo::proposal`'s module doc comment.
async fn pending_proposal_or_error(pool: &PgPool, proposal_id: &str) -> AppResult<ChunkProposal> {
    let found = fubbik_db::repo::proposal::find_by_id(pool, proposal_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proposal".into()))?;
    if found.status != "pending" {
        return Err(AppError::Validation(format!(
            "Proposal is already {}",
            found.status
        )));
    }
    Ok(found)
}

/// Mirrors Node's `approveProposal`
/// (`packages/api/src/proposals/service.ts:56-78`) in intent, but no longer
/// in its non-atomic *shape*: this now delegates to `proposal::approve`,
/// which applies the proposal's changes to the chunk and flips the proposal
/// to `approved` inside **one transaction** (Phase 2e wave 1 — see
/// `fubbik_db::repo::proposal`'s module doc comment). A non-owner
/// `reviewer_id` still 404s before any write commits, and the proposal row
/// is still never touched in that case — same observable behaviour as
/// before, just derived atomically instead of via two sequential calls.
///
/// All eight `ProposedChanges` fields are now applied — `tags`, `alternatives`,
/// and `scope` used to be silently dropped here; see
/// `fubbik_db::repo::proposal::ProposedChanges`'s doc comment for that fix.
pub async fn approve_proposal(
    pool: &PgPool,
    proposal_id: &str,
    reviewer_id: &str,
    note: Option<String>,
) -> AppResult<ChunkProposal> {
    let found = pending_proposal_or_error(pool, proposal_id).await?;
    let changes = found.changes.0.clone();

    proposal::approve(
        pool,
        proposal_id,
        &found.chunk_id,
        reviewer_id,
        ApproveChunkChanges {
            title: changes.title,
            content: changes.content,
            chunk_type: changes.proposed_type,
            rationale: changes.rationale,
            consequences: changes.consequences,
            alternatives: changes.alternatives,
            scope: changes.scope.map(|m| {
                serde_json::to_value(m).expect("HashMap<String, String> serialises infallibly")
            }),
            tags: changes.tags,
        },
        note.as_deref(),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("chunk".into()))
}

/// Mirrors Node's `rejectProposal`
/// (`packages/api/src/proposals/service.ts:80-90`) in intent, but no longer
/// in its unscoped *shape*: Node has **no chunk ownership check of any
/// kind** on reject, so any authenticated user can reject any pending
/// proposal regardless of who owns the underlying chunk. Phase 2e wave 1
/// closes that — a deliberate divergence, same shape as accepted
/// divergences #4/#9/#10/#13/#14/#15/#17/#19 — via `proposal::reject`'s
/// `EXISTS`-through-chunk SQL guard, mirroring how `approve` is scoped.
pub async fn reject_proposal(
    pool: &PgPool,
    proposal_id: &str,
    reviewer_id: &str,
    note: Option<String>,
) -> AppResult<ChunkProposal> {
    pending_proposal_or_error(pool, proposal_id).await?;

    proposal::reject(pool, proposal_id, reviewer_id, note.as_deref())
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
