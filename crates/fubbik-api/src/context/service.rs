//! Ports `enrichChunks` and `resolveFeatureOverlays`
//! (`packages/api/src/context/resolvers.ts:29-133`): fetches full chunk
//! rows for a set of candidate ids, computes health and score, flags
//! staleness/pending-proposal state, and applies active-feature overlays.
//!
//! A per-chunk failure never fails the whole batch — Node wraps every
//! sub-fetch in `Effect.catchAll`, and a chunk that can't be loaded at all
//! (`getChunkById` returns `null`, e.g. the caller doesn't own it) is
//! dropped from the result rather than surfacing an error
//! (`resolvers.ts:37-38`). This mirrors `enrich::routes::enrich_all`'s
//! documented convention for the same reason: "one chunk failing must not
//! abort the batch."

use std::collections::{HashMap, HashSet};

use fubbik_core::error::AppResult;
use fubbik_core::format::ChunkWithMetadata;
use fubbik_core::health::{ChunkHealthInput, compute_health_score};
use fubbik_core::score::{ScoreInput, ScoredChunk, score_chunk};
use fubbik_db::repo::{chunk, connection, feature, proposal, staleness, tag};
use sqlx::PgPool;

/// Fetches, scores, and enriches a set of candidate chunk ids. Ids that
/// don't exist, or aren't `user_id`'s, are silently dropped — same as
/// Node's `null`-filter (`resolvers.ts:83`). **Input order IS preserved**:
/// this loop walks `unique` (deduplicated, first-occurrence order)
/// sequentially, so the result comes back in the same order the ids were
/// given. Node's `Effect.all(..., { concurrency: 8 })` also preserves input
/// order in its return value — `Effect.all`, like `Promise.all`, always
/// returns results indexed by input position regardless of concurrency or
/// completion order; concurrency only affects how many sub-fetches run at
/// once, never the order they're assembled back into. This ordering
/// guarantee matters: `budget_metadata`/`budget_and_format`
/// (`context/routes.rs`) rely on `enrich_chunks`' output order for
/// deterministic tie-breaking when chunks share a score. Do not
/// "optimize" this loop into a concurrent/unordered fetch (e.g.
/// `futures::stream::iter(...).buffer_unordered(8)`) without re-deriving
/// that ordering guarantee some other way first.
pub async fn enrich_chunks(
    pool: &PgPool,
    user_id: &str,
    ids: &[String],
) -> AppResult<Vec<ChunkWithMetadata>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let unique: Vec<String> = {
        let mut seen: HashSet<String> = HashSet::new();
        ids.iter()
            .filter(|id| seen.insert((*id).clone()))
            .cloned()
            .collect()
    };

    let rows = chunk::find_by_ids(pool, user_id, &unique)
        .await
        .unwrap_or_default();
    let row_ids = rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();
    let connection_counts = connection::count_for_chunks(pool, &row_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.chunk_id, row.count))
        .collect::<HashMap<_, _>>();
    let mut tags_by_chunk = HashMap::<String, Vec<String>>::new();
    for row in tag::tags_for_chunks(pool, user_id, &row_ids)
        .await
        .unwrap_or_default()
    {
        tags_by_chunk
            .entry(row.chunk_id)
            .or_default()
            .push(row.tag_name);
    }

    let mut out: Vec<ChunkWithMetadata> = Vec::with_capacity(rows.len());
    for row in rows {
        let connection_count = connection_counts.get(&row.id).copied().unwrap_or(0);
        let tags = tags_by_chunk.remove(&row.id).unwrap_or_default();
        let is_stale = staleness::chunk_is_stale(pool, user_id, &row.id)
            .await
            .unwrap_or(false);
        let has_pending_proposal = proposal::list_for_chunk(pool, &row.id, Some("pending"))
            .await
            .map(|p| !p.is_empty())
            .unwrap_or(false);

        let alternatives: Option<Vec<String>> = row.alternatives.as_ref().map(|j| j.0.clone());
        let health = compute_health_score(&ChunkHealthInput {
            content: &row.content,
            summary: row.summary.as_deref(),
            rationale: row.rationale.as_deref(),
            alternatives: alternatives.as_deref(),
            consequences: row.consequences.as_deref(),
            connection_count,
            // Node hardcodes these four to 0/false inside `enrichChunks`
            // itself (`resolvers.ts:57-61`) rather than computing them —
            // real centrality/requirement-coverage numbers exist elsewhere
            // in this port (`chunks::service::get`'s detail path) but this
            // pipeline never fetches them, matching Node exactly.
            centrality_degree: 0,
            has_embedding: row.embedding.is_some(),
            requirement_count: 0,
            all_requirements_passing: false,
            referenced_in_session: false,
        });

        let score = score_chunk(&ScoreInput {
            chunk_type: &row.chunk_type,
            rationale: row.rationale.as_deref(),
            review_status: &row.review_status,
            connection_count,
            health: &health,
        });

        out.push(ChunkWithMetadata {
            chunk: ScoredChunk {
                id: row.id,
                title: row.title,
                content: row.content,
                chunk_type: row.chunk_type,
                rationale: row.rationale,
                tags,
                score,
            },
            health_score: health.total,
            is_stale,
            has_pending_proposal,
        });
    }

    resolve_feature_overlays(pool, user_id, out).await
}

/// Ports `resolveFeatureOverlays` (`resolvers.ts:92-133`): applies active
/// feature deltas on top of the enriched chunk, highest priority last so it
/// wins a same-field conflict (`Object.assign(overlay, ...deltasAscByPriority)`).
///
/// Only `title`/`content`/`type`/`rationale` are overlaid, matching Node's
/// explicit four-field spread (`resolvers.ts:126-129`) — `summary`,
/// `alternatives`, `consequences` may be present in a delta but are never
/// applied here, because `ChunkWithMetadata` doesn't carry them and Node's
/// own overlay object doesn't either.
async fn resolve_feature_overlays(
    pool: &PgPool,
    user_id: &str,
    chunks: Vec<ChunkWithMetadata>,
) -> AppResult<Vec<ChunkWithMetadata>> {
    if chunks.is_empty() {
        return Ok(chunks);
    }

    let active: HashSet<String> = feature::active_feature_ids(pool, user_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    if active.is_empty() {
        return Ok(chunks);
    }

    let mut out = chunks;
    for meta in out.iter_mut() {
        let deltas = feature::deltas_for_chunk(pool, &meta.chunk.id, user_id)
            .await
            .unwrap_or_default();

        // `deltas_for_chunk` returns every feature's delta on this chunk,
        // ordered `priority ASC`; keep only the active ones (Node's
        // `batchFetchDeltas` filters this at the query level instead, but
        // the fold order — and therefore the result — is identical either
        // way) and fold field-by-field, later (higher-priority) deltas
        // winning, exactly like `Object.assign(overlay, d.delta)`.
        let mut overlay = serde_json::Map::new();
        for d in deltas {
            if !active.contains(&d.feature_id) {
                continue;
            }
            if let serde_json::Value::Object(map) = &d.delta.0 {
                for (k, v) in map {
                    overlay.insert(k.clone(), v.clone());
                }
            }
        }

        if let Some(v) = overlay
            .get("title")
            .filter(|v| !v.is_null())
            .and_then(|v| v.as_str())
        {
            meta.chunk.title = v.to_string();
        }
        if let Some(v) = overlay
            .get("content")
            .filter(|v| !v.is_null())
            .and_then(|v| v.as_str())
        {
            meta.chunk.content = v.to_string();
        }
        if let Some(v) = overlay
            .get("type")
            .filter(|v| !v.is_null())
            .and_then(|v| v.as_str())
        {
            meta.chunk.chunk_type = v.to_string();
        }
        if let Some(v) = overlay.get("rationale").filter(|v| !v.is_null()) {
            meta.chunk.rationale = v.as_str().map(str::to_string);
        }
    }

    Ok(out)
}
