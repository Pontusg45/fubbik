//! `chunk_staleness` (flags) and `staleness_scan` (per-space scan
//! bookkeeping, not yet surfaced by any route in this slice).
//!
//! `reason` is unconstrained free text end to end — same rationale as
//! `notification::Notification::notification_type` (see that doc comment):
//! Node's `chunkStaleness.reason` column is `text NOT NULL` with no check
//! constraint and no enum in `packages/db/src/schema/staleness.ts`, just a
//! comment listing example values. Confirmed writers in the live pipeline:
//! `age`, `requirement_uncovered`, `requirement_failing`,
//! `upstream_impact`, `diverged_duplicate`. `file_changed` has zero writers
//! anywhere (schema comment, dead seed script, dead frontend icon-mapping
//! only) and is deliberately not implemented here.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// One row of `GET /api/chunks/stale`
/// (`packages/db/src/repository/staleness.ts::getStaleFlags`): the flag
/// joined to its parent chunk's `title`/`type` for display. `camelCase`
/// serialisation matches every other wire type in this crate.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaleFlag {
    pub id: String,
    pub chunk_id: String,
    pub reason: String,
    pub detail: Option<String>,
    pub related_chunk_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub detected_at: UtcTimestamp,
    pub chunk_title: String,
    pub chunk_type: String,
}

/// Filters shared by [`list`]. `None`/absent means "no filter" for each
/// field, matching Node's `params?.reason`/`params?.spaceId` optionality.
#[derive(Debug, Default)]
pub struct ListParams {
    pub reason: Option<String>,
    pub space_id: Option<String>,
    /// `None` defaults to 50 inside [`list`], matching Node's
    /// `params?.limit ?? 50`.
    pub limit: Option<i64>,
}

/// Appends the `space_id` branch shared by every query in this module:
/// a chunk explicitly in the named space, OR a chunk with no space
/// assignment at all (global chunks pass every space filter) — identical
/// semantics to `chunk::push_filters`'s `space_id` branch, reproduced here
/// against the bare `chunk` alias since these queries aren't chunk-list
/// queries themselves.
fn push_space_condition<'a>(
    qb: &mut sqlx::QueryBuilder<'a, sqlx::Postgres>,
    chunk_id_col: &str,
    space_id: &'a str,
) {
    qb.push(" AND (");
    qb.push(chunk_id_col);
    qb.push(" IN (SELECT chunk_id FROM chunk_space WHERE space_id = ");
    qb.push_bind(space_id);
    qb.push(") OR ");
    qb.push(chunk_id_col);
    qb.push(" NOT IN (SELECT chunk_id FROM chunk_space))");
}

/// Lists a user's undismissed, unsuppressed staleness flags.
///
/// A row is excluded the moment *either* `dismissed_at` or `suppress_pair`
/// is set — see the module doc on dismiss vs. suppress for why those are
/// two different mechanisms that both have to gate reads here.
///
/// `ORDER BY detected_at DESC, id ASC`: Node's `getStaleFlags` only orders
/// by `desc(chunkStaleness.detectedAt)` with no tiebreaker at all. Scans
/// (`detect_age_stale_chunks`, `detect_uncovered_chunks`) routinely insert
/// a batch of flags with the exact same `detected_at`, making ties likely
/// — same bug class as `chunk::list`/`notification::list`/`activity::list`
/// (see their equivalent comments), so this port adds `id ASC` as a total
/// order the way every other list query in this crate already does.
pub async fn list(pool: &PgPool, user_id: &str, params: ListParams) -> AppResult<Vec<StaleFlag>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT cs.id, cs.chunk_id, cs.reason, cs.detail, cs.related_chunk_id, cs.detected_at, \
         c.title AS chunk_title, c.type AS chunk_type \
         FROM chunk_staleness cs JOIN chunk c ON c.id = cs.chunk_id \
         WHERE c.user_id = ",
    );
    qb.push_bind(user_id);
    qb.push(" AND cs.dismissed_at IS NULL AND cs.suppress_pair IS NULL");

    if let Some(reason) = &params.reason {
        qb.push(" AND cs.reason = ").push_bind(reason);
    }
    if let Some(space_id) = &params.space_id {
        push_space_condition(&mut qb, "c.id", space_id);
    }

    qb.push(" ORDER BY cs.detected_at DESC, cs.id ASC LIMIT ");
    qb.push_bind(params.limit.unwrap_or(50));

    let rows = qb.build_query_as::<StaleFlag>().fetch_all(pool).await?;
    Ok(rows)
}

/// Whether `chunk_id` currently carries any undismissed, unsuppressed
/// staleness flag. Backs `context::service::enrich_chunks`'s `is_stale`
/// flag — Node's `enrichChunks` fetches the full row set via
/// `getStaleFlagsForChunk(id)` (`packages/db/src/repository/staleness.ts:
/// 61-74`) and only ever reads `staleFlags.length > 0`
/// (`packages/api/src/context/resolvers.ts:64`), so an existence check
/// reproduces the observable behaviour without materialising rows nothing
/// downstream reads — the same "just the boolean" shape
/// `chunk_meta::file_ref_path_exists` uses for an analogous Node call site.
///
/// Scoped through the chunk's owner in SQL — the same "through the parent"
/// hardening as `tag::tags_for_chunk` — even though every caller today
/// only reaches this after already loading the chunk under `user_id`
/// itself (`chunk::find_by_id`), so a foreign `chunk_id` is defence in
/// depth here, not the primary rejection path.
pub async fn chunk_is_stale(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<bool> {
    let hit = sqlx::query_scalar!(
        r#"SELECT 1 AS "exists!" FROM chunk_staleness cs
           WHERE cs.chunk_id = $1
             AND cs.dismissed_at IS NULL AND cs.suppress_pair IS NULL
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)
           LIMIT 1"#,
        chunk_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(hit.is_some())
}

/// Counts a user's undismissed, unsuppressed staleness flags. Same filters
/// as [`list`] minus `reason`/`limit` — Node's `getStaleCount` only takes
/// `spaceId` (`packages/api/src/staleness/routes.ts`'s `/chunks/stale/count`
/// query schema has no `reason` key).
pub async fn count(pool: &PgPool, user_id: &str, space_id: Option<&str>) -> AppResult<i64> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM chunk_staleness cs JOIN chunk c ON c.id = cs.chunk_id \
         WHERE c.user_id = ",
    );
    qb.push_bind(user_id);
    qb.push(" AND cs.dismissed_at IS NULL AND cs.suppress_pair IS NULL");
    if let Some(space_id) = space_id {
        push_space_condition(&mut qb, "c.id", space_id);
    }
    let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
    Ok(count)
}

/// The visible surface of a raw `pg` driver `QueryResult`, which is exactly
/// what Node's `dismissStaleFlag`/`suppressDuplicatePair` leak to the HTTP
/// response: neither calls `.returning()`, so drizzle's node-postgres
/// session (`node_modules/drizzle-orm/node-postgres/session.js`, the
/// `!fields && !customResultMapper` branch) resolves the query builder's
/// promise with `client.query(...)`'s return value unmodified — the raw
/// `pg.Result` instance — and Elysia serialises whatever that Effect
/// resolves to.
///
/// Verified against a disposable database (not the real `pg`/`pg-types`
/// version pinned in `packages/db`, but the same `pg@8.20.0` this
/// workspace's `pnpm-lock.yaml` resolves, run directly against
/// `fubbik-rs-db`): `JSON.stringify(result)` for an `UPDATE ... ` with no
/// matching `RETURNING` includes `command`, `rowCount`, `oid`, `rows`,
/// `fields` — the five fields `pg`'s README documents as the `Result`
/// API — plus `_types`, `RowCtor`, `rowAsArray`, `_prebuiltEmptyResultObject`
/// (`_parsers` is present as an own property but serialises to nothing:
/// its value is `undefined`). Those four extra keys are underscore- or
/// otherwise internal-prefixed implementation details of the installed
/// `pg`/`pg-types` version — `_types` in particular embeds that version's
/// entire OID-to-typename registry (60+ entries) as a live object
/// reference, not stable response data. Hardcoding that blob here would
/// give zero behavioural value and would silently drift out of sync with
/// whatever `pg` version Node is actually running. This port reproduces
/// only the five documented, stable fields and omits the rest — a
/// deliberate divergence from a byte-for-byte leak, not a guess.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RawUpdateResult {
    pub command: String,
    pub row_count: i64,
    pub oid: Option<i64>,
    pub rows: Vec<serde_json::Value>,
    pub fields: Vec<serde_json::Value>,
}

impl RawUpdateResult {
    fn update(rows_affected: u64) -> Self {
        Self {
            command: "UPDATE".to_string(),
            row_count: rows_affected as i64,
            oid: None,
            rows: Vec::new(),
            fields: Vec::new(),
        }
    }
}

/// Dismisses a single flag by id, stamping `dismissed_at`/`dismissed_by`.
///
/// **Divergence #14**: Node's `dismissStaleFlag(flagId, userId)`
/// (`packages/db/src/repository/staleness.ts:91-95`) updates by `id` alone
/// and only ever *stamps* `userId` into `dismissedBy` — it never checks
/// that the flag's chunk belongs to that user, so any authenticated caller
/// can dismiss any flag by id. This port closes that gap by folding an
/// ownership check into the `WHERE` clause via the flag's parent chunk, in
/// one atomic statement (not a separate check-then-update, which would be
/// TOCTOU-able). Returns `None` — not an error — both when the flag id
/// doesn't exist at all *and* when it exists but belongs to another user's
/// chunk; the two cases are deliberately not distinguished (doing so would
/// leak whether a given flag id exists to a caller who doesn't own it).
/// The service layer turns `None` into 404, matching the
/// `favorite`/`notification` precedent of "guard lives in SQL, not the
/// caller" (see `chunk::reset`'s doc comment for the same shape of
/// defense-in-depth this crate already established for `space::reset`).
pub async fn dismiss(
    pool: &PgPool,
    user_id: &str,
    flag_id: &str,
) -> AppResult<Option<RawUpdateResult>> {
    let res = sqlx::query!(
        r#"UPDATE chunk_staleness cs
           SET dismissed_at = now(), dismissed_by = $2
           WHERE cs.id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = cs.chunk_id AND c.user_id = $2)"#,
        flag_id,
        user_id
    )
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(RawUpdateResult::update(res.rows_affected())))
}

/// Suppresses a duplicate pair: sets `suppress_pair` to the sorted
/// `"idA:idB"` key on every undismissed `diverged_duplicate` row whose
/// `chunk_id` or `related_chunk_id` is either member of the pair — both
/// directions, since a divergence between A and B is typically flagged
/// twice (once from each side).
///
/// Node's `suppressDuplicatePair(chunkIdA, chunkIdB)`
/// (`packages/db/src/repository/staleness.ts:97-111`) takes **no `userId`
/// at all** — it's a global write with zero ownership check, reachable by
/// any authenticated caller for any two chunk ids. This port adds a guard
/// Node lacks (same class of addition as `space::reset`'s `EXISTS` check
/// and `activity::list`'s `space_id` guard, both documented as defense in
/// depth Node doesn't have): both `chunk_id_a` and `chunk_id_b` must exist
/// and belong to `user_id`, checked up front in one query. Returns `None`
/// when either chunk isn't owned by the caller — the service layer maps
/// that to 404, mirroring the "404 before insert" precedent in
/// `favorites::service::add`. When the guard passes, the actual
/// `UPDATE ... SET suppress_pair` statement is otherwise identical to
/// Node's — still unfiltered by chunk ownership in its own `WHERE`, since
/// by this point both pair members are already known to be the caller's.
pub async fn suppress_duplicate(
    pool: &PgPool,
    user_id: &str,
    chunk_id_a: &str,
    chunk_id_b: &str,
) -> AppResult<Option<RawUpdateResult>> {
    let owns_both = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM chunk WHERE id = $1 AND user_id = $3)
             AND EXISTS(SELECT 1 FROM chunk WHERE id = $2 AND user_id = $3) AS "owns_both!""#,
        chunk_id_a,
        chunk_id_b,
        user_id
    )
    .fetch_one(pool)
    .await?;

    if !owns_both {
        return Ok(None);
    }

    let mut pair = [chunk_id_a, chunk_id_b];
    pair.sort_unstable();
    let pair_key = format!("{}:{}", pair[0], pair[1]);

    let res = sqlx::query!(
        r#"UPDATE chunk_staleness
           SET suppress_pair = $1
           WHERE reason = 'diverged_duplicate'
             AND dismissed_at IS NULL
             AND (chunk_id IN ($2, $3) OR related_chunk_id IN ($2, $3))"#,
        pair_key,
        chunk_id_a,
        chunk_id_b
    )
    .execute(pool)
    .await?;

    Ok(Some(RawUpdateResult::update(res.rows_affected())))
}

/// Flags chunks not updated in `threshold_days` days (default 90 — Node's
/// `detectAgeStaleChunks`'s `thresholdDays = 90` default parameter) with
/// reason `"age"`. Returns the number of *newly* flagged chunks.
///
/// Idempotency is a pre-filter, not `ON CONFLICT`: chunks already carrying
/// an undismissed `"age"` flag are excluded from the candidate set up
/// front (`chunk_staleness` has no unique constraint an `ON CONFLICT`
/// clause could target), matching Node's own `alreadyFlagged` subquery
/// exclusion exactly.
pub async fn detect_age_stale_chunks(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    threshold_days: i64,
) -> AppResult<i64> {
    let mut qb = sqlx::QueryBuilder::new("SELECT id, updated_at FROM chunk WHERE user_id = ");
    qb.push_bind(user_id);
    qb.push(" AND updated_at < now() - (");
    qb.push_bind(threshold_days);
    qb.push(" * interval '1 day') AND archived_at IS NULL");
    qb.push(
        " AND id NOT IN (SELECT chunk_id FROM chunk_staleness \
           WHERE reason = 'age' AND dismissed_at IS NULL)",
    );
    if let Some(space_id) = space_id {
        push_space_condition(&mut qb, "id", space_id);
    }

    let rows: Vec<(String, UtcTimestamp)> = qb.build_query_as().fetch_all(pool).await?;
    if rows.is_empty() {
        return Ok(0);
    }

    let mut insert =
        sqlx::QueryBuilder::new("INSERT INTO chunk_staleness (id, chunk_id, reason, detail) ");
    insert.push_values(rows.iter(), |mut b, (chunk_id, updated_at)| {
        b.push_bind(crate::new_id())
            .push_bind(chunk_id.clone())
            .push_bind("age")
            .push_bind(format!("Last updated {}", updated_at.0.date()));
    });
    insert.build().execute(pool).await?;
    Ok(rows.len() as i64)
}

/// Flags chunks not updated in `threshold_days` days (default 30 — Node's
/// `detectUncoveredChunks`'s `thresholdDays = 30` default, **not** the
/// same default as [`detect_age_stale_chunks`]'s 90) that have no row in
/// `requirement_chunk`, with reason `"requirement_uncovered"`.
///
/// As of the `requirements` domain port (`repo::requirement::set_chunks`),
/// `requirement_chunk` has a real write API — this query now genuinely
/// excludes chunks that carry at least one requirement link, matching
/// Node's query shape exactly. Before that port landed, this table was
/// always empty in this workspace and the `NOT IN` clause was vacuously
/// true for every row (every eligible chunk got flagged); that history is
/// noted here only because the previous version of this comment described
/// that now-stale state as expected.
pub async fn detect_uncovered_chunks(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    threshold_days: i64,
) -> AppResult<i64> {
    let mut qb = sqlx::QueryBuilder::new("SELECT id FROM chunk WHERE user_id = ");
    qb.push_bind(user_id);
    qb.push(" AND updated_at < now() - (");
    qb.push_bind(threshold_days);
    qb.push(" * interval '1 day') AND archived_at IS NULL");
    qb.push(
        " AND id NOT IN (SELECT chunk_id FROM chunk_staleness \
           WHERE reason = 'requirement_uncovered' AND dismissed_at IS NULL)",
    );
    qb.push(" AND id NOT IN (SELECT chunk_id FROM requirement_chunk)");
    if let Some(space_id) = space_id {
        push_space_condition(&mut qb, "id", space_id);
    }

    let ids: Vec<String> = qb.build_query_scalar().fetch_all(pool).await?;
    if ids.is_empty() {
        return Ok(0);
    }

    let mut insert =
        sqlx::QueryBuilder::new("INSERT INTO chunk_staleness (id, chunk_id, reason, detail) ");
    insert.push_values(ids.iter(), |mut b, chunk_id| {
        b.push_bind(crate::new_id())
            .push_bind(chunk_id.clone())
            .push_bind("requirement_uncovered")
            .push_bind("No requirements linked — consider adding requirement coverage");
    });
    insert.build().execute(pool).await?;
    Ok(ids.len() as i64)
}

/// Flags every chunk `age::compute_impact_ripple(chunk_id)` finds
/// downstream of `chunk_id` with reason `"upstream_impact"`, backing `POST
/// /api/chunks/{id}/scan-impact`. Port of Node's `flagImpactRipple`
/// (`packages/api/src/staleness/detect-impact.ts:5-32`).
///
/// Returns `Ok(None)` when `chunk_id` isn't owned by `user_id` — this
/// port's own guard, the same "ownership check this route lacks in Node"
/// pattern as divergence #14 (`dismiss`) and #15 (`suppress_duplicate`)
/// above: Node's route (`packages/api/src/staleness/routes.ts:93-105`)
/// calls `flagImpactRipple(ctx.params.id, ...)` with **no ownership check
/// at all** on the chunk id — any authenticated caller can trigger a scan
/// (and write flags) against any chunk. The service layer maps `None` to
/// 404.
///
/// **Divergence #19: ripple targets are scoped to the caller.** Graph edges
/// cross ownership — `age::compute_impact_ripple` walks `:connects` edges
/// regardless of who owns each endpoint, and Node's `flagImpactRipple`
/// (`packages/api/src/staleness/detect-impact.ts:5-32`) writes a flag onto
/// **every** chunk it reaches this way, including chunks owned by other
/// users. Node's own idempotency pre-filter then makes this worse: its
/// `alreadyFlagged` check goes through `getStaleFlags(userId, {reason:
/// "upstream_impact"})`, which joins through `chunk.user_id = userId` — the
/// *caller's* id, not the impacted chunk's owner — so a cross-user target
/// is invisible to that pre-filter and gets re-flagged, unbounded, on every
/// re-run.
///
/// **The human partner decided to scope ripple targets to the caller
/// instead of reproducing this bug-for-bug.** The graph traversal itself is
/// unchanged (`age::compute_impact_ripple` still walks through other
/// users' chunks to reach further targets); only the *write* is now
/// restricted to targets `user_id` owns. This closes the leak (no flag
/// lands on a chunk the caller doesn't own) and, for free, fixes the
/// duplicate-accumulation bug too: once every target is guaranteed to be
/// the caller's own, the existing `already_flagged` pre-filter (still
/// scoped to `user_id`, unchanged below) covers all of them.
///
/// Detail text is simplified from Node's `Impacted by change to "<title>"
/// (degree: N.NN, H hops via a → b)`: `age::compute_impact_ripple` returns
/// only the surviving chunk ids (see that function's doc comment for why),
/// so the degree/hop/path breakdown isn't available here to format.
pub async fn flag_impact_ripple(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    chunk_title: &str,
) -> AppResult<Option<i64>> {
    let owns: bool = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM chunk WHERE id = $1 AND user_id = $2) AS "owns!""#,
        chunk_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    if !owns {
        return Ok(None);
    }

    let ripple = crate::age::compute_impact_ripple(pool, chunk_id).await?;
    if ripple.is_empty() {
        return Ok(Some(0));
    }

    // Second, distinct guard from the one above: restricts WRITE targets to
    // chunks `user_id` owns. The primary `chunk_id` ownership guard already
    // proved the caller owns the chunk the ripple originates FROM; this
    // proves it for every chunk the ripple would flag. Dropping this filter
    // reopens divergence #19 — cross-user targets get flagged again, and
    // the pre-filter below stops deduplicating them (see
    // `tests/staleness.rs` for the load-bearing proof).
    let targets: Vec<String> = sqlx::query_scalar!(
        r#"SELECT id FROM chunk WHERE user_id = $1 AND id = ANY($2::text[])"#,
        user_id,
        &ripple
    )
    .fetch_all(pool)
    .await?;
    if targets.is_empty() {
        return Ok(Some(0));
    }

    let already_flagged: std::collections::HashSet<String> = sqlx::query_scalar!(
        r#"SELECT cs.chunk_id FROM chunk_staleness cs
           JOIN chunk c ON c.id = cs.chunk_id
           WHERE c.user_id = $1
             AND cs.reason = 'upstream_impact'
             AND cs.dismissed_at IS NULL
             AND cs.related_chunk_id = $2"#,
        user_id,
        chunk_id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let detail = format!("Impacted by change to \"{chunk_title}\"");
    let mut flagged = 0i64;
    for target in targets {
        if already_flagged.contains(&target) {
            continue;
        }
        sqlx::query!(
            "INSERT INTO chunk_staleness (id, chunk_id, reason, detail, related_chunk_id) \
             VALUES ($1, $2, 'upstream_impact', $3, $4)",
            crate::new_id(),
            target,
            detail,
            chunk_id
        )
        .execute(pool)
        .await?;
        flagged += 1;
    }

    Ok(Some(flagged))
}

/// Flags every chunk in `chunk_ids` linked to a failing requirement, with
/// reason `"requirement_failing"`. Port of Node's `flagRequirementFailing`
/// (`packages/db/src/repository/staleness.ts:231-266`), called from
/// `fubbik_api::requirements::service` whenever a requirement's status is
/// set to `"failing"` (both the dedicated status route and `updateRequirement`
/// when it happens to carry `status: "failing"`, though no route actually
/// exposes that second path — see `requirement::RequirementPatch`'s doc
/// comment).
///
/// Node takes no `user_id` at all — it flags whatever chunk ids the caller
/// hands it unconditionally. This port adds a `user_id` guard on the
/// INSERT (a chunk id not owned by `user_id` is silently skipped, same
/// "second distinct guard restricting WRITE targets" shape
/// `flag_impact_ripple` documents above), consistent with this crate's
/// "scope by `user_id` in SQL, never in the caller" convention. In
/// practice this guard is never load-bearing through this port's own call
/// site: `chunk_ids` always comes from `requirement::get_chunks`, which is
/// itself scoped to `user_id` via `requirement_chunk`'s parent-ownership
/// join — but the guard still holds if that assumption ever changes.
///
/// Idempotent per `(chunk_id, detail)`: a chunk already carrying an
/// undismissed flag with this exact requirement's detail string is not
/// flagged again, matching Node's `alreadyFlagged` pre-filter exactly.
/// Returns the number of chunks actually flagged — Node's equivalent
/// return value (`{flagged: toFlag.length}`) is discarded by every caller
/// in this port (and in Node), so the two can differ under the added
/// ownership guard without affecting observable behaviour.
pub async fn flag_requirement_failing(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
    requirement_title: &str,
    chunk_ids: &[String],
) -> AppResult<i64> {
    if chunk_ids.is_empty() {
        return Ok(0);
    }

    let detail = format!("Requirement \"{requirement_title}\" ({requirement_id}) is failing");

    let already_flagged: std::collections::HashSet<String> = sqlx::query_scalar!(
        r#"SELECT chunk_id FROM chunk_staleness
           WHERE reason = 'requirement_failing'
             AND detail = $1
             AND dismissed_at IS NULL
             AND chunk_id = ANY($2)"#,
        detail,
        chunk_ids
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let mut flagged = 0i64;
    for chunk_id in chunk_ids {
        if already_flagged.contains(chunk_id) {
            continue;
        }
        let res = sqlx::query!(
            "INSERT INTO chunk_staleness (id, chunk_id, reason, detail)
             SELECT $1, $2, 'requirement_failing', $3
             WHERE EXISTS (SELECT 1 FROM chunk c WHERE c.id = $2 AND c.user_id = $4)",
            crate::new_id(),
            chunk_id,
            detail,
            user_id
        )
        .execute(pool)
        .await?;
        flagged += res.rows_affected() as i64;
    }

    Ok(flagged)
}
