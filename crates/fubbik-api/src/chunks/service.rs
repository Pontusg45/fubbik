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

pub async fn create(pool: &PgPool, user_id: &str, body: CreateChunkBody) -> AppResult<Chunk> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if title.chars().count() > 200 {
        return Err(AppError::Validation(
            "title must be at most 200 characters".into(),
        ));
    }

    chunk::create(
        pool,
        user_id,
        NewChunk {
            title: title.to_string(),
            content: body.content,
            chunk_type: body.chunk_type.unwrap_or_else(|| "note".into()),
            rationale: body.rationale,
        },
    )
    .await
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
        .map(|title| {
            let trimmed = title.trim();
            if trimmed.is_empty() {
                return Err(AppError::Validation("title is required".into()));
            }
            if trimmed.chars().count() > 200 {
                return Err(AppError::Validation(
                    "title must be at most 200 characters".into(),
                ));
            }
            Ok(trimmed.to_string())
        })
        .transpose()?;

    let current = get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::snapshot(pool, &current).await?;

    chunk::update(
        pool,
        user_id,
        id,
        ChunkPatch {
            title,
            content: body.content,
            chunk_type: body.chunk_type,
            rationale: body.rationale,
            consequences: body.consequences,
            // `UpdateChunkBody` (the regular `PATCH /chunks/{id}` body) has
            // no `alternatives`/`scope` fields — only the proposals domain's
            // atomic approve path sets these, via its own dedicated repo
            // function. Leaving both `None` here keeps this call's observed
            // behaviour byte-for-byte identical to before `ChunkPatch` grew
            // the two fields.
            alternatives: None,
            scope: None,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("chunk".into()))
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
