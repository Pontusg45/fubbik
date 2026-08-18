//! Chunk proposals — suggested edits to a chunk, reviewed (approved or
//! rejected) by the chunk's owner. `chunk_proposal` is
//! `(id, chunk_id, changes jsonb, reason, status, proposed_by, reviewed_by,
//! reviewed_at, review_note, created_at)`; `changes` stores a
//! [`ProposedChanges`] blob wholesale, the same one-JSON-column shape
//! `collection.filter` uses (see `collection::CollectionFilter`'s doc
//! comment).
//!
//! **Creation and read access are deliberately global — this is not a
//! divergence, it is faithfully porting Node.** Node's `createProposal`
//! (`packages/api/src/proposals/service.ts:15-29`) never checks that the
//! caller owns (or that anyone owns) `chunkId` before inserting — any
//! authenticated user can propose changes to any chunk, and an unknown
//! `chunkId` fails only via the `chunk_proposal_chunk_id_chunk_id_fk`
//! foreign key (surfacing as a 500, not a 404 — see `create`'s doc
//! comment). `getProposal`, `listProposals`, and `listProposalsForChunk`
//! (`packages/api/src/proposals/service.ts:31-54`) carry no `user_id` /
//! `space_id` filter of any kind — the proposal queue is global across
//! every user. This port keeps all of that: `find_by_id`, `list`, and
//! `list_for_chunk` below take no `user_id` parameter at all.
//!
//! **Only the review step is scoped, and only by derivation through the
//! parent chunk — approve asymmetrically, reject not at all.** Node's
//! `approveProposal` (`packages/api/src/proposals/service.ts:56-78`) applies
//! the proposal's changes by calling `updateChunk(proposal.chunkId,
//! reviewerId, ...)`, whose `getChunkById(chunkId, userId)` is `WHERE id = ..
//! AND user_id = ..` — a non-owner's approve attempt 404s there, before the
//! proposal row is ever touched. `rejectProposal`
//! (`packages/api/src/proposals/service.ts:80-90`) does **not** call
//! `updateChunk` or check chunk ownership at all — any authenticated user
//! can reject any pending proposal regardless of who owns the underlying
//! chunk. This port mirrors both halves of that asymmetry exactly: see
//! `fubbik_api::proposals::service::approve` (which delegates to
//! `chunks::service::update` for the ownership-scoped write) and `::reject`
//! (which does not). Flagged for the human, not silently "fixed" — see the
//! phase report.
//!
//! **Approve is two sequential, non-atomic writes, matching Node exactly.**
//! `approveProposal` awaits `updateChunk(...)` (itself: version snapshot +
//! chunk `UPDATE`) and only then awaits `updateProposalStatus(...)`; there
//! is no transaction wrapping the pair in Node, and none is added here. If
//! the process dies between the two, the chunk carries the applied changes
//! but the proposal row is left `pending` forever (re-approving would
//! reapply the same changes on top of the chunk's now-already-updated
//! state). This is the same *shape* as divergence #12 (a non-atomic Node
//! pair some earlier phase wrapped in a transaction) — this port does
//! **not** make that call unilaterally; it is flagged in the phase report
//! for a human decision instead.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// The eight fields Node's `ProposedChanges` interface accepts
/// (`packages/db/src/schema/chunk-proposal.ts:40-49`), stored and echoed
/// back verbatim as one JSON blob — `chunk_proposal.changes` has no
/// per-field columns. `#[serde(skip_serializing_if = "Option::is_none")]`
/// on every field keeps a proposal that only touched one or two fields from
/// growing the rest back in as explicit `null`s on the way out, matching
/// Node's plain object (`Object.keys` only sees the keys actually present).
///
/// **Only `title`/`content`/`type`/`rationale`/`consequences` are ever
/// applied to the chunk on approve** — `tags`, `alternatives`, and `scope`
/// round-trip through this struct (create, list, get) but are silently
/// dropped when `approve` calls `chunks::service::update`, because
/// `ChunkPatch` (Phase 1) has no fields for them yet. Node's `updateChunk`
/// *does* apply all eight (`packages/api/src/chunks/chunk-mutations.ts:181-212`
/// handles `tags` via a separate `setChunkTags` tap, and `scope`/
/// `alternatives` pass straight through `UpdateChunkParams`). This is a real
/// behavioral gap versus Node, not a deliberate scoping choice — flagged in
/// the phase report rather than silently accepted.
#[derive(
    Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "camelCase", default)]
pub struct ProposedChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub proposed_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternatives: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consequences: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<std::collections::HashMap<String, String>>,
}

impl ProposedChanges {
    /// Mirrors Node's `Object.keys(body.changes).length === 0` guard
    /// (`packages/api/src/proposals/service.ts:17-18`): true only when every
    /// field is absent, exactly the same condition an empty `{}` JSON body
    /// produces.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.content.is_none()
            && self.proposed_type.is_none()
            && self.tags.is_none()
            && self.rationale.is_none()
            && self.alternatives.is_none()
            && self.consequences.is_none()
            && self.scope.is_none()
    }
}

/// `camelCase` serialisation matches every other wire type in this crate.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkProposal {
    pub id: String,
    pub chunk_id: String,
    #[schema(value_type = ProposedChanges)]
    pub changes: Json<ProposedChanges>,
    pub reason: Option<String>,
    pub status: String,
    pub proposed_by: String,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<UtcTimestamp>,
    pub review_note: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Row shape for `GET /api/proposals` — the global queue joins `chunk` for
/// `chunkTitle`/`chunkType` (`packages/db/src/repository/chunk-proposal.ts:41-63`),
/// which `find_by_id` and `list_for_chunk` do not carry. A separate struct
/// rather than optional extra fields on [`ChunkProposal`] keeps the two
/// shapes from being confusable at the type level.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProposalWithChunk {
    pub id: String,
    pub chunk_id: String,
    #[schema(value_type = ProposedChanges)]
    pub changes: Json<ProposedChanges>,
    pub reason: Option<String>,
    pub status: String,
    pub proposed_by: String,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<UtcTimestamp>,
    pub review_note: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    pub chunk_title: String,
    pub chunk_type: String,
}

pub struct NewProposal<'a> {
    pub chunk_id: &'a str,
    pub proposed_by: &'a str,
    pub changes: &'a ProposedChanges,
    pub reason: Option<&'a str>,
}

/// Plain insert, no ownership or even existence check on `chunk_id` — see
/// this module's doc comment. An unknown `chunk_id` fails the
/// `chunk_proposal_chunk_id_chunk_id_fk` foreign key, surfacing as
/// `AppError::Database` (500 "Internal server error"), the same outcome
/// Node's bare `db.insert(chunkProposal).values(input).returning()` produces
/// via `dbEffect`'s `DatabaseError` wrapping — not a 404, because nothing
/// here ever checked the chunk exists in the first place.
pub async fn create(pool: &PgPool, new: NewProposal<'_>) -> AppResult<ChunkProposal> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        ChunkProposal,
        r#"INSERT INTO chunk_proposal (id, chunk_id, changes, reason, status, proposed_by)
           VALUES ($1, $2, $3, $4, 'pending', $5)
           RETURNING id, chunk_id, changes AS "changes: Json<ProposedChanges>",
                     reason, status, proposed_by, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp", review_note,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        new.chunk_id,
        Json(new.changes) as _,
        new.reason,
        new.proposed_by
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// No `user_id` filter — see this module's doc comment on why the proposal
/// queue is global in Node and stays global here.
pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<ChunkProposal>> {
    let row = sqlx::query_as!(
        ChunkProposal,
        r#"SELECT id, chunk_id, changes AS "changes: Json<ProposedChanges>",
                  reason, status, proposed_by, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp", review_note,
                  created_at AS "created_at: UtcTimestamp"
           FROM chunk_proposal WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub struct ListProposalsFilter<'a> {
    pub chunk_id: Option<&'a str>,
    /// Always `Some` by the time this reaches the repo — the service layer
    /// defaults an absent query-string `status` to `"pending"`, matching
    /// Node's `filter.status ?? "pending"`
    /// (`packages/api/src/proposals/service.ts:45`). Unlike
    /// `list_for_chunk`'s `status`, this one has already been validated
    /// against `["pending", "approved", "rejected"]` by the caller.
    pub status: &'a str,
    pub limit: i64,
    pub offset: i64,
}

/// Backs `GET /api/proposals`, the global queue. `INNER JOIN chunk` mirrors
/// Node's `listProposals`
/// (`packages/db/src/repository/chunk-proposal.ts:41-63`) — a proposal whose
/// chunk has been deleted cannot appear (moot in practice: `chunk_id`
/// cascades on chunk delete, so the proposal row would already be gone).
///
/// `ORDER BY created_at DESC, id ASC` — Node orders by `desc(createdAt)`
/// alone with no secondary key; `, id ASC` is an added tiebreaker, the same
/// pattern as `notification::list` and `chunk::list`, needed because a burst
/// of proposals created in the same request (e.g. seed data, or several
/// created in the same millisecond) share a tied `created_at` and an
/// `ORDER BY` with no deterministic tiebreaker is a query-plan artifact, not
/// a stable order.
pub async fn list(pool: &PgPool, filter: ListProposalsFilter<'_>) -> AppResult<Vec<ProposalWithChunk>> {
    let rows = sqlx::query_as!(
        ProposalWithChunk,
        r#"SELECT p.id, p.chunk_id, p.changes AS "changes: Json<ProposedChanges>",
                  p.reason, p.status, p.proposed_by, p.reviewed_by,
                  p.reviewed_at AS "reviewed_at: UtcTimestamp", p.review_note,
                  p.created_at AS "created_at: UtcTimestamp",
                  c.title AS chunk_title, c.type AS chunk_type
           FROM chunk_proposal p
           INNER JOIN chunk c ON c.id = p.chunk_id
           WHERE p.status = $1 AND ($2::text IS NULL OR p.chunk_id = $2)
           ORDER BY p.created_at DESC, p.id ASC
           LIMIT $3 OFFSET $4"#,
        filter.status,
        filter.chunk_id,
        filter.limit,
        filter.offset
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Backs `GET /chunks/{id}/proposals`. `status` is passed straight through
/// with **no validation** — unlike `list`'s global queue, Node's
/// `listProposalsForChunk` (`packages/api/src/proposals/service.ts:52-54`)
/// forwards `status` to the repository unchecked, so `?status=bogus` here
/// silently returns zero rows rather than a 400. Not "fixed" — see this
/// module's doc comment on preserving Node's actual, sometimes-inconsistent
/// validation posture.
///
/// `ORDER BY created_at ASC, id ASC` — ascending, the opposite direction
/// from `list`'s global queue (`packages/db/src/repository/chunk-proposal.ts:67-78`'s
/// `asc(chunkProposal.createdAt)`). `, id ASC` is the same added tiebreaker.
pub async fn list_for_chunk(
    pool: &PgPool,
    chunk_id: &str,
    status: Option<&str>,
) -> AppResult<Vec<ChunkProposal>> {
    let rows = sqlx::query_as!(
        ChunkProposal,
        r#"SELECT id, chunk_id, changes AS "changes: Json<ProposedChanges>",
                  reason, status, proposed_by, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp", review_note,
                  created_at AS "created_at: UtcTimestamp"
           FROM chunk_proposal
           WHERE chunk_id = $1 AND ($2::text IS NULL OR status = $2)
           ORDER BY created_at ASC, id ASC"#,
        chunk_id,
        status
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Sets a proposal's terminal review state. Returns `None` — not an error —
/// if `id` doesn't exist, matching the *shape* of every other `Option`-
/// returning update in this crate; in practice this is unreachable through
/// the HTTP surface, because both `proposals::service::approve` and
/// `::reject` already fetched the proposal by id (404-ing if absent) before
/// reaching here — see this module's doc comment on the two-call,
/// non-atomic approve sequence.
pub async fn update_status(
    pool: &PgPool,
    id: &str,
    status: &str,
    reviewed_by: &str,
    review_note: Option<&str>,
) -> AppResult<Option<ChunkProposal>> {
    let row = sqlx::query_as!(
        ChunkProposal,
        r#"UPDATE chunk_proposal SET
             status = $2, reviewed_by = $3, reviewed_at = now(), review_note = $4
           WHERE id = $1
           RETURNING id, chunk_id, changes AS "changes: Json<ProposedChanges>",
                     reason, status, proposed_by, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp", review_note,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        status,
        reviewed_by,
        review_note
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Global pending count — no `user_id` filter, matching Node's
/// `getPendingCount` (`packages/db/src/repository/chunk-proposal.ts:102-107`),
/// which counts every user's pending proposals. Backs `GET /proposals/count`,
/// which the web dashboard's `stats-bar.tsx` reads as `{ pending }.pending`
/// — see `fubbik_api::proposals::routes::proposal_count`'s doc comment for
/// why the response is an object with a `pending` key, not a bare number.
pub async fn count_pending(pool: &PgPool) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_proposal WHERE status = 'pending'"#
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}
