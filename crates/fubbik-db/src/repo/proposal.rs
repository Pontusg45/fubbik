//! Chunk proposals — suggested edits to a chunk, reviewed (approved or
//! rejected) by the chunk's owner. `chunk_proposal` is
//! `(id, chunk_id, changes jsonb, reason, status, proposed_by, reviewed_by,
//! reviewed_at, review_note, created_at)`; `changes` stores a
//! [`ProposedChanges`] blob wholesale, the same one-JSON-column shape
//! `collection.filter` uses (see `collection::CollectionFilter`'s doc
//! comment).
//!
//! **Creation stays deliberately global, faithfully porting Node.** Node's
//! `createProposal` (`packages/api/src/proposals/service.ts:15-29`) never
//! checks that the caller owns (or that anyone owns) `chunkId` before
//! inserting — any authenticated user can propose changes to any chunk, and
//! an unknown `chunkId` fails only via the `chunk_proposal_chunk_id_chunk_id_fk`
//! foreign key (surfacing as a 500, not a 404 — see `create`'s doc comment).
//! `list_for_chunk` (`GET /chunks/{id}/proposals`) is unchanged too — still
//! no `user_id` filter, matching Node's `listProposalsForChunk`.
//!
//! **`list` (the global queue, `GET /proposals`) and the single-proposal
//! lookup backing `GET /proposals/{id}` are now scoped to the caller —
//! a deliberate Phase 2e wave-1 divergence from Node, the same shape as
//! accepted divergences #4/#9/#10/#13/#14/#15/#17/#19.** Node's
//! `getProposal`/`listProposals` (`packages/api/src/proposals/service.ts:31-50`)
//! carry no `user_id` filter at all, so any authenticated user could
//! previously read every other user's proposal queue. `list` now joins
//! `chunk` on `c.user_id = $user_id`; the id lookup for `GET /proposals/{id}`
//! goes through the new [`find_by_id_for_owner`] rather than the still-unscoped
//! [`find_by_id`] (which stays as-is: `approve`/`reject`'s internal
//! pending-check deliberately keeps using the unscoped form, since ownership
//! for those two is enforced independently at the write layer below).
//!
//! **`reject` is now scoped through the parent chunk, closing the asymmetry
//! with `approve` — also a deliberate divergence from Node, same shape as
//! the list above.** Node's `rejectProposal`
//! (`packages/api/src/proposals/service.ts:80-90`) never calls `updateChunk`
//! or checks chunk ownership at all; this port's [`reject`] now carries its
//! own `EXISTS`-through-`chunk` guard on the `UPDATE`, independent of
//! `approve`'s guard (which lives on the chunk `UPDATE` inside [`approve`]).
//!
//! **`approve` is now one atomic transaction, not two sequential writes.**
//! Node's `approveProposal` awaits `updateChunk(...)` and only then awaits
//! `updateProposalStatus(...)` with no transaction wrapping the pair — the
//! same *shape* as divergence #12, which an earlier phase already decided to
//! fix rather than reproduce. [`approve`] wraps the chunk-version-snapshot +
//! chunk `UPDATE` + tag replace + proposal-status `UPDATE` in a single
//! `sqlx` transaction: either all of it commits, or none of it does. All
//! eight `ProposedChanges` fields are now applied (previously only five were
//! — see [`ProposedChanges`]'s doc comment for the data-loss bug this
//! closes).

use fubbik_core::error::AppResult;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

use crate::timestamp::UtcTimestamp;

/// The eight fields Node's `ProposedChanges` interface accepts
/// (`packages/db/src/schema/chunk-proposal.ts:40-49`), stored and echoed
/// back verbatim as one JSON blob — `chunk_proposal.changes` has no
/// per-field columns. `#[serde(skip_serializing_if = "Option::is_none")]`
/// on every field keeps a proposal that only touched one or two fields from
/// growing the rest back in as explicit `null`s on the way out, matching
/// Node's plain object (`Object.keys` only sees the keys actually present).
///
/// **All eight fields are now applied to the chunk on approve** —
/// `title`/`content`/`type`/`rationale`/`consequences`/`alternatives`/`scope`
/// via [`approve`]'s chunk `UPDATE`, and `tags` via the same `UPDATE`'s
/// find-or-create-then-replace pass over `chunk_tag`. Previously only the
/// first five were applied and `tags`/`alternatives`/`scope` were silently
/// discarded, because `ChunkPatch` (Phase 1) had no fields for them and
/// `approve` had no path for the join table either — a real data-loss bug
/// versus Node's `updateChunk`, which applies all eight
/// (`packages/api/src/chunks/chunk-mutations.ts:181-212` handles `tags` via
/// a separate `setChunkTags` tap; `scope`/`alternatives` pass straight
/// through `UpdateChunkParams`). Closed in Phase 2e wave 1.
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
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
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

/// Scoped counterpart to [`find_by_id`], backing `GET /proposals/{id}` —
/// see this module's doc comment for why this is a separate function
/// rather than a change to `find_by_id` itself (which stays unscoped for
/// `approve`/`reject`'s internal pending-check; ownership for those two is
/// enforced independently, at the write layer). A caller who doesn't own
/// the proposal's underlying chunk gets `Ok(None)`, indistinguishable from
/// an unknown id — both map to the same 404 at the service layer.
pub async fn find_by_id_for_owner(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<Option<ChunkProposal>> {
    let row = sqlx::query_as!(
        ChunkProposal,
        r#"SELECT p.id, p.chunk_id, p.changes AS "changes: Json<ProposedChanges>",
                  p.reason, p.status, p.proposed_by, p.reviewed_by,
                  p.reviewed_at AS "reviewed_at: UtcTimestamp", p.review_note,
                  p.created_at AS "created_at: UtcTimestamp"
           FROM chunk_proposal p
           WHERE p.id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = p.chunk_id AND c.user_id = $2)"#,
        id,
        user_id
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

/// Backs `GET /api/proposals`, the global queue — scoped to the caller
/// (`c.user_id = $1`), a deliberate divergence from Node; see this module's
/// doc comment. `INNER JOIN chunk` otherwise mirrors Node's `listProposals`
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
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    filter: ListProposalsFilter<'_>,
) -> AppResult<Vec<ProposalWithChunk>> {
    let rows = sqlx::query_as!(
        ProposalWithChunk,
        r#"SELECT p.id, p.chunk_id, p.changes AS "changes: Json<ProposedChanges>",
                  p.reason, p.status, p.proposed_by, p.reviewed_by,
                  p.reviewed_at AS "reviewed_at: UtcTimestamp", p.review_note,
                  p.created_at AS "created_at: UtcTimestamp",
                  c.title AS chunk_title, c.type AS chunk_type
           FROM chunk_proposal p
           INNER JOIN chunk c ON c.id = p.chunk_id
           WHERE c.user_id = $1 AND p.status = $2 AND ($3::text IS NULL OR p.chunk_id = $3)
           ORDER BY p.created_at DESC, p.id ASC
           LIMIT $4 OFFSET $5"#,
        user_id,
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

/// Bare, unscoped terminal-review-state setter. Returns `None` — not an
/// error — if `id` doesn't exist, matching the *shape* of every other
/// `Option`-returning update in this crate.
///
/// No longer called by `proposals::service::approve`/`::reject` — those now
/// go through [`approve`] (one atomic transaction) and [`reject`] (its own
/// chunk-ownership-scoped `UPDATE`) respectively. Kept as a low-level,
/// unscoped primitive (and its own direct test coverage) rather than
/// removed outright: nothing in this port's behaviour depends on it being
/// gone, and deleting a working, independently-useful function isn't part
/// of either fix.
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

/// Rejects a pending proposal, scoped through its parent chunk — the SQL
/// counterpart to [`approve`]'s chunk-owned `UPDATE`, closing the asymmetry
/// this module's doc comment used to flag: Node's `rejectProposal`
/// (`packages/api/src/proposals/service.ts:80-90`) has no ownership check of
/// any kind. This port now diverges deliberately (same shape as accepted
/// divergences #4/#9/#10/#13/#14/#15/#17/#19): `EXISTS (... c.user_id = $2)`
/// is bound against `chunk_proposal.chunk_id`, the same "guard through the
/// parent" shape `use_case::create`'s `space_id` guard and
/// `tag::set_chunk_tags`'s ownership `EXISTS` use both follow.
///
/// A reviewer who doesn't own the underlying chunk matches zero rows and
/// gets `Ok(None)` — not an error, identical to an unknown `id` — so the
/// service layer's single `.ok_or_else(NotFound)` handles both cases the
/// same way `approve`'s chunk-ownership-miss already does.
pub async fn reject(
    pool: &PgPool,
    id: &str,
    reviewer_id: &str,
    review_note: Option<&str>,
) -> AppResult<Option<ChunkProposal>> {
    let mut conn = pool.acquire().await?;
    reject_in_transaction(&mut conn, id, reviewer_id, review_note).await
}

async fn reject_in_transaction(
    conn: &mut PgConnection,
    id: &str,
    reviewer_id: &str,
    review_note: Option<&str>,
) -> AppResult<Option<ChunkProposal>> {
    let row = sqlx::query_as!(
        ChunkProposal,
        r#"UPDATE chunk_proposal SET
             status = 'rejected', reviewed_by = $2, reviewed_at = now(), review_note = $3
           WHERE id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = chunk_proposal.chunk_id AND c.user_id = $2)
           RETURNING id, chunk_id, changes AS "changes: Json<ProposedChanges>",
                     reason, status, proposed_by, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp", review_note,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        reviewer_id,
        review_note
    )
    .fetch_optional(conn)
    .await?;
    Ok(row)
}

/// The chunk-side changes [`approve`] applies inside its transaction.
/// Mirrors `chunk::ChunkPatch`'s five scalar fields plus the two it grew for
/// this fix (`alternatives`/`scope`), plus `tags` — never a `ChunkPatch`
/// field, since `chunk_tag` is a join table, not a column (see
/// `tag::set_chunk_tags`'s doc comment for the ownership pattern this
/// mirrors). `tags` holds tag *names*, matching `ProposedChanges::tags` and
/// Node's `findOrCreateTag(name, userId)` resolution — not tag ids.
pub struct ApproveChunkChanges {
    pub title: Option<String>,
    pub content: Option<String>,
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub alternatives: Option<Vec<String>>,
    pub scope: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

/// Applies a pending proposal's changes to its chunk and flips the proposal
/// to `approved` — all in **one transaction**, closing the non-atomicity
/// this module's doc comment used to flag (the same *shape* as divergence
/// #12). A failure between the chunk write and the proposal-status write can
/// no longer leave the chunk changed with the proposal still `pending`:
/// either both commit, or the whole transaction rolls back and neither does.
///
/// Ownership is enforced by the chunk `UPDATE`'s own
/// `WHERE id = $1 AND user_id = $2` — the same guard `chunk::update` uses.
/// A non-owner `reviewer_id` matches zero rows on the very first read (the
/// pre-edit snapshot fetch), the whole transaction rolls back, and this
/// returns `Ok(None)` without ever touching the proposal row — the same
/// "reject the write before it reaches the proposal" behaviour `approve` had
/// before this fix, just derived one step earlier.
///
/// All eight `ProposedChanges` fields Node's `approveProposal` applies are
/// now carried through: five plain chunk columns, two more added by this
/// fix (`alternatives`/`scope`), and `tags` — a join-table replace via the
/// same delete-then-insert, ownership-guarded-through-both-parents shape as
/// `tag::set_chunk_tags` (this cannot literally call that function: it
/// commits its own transaction internally, which cannot compose with this
/// one). Tag names with no existing `(name, user_id)` row for `reviewer_id`
/// are created on the fly, mirroring Node's `findOrCreateTag`.
pub async fn approve(
    pool: &PgPool,
    proposal_id: &str,
    chunk_id: &str,
    reviewer_id: &str,
    changes: ApproveChunkChanges,
    note: Option<&str>,
) -> AppResult<Option<ChunkProposal>> {
    let mut tx = pool.begin().await?;
    let proposal =
        approve_in_transaction(&mut tx, proposal_id, chunk_id, reviewer_id, changes, note).await?;
    tx.commit().await?;
    Ok(proposal)
}

async fn approve_in_transaction(
    conn: &mut PgConnection,
    proposal_id: &str,
    chunk_id: &str,
    reviewer_id: &str,
    changes: ApproveChunkChanges,
    note: Option<&str>,
) -> AppResult<Option<ChunkProposal>> {
    // Pre-edit snapshot for chunk_version, scoped by owner in the same
    // breath — a non-owner reviewer_id matches nothing here, and the
    // transaction below is rolled back before any write happens.
    let current = sqlx::query!(
        r#"SELECT title, content, type, rationale, consequences
           FROM chunk WHERE id = $1 AND user_id = $2"#,
        chunk_id,
        reviewer_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(current) = current else {
        return Ok(None);
    };

    let version_id = crate::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_version
             (id, chunk_id, version, title, content, type, tags,
              rationale, consequences, created_at)
           SELECT $1, $2, COALESCE(MAX(v.version), 0) + 1, $3, $4, $5,
                  '[]'::jsonb, $6, $7, now()
           FROM chunk_version v WHERE v.chunk_id = $2"#,
        version_id,
        chunk_id,
        current.title,
        current.content,
        current.r#type,
        current.rationale,
        current.consequences
    )
    .execute(&mut *conn)
    .await?;

    sqlx::query!(
        r#"UPDATE chunk SET
             title = COALESCE($3, title),
             content = COALESCE($4, content),
             type = COALESCE($5, type),
             rationale = COALESCE($6, rationale),
             consequences = COALESCE($7, consequences),
             alternatives = COALESCE($8, alternatives),
             scope = COALESCE($9, scope),
             updated_at = now()
           WHERE id = $1 AND user_id = $2"#,
        chunk_id,
        reviewer_id,
        changes.title,
        changes.content,
        changes.chunk_type,
        changes.rationale,
        changes.consequences,
        changes.alternatives.map(Json) as _,
        changes.scope.map(Json) as _
    )
    .execute(&mut *conn)
    .await?;

    if let Some(names) = &changes.tags {
        let mut tag_ids: Vec<String> = Vec::with_capacity(names.len());
        for name in names {
            let existing = sqlx::query_scalar!(
                "SELECT id FROM tag WHERE name = $1 AND user_id = $2",
                name,
                reviewer_id
            )
            .fetch_optional(&mut *conn)
            .await?;
            let tag_id = match existing {
                Some(id) => id,
                None => {
                    let id = crate::new_id();
                    sqlx::query!(
                        "INSERT INTO tag (id, name, user_id) VALUES ($1, $2, $3)",
                        id,
                        name,
                        reviewer_id
                    )
                    .execute(&mut *conn)
                    .await?;
                    id
                }
            };
            tag_ids.push(tag_id);
        }

        sqlx::query!("DELETE FROM chunk_tag WHERE chunk_id = $1", chunk_id)
            .execute(&mut *conn)
            .await?;
        if !tag_ids.is_empty() {
            sqlx::query!(
                r#"INSERT INTO chunk_tag (chunk_id, tag_id)
                   SELECT $1, t FROM unnest($2::text[]) AS t
                   ON CONFLICT (chunk_id, tag_id) DO NOTHING"#,
                chunk_id,
                &tag_ids
            )
            .execute(&mut *conn)
            .await?;
        }
    }

    let proposal = sqlx::query_as!(
        ChunkProposal,
        r#"UPDATE chunk_proposal SET
             status = 'approved', reviewed_by = $2, reviewed_at = now(), review_note = $3
           WHERE id = $1
           RETURNING id, chunk_id, changes AS "changes: Json<ProposedChanges>",
                     reason, status, proposed_by, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp", review_note,
                     created_at AS "created_at: UtcTimestamp""#,
        proposal_id,
        reviewer_id,
        note
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(proposal)
}

pub enum BulkReviewAction {
    Approve,
    Reject,
}

pub struct BulkReviewItem {
    pub proposal_id: String,
    pub action: BulkReviewAction,
    pub note: Option<String>,
}

/// Reviews a batch under one transaction. Every proposal is locked before
/// its mutation, and any validation, ownership, or database failure rolls
/// the complete batch back.
pub async fn review_bulk(
    pool: &PgPool,
    reviewer_id: &str,
    actions: Vec<BulkReviewItem>,
) -> AppResult<Vec<ChunkProposal>> {
    let mut tx = pool.begin().await?;
    let mut results = Vec::with_capacity(actions.len());

    for item in actions {
        let found = sqlx::query_as::<_, ChunkProposal>(
            r#"SELECT id, chunk_id, changes, reason, status, proposed_by,
                      reviewed_by, reviewed_at, review_note, created_at
               FROM chunk_proposal WHERE id = $1 FOR UPDATE"#,
        )
        .bind(&item.proposal_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| fubbik_core::error::AppError::NotFound("Proposal".into()))?;

        if found.status != "pending" {
            return Err(fubbik_core::error::AppError::Validation(format!(
                "Proposal is already {}",
                found.status
            )));
        }

        let reviewed = match item.action {
            BulkReviewAction::Approve => {
                let changes = found.changes.0.clone();
                approve_in_transaction(
                    &mut tx,
                    &found.id,
                    &found.chunk_id,
                    reviewer_id,
                    ApproveChunkChanges {
                        title: changes.title,
                        content: changes.content,
                        chunk_type: changes.proposed_type,
                        rationale: changes.rationale,
                        consequences: changes.consequences,
                        alternatives: changes.alternatives,
                        scope: changes.scope.map(|scope| {
                            serde_json::to_value(scope)
                                .expect("HashMap<String, String> serialises infallibly")
                        }),
                        tags: changes.tags,
                    },
                    item.note.as_deref(),
                )
                .await?
                .ok_or_else(|| fubbik_core::error::AppError::NotFound("chunk".into()))?
            }
            BulkReviewAction::Reject => {
                reject_in_transaction(&mut tx, &found.id, reviewer_id, item.note.as_deref())
                    .await?
                    .ok_or_else(|| fubbik_core::error::AppError::NotFound("Proposal".into()))?
            }
        };
        results.push(reviewed);
    }

    tx.commit().await?;
    Ok(results)
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
