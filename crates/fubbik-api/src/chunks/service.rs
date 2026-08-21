use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{self, Chunk, ChunkPatch, ListParams, NewChunk};
use sqlx::PgPool;

use super::dto::{ChunkListResponse, CreateChunkBody, UpdateChunkBody};

pub async fn list(
    pool: &PgPool,
    user_id: &str,
    params: ListParams,
) -> AppResult<ChunkListResponse> {
    let chunks = chunk::list(pool, user_id, &params).await?;
    let total = chunk::count(pool, user_id, &params).await?;
    Ok(ChunkListResponse {
        chunks,
        total,
        limit: params.limit,
        offset: params.offset,
    })
}

/// Node derives `reviewStatus` from `origin` rather than accepting it on
/// create: an `ai`-authored chunk starts as a `draft` needing review,
/// anything else starts `approved` (`chunk-mutations.ts:75,86`). Kept as one
/// function so the pairing cannot drift apart.
fn review_status_for_origin(origin: &str) -> &'static str {
    if origin == "ai" { "draft" } else { "approved" }
}

// ---------------------------------------------------------------------------
// Route-schema limits
// ---------------------------------------------------------------------------
//
// Node enforces these in its Elysia `t.Object` before the handler runs, so
// there is no service-layer equivalent to port — they have to be
// re-expressed here or they simply vanish. Growing the request bodies to
// Node's full field set without also porting its constraints would trade
// one silent-acceptance bug (fields dropped) for another (values Node would
// have rejected being stored).
//
// The two literal unions stay `String` at the DTO and DB layers and are
// checked here, not modelled as serde enums: both columns are free `text`
// with no CHECK, older rows may hold anything, and a serde enum would
// reject with a parse error instead of a message naming the field. This is
// the same disposition `FileRefEntry::relation` carries.

const ORIGINS: [&str; 2] = ["human", "ai"];
const REVIEW_STATUSES: [&str; 3] = ["draft", "reviewed", "approved"];

fn check_len(value: &str, max: usize, field: &str) -> AppResult<()> {
    if value.chars().count() > max {
        return Err(AppError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(())
}

fn check_one_of(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if !allowed.contains(&value) {
        return Err(AppError::Validation(format!(
            "{field} must be one of {}",
            allowed.join(", ")
        )));
    }
    Ok(())
}

/// `tags` (`maxItems: 20`, each `maxLength: 50`) and `spaceIds`
/// (`maxItems: 20`) carry the same limits on both the create and update
/// bodies, so they are checked in one place.
fn check_tags_and_spaces(tags: Option<&[String]>, space_ids: Option<&[String]>) -> AppResult<()> {
    if let Some(tags) = tags {
        if tags.len() > 20 {
            return Err(AppError::Validation("at most 20 tags are allowed".into()));
        }
        for tag in tags {
            check_len(tag, 50, "tag")?;
        }
    }
    if let Some(ids) = space_ids
        && ids.len() > 20
    {
        return Err(AppError::Validation(
            "at most 20 spaceIds are allowed".into(),
        ));
    }
    Ok(())
}

pub async fn create(pool: &PgPool, user_id: &str, body: CreateChunkBody) -> AppResult<Chunk> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    check_len(title, 200, "title")?;
    check_len(&body.content, 50_000, "content")?;
    if let Some(t) = body.chunk_type.as_deref() {
        check_len(t, 20, "type")?;
    }
    if let Some(r) = body.rationale.as_deref() {
        check_len(r, 5_000, "rationale")?;
    }
    if let Some(c) = body.consequences.as_deref() {
        check_len(c, 5_000, "consequences")?;
    }
    if let Some(o) = body.origin.as_deref() {
        check_one_of(o, &ORIGINS, "origin")?;
    }
    if let Some(t) = body.update_tag.as_deref() {
        check_len(t, 100, "updateTag")?;
    }
    check_tags_and_spaces(body.tags.as_deref(), body.space_ids.as_deref())?;

    // `documentId` is verified to exist AND to belong to the caller before
    // the insert, matching Node's `resolveDocumentLinkageForNewChunk`
    // (`chunk-mutations.ts:32-57`) — including its detail that a blank or
    // whitespace-only value is treated as absent rather than as an error,
    // and that an unknown/foreign id is a 400, not a silent null.
    let document_id = match body.document_id.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(id) => {
            let owned = fubbik_db::repo::document::find_by_id(pool, id, user_id).await?;
            if owned.is_none() {
                return Err(AppError::Validation(
                    "Invalid or unknown document for documentId".into(),
                ));
            }
            Some(id.to_string())
        }
    };
    // Node only carries `documentOrder` through when a document was
    // actually resolved (`chunk-mutations.ts:52-55` returns `undefined` for
    // both in the no-document branch), so an order without a document is
    // dropped rather than stored dangling.
    let document_order = document_id.as_ref().and(body.document_order);

    let origin = body.origin.unwrap_or_else(|| "human".into());
    let review_status = review_status_for_origin(&origin).to_string();

    let created = chunk::create(
        pool,
        user_id,
        NewChunk {
            title: title.to_string(),
            content: body.content,
            chunk_type: body.chunk_type.unwrap_or_else(|| "note".into()),
            rationale: body.rationale,
            alternatives: body.alternatives,
            consequences: body.consequences,
            origin,
            review_status,
            document_id,
            document_order,
        },
    )
    .await?;

    // `scope` is accepted by the route body and then DROPPED — on both
    // stacks. Node's route schema declares it, but `createChunk`'s own
    // parameter type omits it and `createChunkRepo` never passes it
    // through (`chunk-mutations.ts:60-90`), so a scope supplied at create
    // time never reaches the column.
    //
    // Reproduced rather than fixed, after measuring: no first-party client
    // sends `scope` on create (the web's `chunks.new` page doesn't; the MCP
    // server's `scope` field is on `propose_chunk_update`, a different
    // route), so "fixing" it would buy nothing real while making the two
    // stacks disagree — the same call the ledger already recorded for blank
    // query params. Pinned by
    // `chunk_write.rs::create_accepts_scope_and_drops_it_like_node` so the
    // behaviour is a documented decision rather than an oversight. Setting
    // a scope is `PATCH`'s job, where it does land.
    let _ = &body.scope;

    apply_tags_and_spaces(pool, user_id, &created.id, body.tags, body.space_ids).await?;

    if let Some(tag) = body.update_tag.as_deref() {
        // Node records the create under version 0 with empty title/content
        // (`chunk-mutations.ts:120-131`) — a marker row, not a snapshot.
        // Reproduced by snapshotting the freshly created chunk instead,
        // which carries the same `update_tag` but real content; an empty
        // marker in the history list is worse than useless to the UI, which
        // renders title and content per version.
        fubbik_db::repo::chunk_version::snapshot(pool, &created, Some(tag)).await?;
    }

    Ok(created)
}

/// Writes the two join tables a chunk create/update may touch.
///
/// Both are all-or-nothing replacements, and both are skipped entirely when
/// the field is absent — `None` means "don't touch", `Some(vec![])` means
/// "clear". Node makes the same distinction, but only on update: its
/// `createChunk` guards with `body.tags && body.tags.length > 0`, so
/// creating with `tags: []` is a no-op there and here alike (there is
/// nothing to clear on a brand-new chunk).
///
/// Tags arrive as names and are resolved through `tag::find_or_create`,
/// sequentially rather than Node's `{ concurrency: 5 }` — these are
/// single-row upserts against one pool, and running them in order removes
/// any chance of two concurrent inserts racing for the same new name.
async fn apply_tags_and_spaces(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    tags: Option<Vec<String>>,
    space_ids: Option<Vec<String>>,
) -> AppResult<()> {
    if let Some(names) = tags {
        let mut tag_ids = Vec::with_capacity(names.len());
        for name in &names {
            tag_ids.push(
                fubbik_db::repo::tag::find_or_create(pool, user_id, name)
                    .await?
                    .id,
            );
        }
        fubbik_db::repo::tag::set_chunk_tags(pool, user_id, chunk_id, &tag_ids).await?;
    }
    if let Some(ids) = space_ids {
        fubbik_db::repo::space::set_chunk_spaces(pool, user_id, chunk_id, &ids).await?;
    }
    Ok(())
}

pub async fn get(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Chunk> {
    chunk::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))
}

pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateChunkBody,
) -> AppResult<Chunk> {
    // Mirrors `create`'s title validation (trim, reject blank, cap at 200
    // chars) so a PATCH cannot put a chunk in a state POST would refuse to
    // create in the first place. `None` means "leave title unchanged" and
    // is left untouched.
    let title = body
        .title
        .as_deref()
        .map(|title| {
            let trimmed = title.trim();
            if trimmed.is_empty() {
                return Err(AppError::Validation("title is required".into()));
            }
            check_len(trimmed, 200, "title")?;
            Ok(trimmed.to_string())
        })
        .transpose()?;

    // Same route-schema limits as `create`, plus the four fields only PATCH
    // accepts. All validated BEFORE the version snapshot is written — a
    // rejected PATCH must not leave a history entry behind for an edit that
    // never happened.
    if let Some(c) = body.content.as_deref() {
        check_len(c, 50_000, "content")?;
    }
    if let Some(t) = body.chunk_type.as_deref() {
        check_len(t, 20, "type")?;
    }
    if let Some(r) = body.rationale.as_deref() {
        check_len(r, 5_000, "rationale")?;
    }
    if let Some(c) = body.consequences.as_deref() {
        check_len(c, 5_000, "consequences")?;
    }
    // `Some(None)` is an explicit null (clear the summary) and has nothing
    // to length-check; only `Some(Some(_))` carries a value.
    if let Some(Some(summary)) = body.summary.as_ref() {
        check_len(summary, 500, "summary")?;
    }
    if let Some(aliases) = body.aliases.as_deref() {
        if aliases.len() > 20 {
            return Err(AppError::Validation(
                "at most 20 aliases are allowed".into(),
            ));
        }
        for alias in aliases {
            check_len(alias, 100, "alias")?;
        }
    }
    if let Some(entries) = body.not_about.as_deref() {
        if entries.len() > 20 {
            return Err(AppError::Validation(
                "at most 20 notAbout entries are allowed".into(),
            ));
        }
        for entry in entries {
            check_len(entry, 100, "notAbout entry")?;
        }
    }
    if let Some(o) = body.origin.as_deref() {
        check_one_of(o, &ORIGINS, "origin")?;
    }
    if let Some(rs) = body.review_status.as_deref() {
        check_one_of(rs, &REVIEW_STATUSES, "reviewStatus")?;
    }
    if let Some(t) = body.update_tag.as_deref() {
        check_len(t, 100, "updateTag")?;
    }
    check_tags_and_spaces(body.tags.as_deref(), body.space_ids.as_deref())?;

    let current = get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::snapshot(pool, &current, body.update_tag.as_deref()).await?;

    // Node stamps the reviewer whenever `reviewStatus` is present, even if
    // the value is unchanged (`chunk-mutations.ts:175-178`) — the stamp
    // records "who last asserted this status", not "who changed it".
    let (reviewed_by, reviewed_at) = match body.review_status {
        Some(_) => (
            Some(user_id.to_string()),
            Some(chrono::Utc::now().naive_utc()),
        ),
        None => (None, None),
    };

    let updated = chunk::update(
        pool,
        user_id,
        id,
        ChunkPatch {
            title,
            content: body.content,
            chunk_type: body.chunk_type,
            rationale: body.rationale,
            consequences: body.consequences,
            alternatives: body.alternatives,
            scope: body.scope,
            summary: body.summary,
            aliases: body.aliases,
            not_about: body.not_about,
            origin: body.origin,
            review_status: body.review_status,
            reviewed_by,
            reviewed_at,
            is_entry_point: body.is_entry_point,
            // Not on the PATCH body — Node's route schema has no
            // `documentOrder` either, though its repo params do. Reordering
            // a chunk within a document is the documents domain's job.
            document_order: None,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("chunk".into()))?;

    apply_tags_and_spaces(pool, user_id, id, body.tags, body.space_ids).await?;

    Ok(updated)
}

pub async fn history(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<Vec<fubbik_db::repo::chunk_version::ChunkVersion>> {
    // Fetch the chunk first so another user's history cannot be read.
    get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::list_for_chunk(pool, id, user_id).await
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if chunk::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("chunk".into()))
    }
}

// ---------------------------------------------------------------------------
// applies-to / file-refs body validation
// ---------------------------------------------------------------------------

/// Node caps both sub-resource bodies at 50 entries
/// (`t.Array(..., { maxItems: 50 })`).
const MAX_SUB_RESOURCE_ENTRIES: usize = 50;

/// The four values Node's `PUT /chunks/:id/file-refs` route schema accepts
/// for `relation` (`packages/api/src/file-refs/routes.ts:23`). Enforced
/// here rather than as a serde enum or a DB CHECK — the column is free
/// `text` and Node's *read* paths never constrain it, so an older row
/// carrying something else must still be readable.
const FILE_REF_RELATIONS: [&str; 4] = ["documents", "configures", "tests", "implements"];

/// Validates and converts `PUT /chunks/{id}/applies-to`'s body.
///
/// Reproduces Node's Elysia schema limits, which are enforced before the
/// handler runs there and therefore have no service-layer equivalent to
/// copy: `maxItems: 50`, `pattern` `maxLength: 500`, `note` `maxLength:
/// 500`. Lengths are counted in `chars()`, not bytes — Elysia's
/// `maxLength` counts UTF-16 code units, and `chars()` is the closer of
/// the two Rust options for any pattern a user would plausibly type.
pub fn validate_applies_to(
    entries: Vec<super::routes::AppliesToEntry>,
) -> AppResult<Vec<fubbik_db::repo::chunk_meta::AppliesToInput>> {
    if entries.len() > MAX_SUB_RESOURCE_ENTRIES {
        return Err(AppError::Validation(format!(
            "at most {MAX_SUB_RESOURCE_ENTRIES} applies-to entries are allowed"
        )));
    }
    entries
        .into_iter()
        .map(|entry| {
            if entry.pattern.chars().count() > 500 {
                return Err(AppError::Validation(
                    "pattern must be at most 500 characters".into(),
                ));
            }
            if entry.note.as_ref().is_some_and(|n| n.chars().count() > 500) {
                return Err(AppError::Validation(
                    "note must be at most 500 characters".into(),
                ));
            }
            Ok(fubbik_db::repo::chunk_meta::AppliesToInput {
                pattern: entry.pattern,
                note: entry.note,
            })
        })
        .collect()
}

/// Validates and converts `PUT /chunks/{id}/file-refs`'s body — same
/// rationale as [`validate_applies_to`]. Node's limits: `maxItems: 50`,
/// `path` `maxLength: 1000`, `anchor` `maxLength: 500`, and `relation`
/// constrained to [`FILE_REF_RELATIONS`].
pub fn validate_file_refs(
    entries: Vec<super::routes::FileRefEntry>,
) -> AppResult<Vec<fubbik_db::repo::chunk_meta::FileRefInput>> {
    if entries.len() > MAX_SUB_RESOURCE_ENTRIES {
        return Err(AppError::Validation(format!(
            "at most {MAX_SUB_RESOURCE_ENTRIES} file-ref entries are allowed"
        )));
    }
    entries
        .into_iter()
        .map(|entry| {
            if entry.path.chars().count() > 1000 {
                return Err(AppError::Validation(
                    "path must be at most 1000 characters".into(),
                ));
            }
            if entry
                .anchor
                .as_ref()
                .is_some_and(|a| a.chars().count() > 500)
            {
                return Err(AppError::Validation(
                    "anchor must be at most 500 characters".into(),
                ));
            }
            if !FILE_REF_RELATIONS.contains(&entry.relation.as_str()) {
                return Err(AppError::Validation(format!(
                    "relation must be one of {}",
                    FILE_REF_RELATIONS.join(", ")
                )));
            }
            Ok(fubbik_db::repo::chunk_meta::FileRefInput {
                path: entry.path,
                anchor: entry.anchor,
                relation: entry.relation,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// chunk detail
// ---------------------------------------------------------------------------

/// Port of Node's `getChunkDetail` (`packages/api/src/chunks/service.ts:
/// 128-186`): the chunk plus its connections, spaces, applies-to patterns,
/// file references, tags, linked requirements and feature deltas, with a
/// health score computed over the lot and active feature overlays applied.
///
/// Node issues the eight queries concurrently via `Effect.all`; these run
/// sequentially. Every one is a single indexed lookup by `chunk_id` against
/// the same pool, so the difference is a handful of round trips, not a
/// change in what is read — and sequencing keeps the ownership 404 from
/// `get` strictly first, before any other query is even issued.
///
/// **Divergence, deliberate:** Node loads active feature ids in a global
/// `.resolve()` wrapped in `try/catch` that swallows failures and proceeds
/// with `activeFeatureIds = []` (`packages/api/src/index.ts:207-218`), so a
/// database hiccup there silently serves the *unoverlaid* chunk — the user
/// sees different content with no indication anything went wrong. Here the
/// caller passes the ids in and any failure loading them propagates as a
/// 500. Failing loudly is the right trade for a field that changes what
/// text the user reads.
pub async fn get_detail(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    active_feature_ids: &[String],
) -> AppResult<super::dto::ChunkDetail> {
    use fubbik_db::repo::{chunk_meta, connection, feature, requirement, space, tag};

    // First, and on its own: a chunk the caller does not own must 404 here
    // rather than after seven more queries have run against its id.
    let chunk = get(pool, user_id, id).await?;

    let connections = connection::connections_for_chunk(pool, id, user_id).await?;
    let spaces = space::spaces_for_chunk(pool, user_id, id).await?;
    let applies_to = chunk_meta::get_applies_to(pool, id, user_id).await?;
    let file_references = chunk_meta::get_file_refs(pool, id, user_id).await?;
    let tags = tag::tags_for_chunk(pool, user_id, id).await?;
    let chunk_ids = [id.to_string()];
    let requirements = requirement::requirements_for_chunks(pool, &chunk_ids, user_id).await?;
    let all_deltas = feature::deltas_for_chunk(pool, id, user_id).await?;

    // Node re-filters `requirements` on `r.chunkId === chunkId` before
    // counting (`service.ts:145`). That filter is a no-op — the query asked
    // for exactly this one chunk — so it is not reproduced; the count is
    // the row count.
    let requirement_count = requirements.len() as i64;
    let all_requirements_passing =
        requirement_count > 0 && requirements.iter().all(|r| r.status == "passing");

    let health_score =
        super::health_score::compute_health_score(&super::health_score::ChunkHealthInput {
            content: &chunk.content,
            summary: chunk.summary.as_deref(),
            rationale: chunk.rationale.as_deref(),
            alternatives: chunk.alternatives.as_ref().map(|a| a.0.as_slice()),
            consequences: chunk.consequences.as_deref(),
            // Counts the *rows*, duplicates included — a neighbour in three
            // spaces contributes three. See
            // `connection::connections_for_chunk`'s doc comment; Node feeds the
            // same inflated `connections.length` in here.
            connection_count: connections.len() as i64,
            // Both hardcoded in Node's call site too (`service.ts:157,163`).
            centrality_degree: 0,
            has_embedding: chunk.embedding.is_some(),
            requirement_count,
            all_requirements_passing,
            referenced_in_session: false,
        });

    let has_deltas = !all_deltas.is_empty();
    let mut applied_features: Vec<String> = Vec::new();

    // `serde_json::to_value` on a `Chunk` cannot fail: it has no non-string
    // map keys and no `f64` that could be NaN/inf, which are serde_json's
    // only two failure modes for a `Serialize` impl this crate controls.
    let mut resolved = serde_json::to_value(&chunk)
        .map_err(|e| AppError::Validation(format!("failed to serialise chunk for overlay: {e}")))?;

    if !active_feature_ids.is_empty() && has_deltas {
        // `deltas_for_chunk` already returns `ORDER BY priority ASC, id
        // ASC`, which is the order Node's `.sort((a, b) => a.featurePriority
        // - b.featurePriority)` produces (JS sort is stable, so ties keep
        // the repository's order). Highest priority is applied last and
        // therefore wins a same-field conflict — see `resolve` in
        // `packages/api/src/features/resolve.ts`.
        for delta in all_deltas
            .iter()
            .filter(|d| active_feature_ids.contains(&d.feature_id))
        {
            // A delta whose JSONB is not an object (`null`, an array, a
            // scalar) is skipped. Node's `{ ...resolvedChunk, ...d.delta }`
            // treats `null`/scalars the same way; for an *array* it would
            // splice in numeric keys, which no writer produces and no
            // reader could use. Not reproduced.
            if let (Some(target), Some(source)) =
                (resolved.as_object_mut(), delta.delta.0.as_object())
            {
                for (key, value) in source {
                    target.insert(key.clone(), value.clone());
                }
            }
            applied_features.push(delta.feature_id.clone());
        }
    }

    Ok(super::dto::ChunkDetail {
        chunk: resolved,
        connections,
        spaces,
        applies_to,
        file_references,
        tags,
        requirements,
        deltas: all_deltas.clone(),
        all_deltas,
        health_score,
        applied_features,
        has_deltas,
    })
}

// ---------------------------------------------------------------------------
// Lifecycle: archive, restore, bulk, merge
// ---------------------------------------------------------------------------

/// The seven actions `POST /api/chunks/bulk-update` accepts.
///
/// `set_codebase` keeps its pre-rename name on the wire — it sets the chunk's
/// *space*, and the `codebase → space` rename never reached this literal.
const BULK_ACTIONS: [&str; 7] = [
    "add_tags",
    "remove_tags",
    "set_type",
    "set_codebase",
    "set_review_status",
    "archive",
    "delete",
];

const REVIEW_STATUSES_BULK: [&str; 3] = ["draft", "reviewed", "approved"];

pub async fn archive(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    chunk::archive(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))?;
    Ok(())
}

pub async fn restore(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    chunk::restore(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))?;
    Ok(())
}

pub async fn list_archived(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<Chunk>> {
    chunk::list_archived(pool, user_id, space_id).await
}

/// Applies one action across up to 100 chunks.
///
/// Ownership is validated for **every** id before anything is written, so a
/// batch containing one foreign id changes nothing at all rather than
/// partially applying. Node does the same (`bulk-service.ts:29-35`) and fails
/// the whole call with an `AuthError`; this reports 404, matching the rest of
/// this crate's posture that an unowned id is indistinguishable from a
/// missing one.
pub async fn bulk_update(
    pool: &PgPool,
    user_id: &str,
    ids: Vec<String>,
    action: &str,
    value: Option<&str>,
) -> AppResult<u64> {
    check_one_of(action, &BULK_ACTIONS, "action")?;
    if ids.len() > 100 {
        return Err(AppError::Validation("at most 100 ids are allowed".into()));
    }
    if ids.is_empty() {
        return Ok(0);
    }

    for id in &ids {
        get(pool, user_id, id).await?;
    }

    // Node returns `{ updated: ids.length }` for the tag and space actions —
    // the count of chunks it *attempted*, not of rows changed — and the real
    // affected-row count for the three that go through a single UPDATE. That
    // inconsistency is preserved: the UI shows this number as "N chunks
    // updated", and for the tag actions every named chunk genuinely was
    // rewritten (its tag set replaced), even when the resulting set is
    // identical.
    let attempted = ids.len() as u64;

    match action {
        "add_tags" | "remove_tags" => {
            let names = parse_csv(value, action)?;
            let mut tag_ids = Vec::with_capacity(names.len());
            for name in &names {
                tag_ids.push(
                    fubbik_db::repo::tag::find_or_create(pool, user_id, name)
                        .await?
                        .id,
                );
            }
            for id in &ids {
                let existing: Vec<String> = fubbik_db::repo::tag::tags_for_chunk(pool, user_id, id)
                    .await?
                    .into_iter()
                    .map(|t| t.id)
                    .collect();
                let next: Vec<String> = if action == "add_tags" {
                    let mut merged = existing;
                    for t in &tag_ids {
                        if !merged.contains(t) {
                            merged.push(t.clone());
                        }
                    }
                    merged
                } else {
                    existing
                        .into_iter()
                        .filter(|t| !tag_ids.contains(t))
                        .collect()
                };
                fubbik_db::repo::tag::set_chunk_tags(pool, user_id, id, &next).await?;
            }
            Ok(attempted)
        }
        "set_type" => {
            let v = value
                .ok_or_else(|| AppError::Validation("value is required for set_type".into()))?;
            check_len(v, 20, "value")?;
            chunk::update_many(pool, user_id, &ids, Some(v), None).await
        }
        "set_review_status" => {
            let v = value.ok_or_else(|| {
                AppError::Validation("value must be draft, reviewed, or approved".into())
            })?;
            check_one_of(v, &REVIEW_STATUSES_BULK, "value")?;
            chunk::update_many(pool, user_id, &ids, None, Some(v)).await
        }
        "set_codebase" => {
            // A null/absent value clears the chunk's spaces, which is how the
            // UI's "no space" option is expressed.
            let space_ids: Vec<String> = value.map(|v| vec![v.to_string()]).unwrap_or_default();
            for id in &ids {
                fubbik_db::repo::space::set_chunk_spaces(pool, user_id, id, &space_ids).await?;
            }
            Ok(attempted)
        }
        "archive" => chunk::archive_many(pool, user_id, &ids).await,
        "delete" => chunk::delete_many(pool, user_id, &ids).await,
        // Unreachable: `check_one_of` above rejects anything else.
        other => Err(AppError::Validation(format!("Unknown action: {other}"))),
    }
}

/// Splits a comma-separated value, trimming and dropping blanks. An input
/// that reduces to nothing is an error rather than a silent no-op — Node
/// rejects a missing `value` for these two actions, and `","` is the same
/// thing arriving by a different route.
fn parse_csv(value: Option<&str>, action: &str) -> AppResult<Vec<String>> {
    let raw =
        value.ok_or_else(|| AppError::Validation(format!("value is required for {action}")))?;
    let names: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Err(AppError::Validation(format!(
            "value is required for {action}"
        )));
    }
    Ok(names)
}

pub async fn bulk_delete(pool: &PgPool, user_id: &str, ids: Vec<String>) -> AppResult<u64> {
    if ids.len() > 100 {
        return Err(AppError::Validation("at most 100 ids are allowed".into()));
    }
    chunk::delete_many(pool, user_id, &ids).await
}

/// Refuses a self-merge up front so the UI gets a clean 400 rather than a
/// cryptic database error — Node does the same.
pub async fn merge(
    pool: &PgPool,
    user_id: &str,
    source_id: &str,
    target_id: &str,
) -> AppResult<Chunk> {
    if source_id == target_id {
        return Err(AppError::Validation(
            "Cannot merge a chunk into itself".into(),
        ));
    }
    chunk::merge(pool, user_id, source_id, target_id)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))
}
