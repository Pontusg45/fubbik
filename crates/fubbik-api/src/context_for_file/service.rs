//! Ports `getContextForFile` (`packages/api/src/context-for-file/
//! service.ts:72-332`) — five ranking strategies over a single file path.
//! **First-strategy-wins, not additive**: a chunk found by more than one
//! strategy is scored and labelled by whichever strategy finds it first
//! (see below); it never accumulates more than one bonus. Each strategy's
//! own bonus, for the chunks it alone contributes:
//!
//! | strategy    | bonus | requires                  |
//! |-------------|-------|----------------------------|
//! | file-ref    | +20   | —                          |
//! | applies-to  | +10   | —                          |
//! | dependency  | +3    | `deps` (json-legacy only)  |
//! | semantic    | +5    | Ollama; capped at 10 hits  |
//! | connected   | +2    | —                          |
//!
//! A chunk found by exactly one strategy keeps that strategy's name as its
//! `matchReason` and gets ONLY that strategy's bonus on top of its base
//! score — **except semantic**: `chunk_rows` (the map this function scores
//! from) is populated by file-ref, applies-to, dependency and connected,
//! but deliberately NOT by semantic, because `semantic_search` returns a
//! narrower row shape (no `rationale`/`alternatives`/`consequences`/
//! `embedding`) that `score_chunk`/`compute_health_score` cannot be run on.
//! A chunk found ONLY by semantic therefore gets a base score of exactly
//! `0.0`, not a health-derived one — this is Node's behaviour verbatim
//! (`service.ts:171-191` never calls `chunkRows.set` in that block), not an
//! oversight to "fix" here.
//!
//! First-strategy-wins: once a chunk id has an entry, later strategies
//! that would also find it skip it entirely (matching Node's `if
//! (results.has(id)) continue`) — a chunk found by both file-ref and
//! applies-to keeps `matchReason: "file-ref"` and only the file-ref bonus,
//! never both.

use std::collections::{HashMap, HashSet};

use fubbik_core::error::AppResult;
use fubbik_core::glob::glob_match;
use fubbik_core::health::{ChunkHealthInput, compute_health_score};
use fubbik_core::score::{ScoreInput, score_chunk};
use fubbik_db::age;
use fubbik_db::repo::{chunk, chunk_meta, connection, requirement, semantic, space};
use sqlx::PgPool;

use super::dto::{
    ContextChunk, ContextRequirement, ContextRequirementStep, FileContext, MatchReason,
};

/// Segments `pathToSearchText` drops as noise — matches
/// `service.ts:49` verbatim.
const IGNORED_SEGMENTS: [&str; 8] = [
    "src",
    "lib",
    "dist",
    "build",
    "index",
    "node_modules",
    "packages",
    "apps",
];
/// Matches `service.ts:50` verbatim.
const IGNORED_EXTENSIONS: [&str; 8] = ["ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "md"];

/// Ports `pathToSearchText` (`service.ts:52-57`): splits on `/` and `.`,
/// drops empty/ignored segments, and joins the rest with spaces — the query
/// text fed to the semantic strategy's embedding call.
fn path_to_search_text(file_path: &str) -> String {
    file_path
        .split(['/', '.'])
        .filter(|seg| {
            !seg.is_empty() && !IGNORED_SEGMENTS.contains(seg) && !IGNORED_EXTENSIONS.contains(seg)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ports `depMatchesCodebase` (`service.ts:63-70`): exact match, or a match
/// on the scoped-package's last segment (`"@acme/auth"` matches `"auth"`).
fn dep_matches_space(dep: &str, space_name: &str) -> bool {
    let dep_lower = dep.to_lowercase();
    let space_lower = space_name.to_lowercase();
    if dep_lower == space_lower {
        return true;
    }
    let last_segment = dep_lower.rsplit('/').next().unwrap_or(&dep_lower);
    last_segment == space_lower
}

/// Ports `RELATION_PRIORITY` (`service.ts:201-207`) — how the "connected"
/// strategy ranks candidate neighbours before taking the top five.
fn relation_priority(relation: &str) -> i32 {
    match relation {
        "part_of" => 4,
        "depends_on" => 3,
        "extends" => 2,
        "references" => 1,
        _ => 0, // "related_to" and anything unrecognised.
    }
}

// Ports `STRATEGY_BONUS` (`service.ts:277-283`). Each constant is
// independently named (rather than folded into one match arm) so Step 5's
// mutation target — "set the file-ref bonus from 20 to 1" — is a one-line
// edit that cannot be confused with any other strategy's bonus.
const BONUS_FILE_REF: f64 = 20.0;
const BONUS_APPLIES_TO: f64 = 10.0;
const BONUS_DEPENDENCY: f64 = 3.0;
const BONUS_SEMANTIC: f64 = 5.0;
const BONUS_CONNECTED: f64 = 2.0;
/// Ports `GRAPH_PROXIMITY_WEIGHT` (`service.ts:284`).
const GRAPH_PROXIMITY_WEIGHT: f64 = 5.0;

fn strategy_bonus(reason: MatchReason) -> f64 {
    match reason {
        MatchReason::FileRef => BONUS_FILE_REF,
        MatchReason::AppliesTo => BONUS_APPLIES_TO,
        MatchReason::Dependency => BONUS_DEPENDENCY,
        MatchReason::Semantic => BONUS_SEMANTIC,
        MatchReason::Connected => BONUS_CONNECTED,
    }
}

/// The five-strategy matcher backing `GET /api/context/for-file`'s
/// `json-legacy` format, and (via `resolve_for_files`) the id-resolution
/// step of `/api/context/for-files` and the `structured-md`/
/// `structured-json` formats of this same route.
///
/// `deps` is `None` on every call from `resolve_for_files` (Node's own
/// `resolveForFiles` never passes a third argument to `getContextForFile`,
/// `resolvers.ts:220-224`) — only the `json-legacy` HTTP handler threads a
/// caller-supplied `deps` list through.
pub async fn get_context_for_file(
    pool: &PgPool,
    ai: &fubbik_ai::OllamaClient,
    background: &crate::background::BackgroundRuntime,
    user_id: &str,
    file_path: &str,
    space_id: Option<&str>,
    deps: Option<&[String]>,
) -> AppResult<FileContext> {
    let mut order: Vec<String> = Vec::new();
    let mut results: HashMap<String, ContextChunk> = HashMap::new();
    let mut chunk_rows: HashMap<String, chunk::Chunk> = HashMap::new();

    // ------------------------------------------------------------------
    // 1. Direct file-ref matches.
    // ------------------------------------------------------------------
    if let Ok(file_ref_ids) =
        chunk_meta::lookup_chunk_ids_by_path(pool, file_path, user_id, space_id).await
    {
        for id in file_ref_ids {
            if results.contains_key(&id) {
                continue;
            }
            let Ok(Some(full)) = chunk::find_by_id(pool, user_id, &id).await else {
                continue;
            };
            results.insert(
                full.id.clone(),
                ContextChunk {
                    id: full.id.clone(),
                    title: full.title.clone(),
                    chunk_type: full.chunk_type.clone(),
                    content: full.content.clone(),
                    summary: full.summary.clone(),
                    match_reason: MatchReason::FileRef,
                    score: 0.0,
                },
            );
            order.push(full.id.clone());
            chunk_rows.insert(full.id.clone(), full);
        }
    }

    // ------------------------------------------------------------------
    // 2. Applies-to glob matches, over chunks not already matched.
    // ------------------------------------------------------------------
    // Node calls the repository `listChunks` directly at `limit: 1000` with
    // no cap (`context-for-file/service.ts:97-101`). `chunk::list` clamps
    // to `[1,100]` regardless of `params.limit` (see its own doc comment),
    // so this internal, non-HTTP-triggered caller must go through
    // `chunk::list_internal` instead — same query, same ordering, but a
    // caller-chosen bound instead of the 100-row HTTP clamp. Using `list`
    // here would silently glob-check only the 100 newest chunks instead of
    // 1000.
    let list_params = chunk::ListParams {
        space_id: space_id.map(str::to_string),
        limit: 1000,
        offset: 0,
        ..Default::default()
    };
    let all_chunks = chunk::list_internal(pool, user_id, &list_params, 1000)
        .await
        .unwrap_or_default();
    let unchecked: Vec<String> = all_chunks
        .iter()
        .filter(|c| !results.contains_key(&c.id))
        .map(|c| c.id.clone())
        .collect();
    if !unchecked.is_empty()
        && let Ok(patterns) = chunk_meta::get_applies_to_for_chunks(pool, &unchecked).await
    {
        let mut patterns_by_chunk: HashMap<String, Vec<String>> = HashMap::new();
        for p in patterns {
            patterns_by_chunk
                .entry(p.chunk_id)
                .or_default()
                .push(p.pattern);
        }
        for c in &all_chunks {
            if results.contains_key(&c.id) {
                continue;
            }
            let Some(pats) = patterns_by_chunk.get(&c.id) else {
                continue;
            };
            if pats.iter().any(|pat| glob_match(pat, file_path)) {
                results.insert(
                    c.id.clone(),
                    ContextChunk {
                        id: c.id.clone(),
                        title: c.title.clone(),
                        chunk_type: c.chunk_type.clone(),
                        content: c.content.clone(),
                        summary: c.summary.clone(),
                        match_reason: MatchReason::AppliesTo,
                        score: 0.0,
                    },
                );
                order.push(c.id.clone());
                chunk_rows.insert(c.id.clone(), c.clone());
            }
        }
    }

    // ------------------------------------------------------------------
    // 3. Dependency-based matches (only when `deps` is non-empty).
    // ------------------------------------------------------------------
    if let Some(deps) = deps
        && !deps.is_empty()
    {
        let spaces = space::list(pool, user_id).await.unwrap_or_default();
        let matched_space_ids: Vec<String> = spaces
            .iter()
            .filter(|s| deps.iter().any(|d| dep_matches_space(d, &s.name)))
            .map(|s| s.id.clone())
            .collect();

        for matched_space_id in matched_space_ids {
            let params = chunk::ListParams {
                space_id: Some(matched_space_id),
                limit: 5,
                offset: 0,
                sort: chunk::Sort::Updated,
                ..Default::default()
            };
            if let Ok(dep_chunks) = chunk::list(pool, user_id, &params).await {
                for c in dep_chunks {
                    if results.contains_key(&c.id) {
                        continue;
                    }
                    results.insert(
                        c.id.clone(),
                        ContextChunk {
                            id: c.id.clone(),
                            title: c.title.clone(),
                            chunk_type: c.chunk_type.clone(),
                            content: c.content.clone(),
                            summary: c.summary.clone(),
                            match_reason: MatchReason::Dependency,
                            score: 0.0,
                        },
                    );
                    order.push(c.id.clone());
                    chunk_rows.insert(c.id.clone(), c);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // 4. Semantic similarity — requires Ollama; skips silently otherwise.
    //    Deliberately does NOT populate `chunk_rows` — see the module doc.
    // ------------------------------------------------------------------
    let search_text = path_to_search_text(file_path);
    if !search_text.is_empty()
        && let Ok(embedding) = ai.embed_query(&search_text).await
        && let Ok(hits) =
            semantic::semantic_search(pool, &embedding, Some(user_id), &[], None, 10).await
    {
        for sc in hits {
            if results.contains_key(&sc.id) {
                continue;
            }
            results.insert(
                sc.id.clone(),
                ContextChunk {
                    id: sc.id.clone(),
                    title: sc.title.clone(),
                    chunk_type: sc.chunk_type.clone(),
                    content: sc.content.clone(),
                    summary: sc.summary.clone(),
                    match_reason: MatchReason::Semantic,
                    score: 0.0,
                },
            );
            order.push(sc.id);
        }
    }

    // ------------------------------------------------------------------
    // 5. Connection expansion — top five graph neighbours of everything
    //    found so far, ranked by relation priority.
    // ------------------------------------------------------------------
    if !order.is_empty() {
        let all_connections = connection::connections_for_chunks(pool, &order)
            .await
            .unwrap_or_default();
        let current_set: HashSet<String> = order.iter().cloned().collect();

        let mut candidates: Vec<(String, i32)> = Vec::new();
        for conn in &all_connections {
            let connected_id = if current_set.contains(&conn.source_id) {
                conn.target_id.clone()
            } else {
                conn.source_id.clone()
            };
            if !results.contains_key(&connected_id) {
                candidates.push((connected_id, relation_priority(&conn.relation)));
            }
        }
        // Stable sort, descending priority — ties keep discovery order,
        // matching `Array.prototype.sort`'s stability.
        candidates.sort_by_key(|c| std::cmp::Reverse(c.1));

        for (candidate_id, _priority) in candidates.into_iter().take(5) {
            let Ok(Some(full)) = chunk::find_by_id(pool, user_id, &candidate_id).await else {
                continue;
            };
            let is_new = !results.contains_key(&full.id);
            results.insert(
                full.id.clone(),
                ContextChunk {
                    id: full.id.clone(),
                    title: full.title.clone(),
                    chunk_type: full.chunk_type.clone(),
                    content: full.content.clone(),
                    summary: full.summary.clone(),
                    match_reason: MatchReason::Connected,
                    score: 0.0,
                },
            );
            if is_new {
                order.push(full.id.clone());
            }
            chunk_rows.insert(full.id.clone(), full);
        }
    }

    // ------------------------------------------------------------------
    // Score every matched chunk, then sort descending (stable, so ties
    // keep the strategies' first-encounter order).
    // ------------------------------------------------------------------
    let chunk_ids_for_scoring = order.clone();

    let connections = connection::connections_for_chunks(pool, &chunk_ids_for_scoring)
        .await
        .unwrap_or_default();
    let mut conn_count_map: HashMap<String, i64> = HashMap::new();
    for conn in &connections {
        *conn_count_map.entry(conn.source_id.clone()).or_insert(0) += 1;
        *conn_count_map.entry(conn.target_id.clone()).or_insert(0) += 1;
    }

    let degree_map = age::get_connection_degrees(pool, &chunk_ids_for_scoring).await;

    let anchor_ids: Vec<String> = order
        .iter()
        .filter(|id| {
            matches!(
                results.get(*id).map(|c| c.match_reason),
                Some(MatchReason::FileRef) | Some(MatchReason::AppliesTo)
            )
        })
        .cloned()
        .collect();
    let semantic_ids: Vec<String> = order
        .iter()
        .filter(|id| {
            matches!(
                results.get(*id).map(|c| c.match_reason),
                Some(MatchReason::Semantic)
            )
        })
        .cloned()
        .collect();

    let graph_boosts = if !anchor_ids.is_empty() && !semantic_ids.is_empty() {
        age::get_graph_proximity_boost(pool, &anchor_ids[0], &semantic_ids, 3).await
    } else {
        HashMap::new()
    };

    for id in &order {
        let raw_row = chunk_rows.get(id);
        let connection_count = conn_count_map.get(id).copied().unwrap_or(0);
        let centrality_degree = degree_map.get(id).copied().unwrap_or(0);

        // Deliberately no base score when `chunk_rows` has no full row for
        // this id — the semantic-only case documented at module level.
        let base_score = if let Some(row) = raw_row {
            let alternatives: Option<Vec<String>> = row.alternatives.as_ref().map(|j| j.0.clone());
            let health = compute_health_score(&ChunkHealthInput {
                content: &row.content,
                summary: row.summary.as_deref(),
                rationale: row.rationale.as_deref(),
                alternatives: alternatives.as_deref(),
                consequences: row.consequences.as_deref(),
                connection_count,
                centrality_degree,
                has_embedding: row.embedding.is_some(),
                requirement_count: 0,
                all_requirements_passing: false,
                referenced_in_session: false,
            });
            score_chunk(&ScoreInput {
                chunk_type: &row.chunk_type,
                rationale: row.rationale.as_deref(),
                review_status: &row.review_status,
                connection_count,
                health: &health,
            })
        } else {
            0.0
        };

        let Some(entry) = results.get_mut(id) else {
            continue;
        };
        let bonus = strategy_bonus(entry.match_reason);
        let proximity = graph_boosts.get(id).copied().unwrap_or(0.0) * GRAPH_PROXIMITY_WEIGHT;
        entry.score = base_score + bonus + proximity;
    }

    let mut matched_chunks: Vec<ContextChunk> =
        order.iter().filter_map(|id| results.remove(id)).collect();
    matched_chunks.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // ------------------------------------------------------------------
    // 6. Requirements linked to the matched chunks.
    // ------------------------------------------------------------------
    let matched_chunk_ids: Vec<String> = matched_chunks.iter().map(|c| c.id.clone()).collect();
    let mut requirements: Vec<ContextRequirement> = Vec::new();
    if !matched_chunk_ids.is_empty()
        && let Ok(rows) =
            requirement::requirements_for_chunks(pool, &matched_chunk_ids, user_id).await
    {
        let mut req_order: Vec<String> = Vec::new();
        let mut req_map: HashMap<String, ContextRequirement> = HashMap::new();
        for row in rows {
            if let Some(existing) = req_map.get_mut(&row.id) {
                existing.matched_chunk_ids.push(row.chunk_id);
            } else {
                let steps = row
                    .steps
                    .0
                    .iter()
                    .map(|s| ContextRequirementStep {
                        keyword: s.keyword,
                        text: s.text.clone(),
                    })
                    .collect();
                req_map.insert(
                    row.id.clone(),
                    ContextRequirement {
                        id: row.id.clone(),
                        title: row.title,
                        status: row.status,
                        priority: row.priority,
                        steps,
                        matched_chunk_ids: vec![row.chunk_id],
                    },
                );
                req_order.push(row.id);
            }
        }
        requirements = req_order
            .into_iter()
            .filter_map(|id| req_map.remove(&id))
            .collect();
    }

    // Fire-and-forget: strengthen edges between co-accessed chunks. Never
    // awaited and never allowed to fail this request — matches Node's
    // `Effect.runPromise(...).catch(() => {})` (`service.ts:327`).
    if matched_chunks.len() >= 2 {
        let co_accessed: Vec<String> = matched_chunks
            .iter()
            .take(10)
            .map(|c| c.id.clone())
            .collect();
        let pool_clone = pool.clone();
        background.spawn(async move {
            if let Err(error) =
                connection::increment_connection_weights(&pool_clone, &co_accessed).await
            {
                tracing::warn!(%error, "failed to update co-access connection weights");
            }
        });
    }

    Ok(FileContext {
        chunks: matched_chunks,
        requirements,
    })
}
