//! Business logic for the `search` domain. Two halves:
//!
//! - [`execute_search`] and [`autocomplete`]: read-only, **infallible**
//!   functions (`-> SearchResult` / `-> Vec<String>`, not `AppResult<..>`)
//!   — a direct port of Node's `Effect<T, never>` signatures
//!   (`packages/api/src/search/service.ts:77`, `:257`). Every internal
//!   step in Node is wrapped `.pipe(Effect.orElse(() => Effect.succeed(...)))`,
//!   so even a database error degrades to an empty result rather than
//!   propagating — reproduced here by catching each fallible call site and
//!   substituting the same fallback Node does, not by wrapping the whole
//!   function in a try/catch. `tests/search.rs`'s
//!   `a_failing_query_degrades_to_empty_results_not_a_500` proves this by
//!   calling `execute_search` against a closed pool.
//! - [`list_saved`], [`create_saved`], [`delete_saved`]: thin `AppResult`
//!   wrappers over `fubbik_db::repo::saved_query`, the same shape as every
//!   other CRUD domain in this crate (`favorites::service`, etc.) — these
//!   *can* fail, and Node doesn't wrap them in `orElse` either
//!   (`packages/api/src/search/routes.ts:67-112` lets them propagate).
//!
//! ## The four graph clauses (Task 9)
//!
//! `near`, `path`, and `affected-by` resolve against Apache AGE
//! (`fubbik_db::age`); `similar-to` is meant to resolve via a pgvector
//! embedding search, but no Ollama/embedding pipeline exists anywhere in
//! this Rust port yet (`crates/fubbik-api` never calls out to Ollama), so
//! it always resolves to zero ids — the same degrade-to-empty result Node
//! produces when `generateQueryEmbedding` itself fails
//! (`service.ts:117-121`'s own `Effect.orElse`). [`resolve_graph_clauses`]
//! does the resolving; [`execute_search`] intersects a query's standard
//! (non-graph) filters with whatever ids came back, exactly like Node's
//! `graphIds ? graphIds.filter(...) : ids` loop (`service.ts:88-127`).
//!
//! The underlying `fubbik_db::age` functions (`get_neighborhood`,
//! `find_shortest_path_with_details`, `get_chunks_affected_by_requirement`)
//! *themselves* already degrade every AGE failure to `Ok(vec![])`/`Ok(None)`
//! rather than erroring — matching Node's `Effect.orElse(() =>
//! Effect.succeed([]))` on each clause (`service.ts:92,99,111-113,120`) — so
//! none of them can return `Err` under their current implementation. Each
//! call site here still handles the `Err` arm explicitly rather than
//! `.unwrap_or_default()`/`.unwrap_or(None)`-ing it away: it logs via
//! `tracing::error!` and substitutes the same fallback Node's `Effect.orElse`
//! would, so a future regression in `age.rs`'s own degrade branch (see that
//! module's `Err(_) => Ok(...)` arms) surfaces in logs instead of vanishing
//! silently a second time. Behaviour is unchanged either way — a graph
//! clause degrades to empty results, never a 500 — only observability of an
//! (currently unreachable) unexpected error improves.
//!
//! `graphMeta.type` carries a fourth value Node's own `types.ts:51-53`
//! declares only three literals for (`"neighborhood" | "path" |
//! "requirement-reach"`): `similar-to` sets it to the literal string
//! `"semantic"` (`service.ts:123`'s `"semantic" as any` — a compile-time
//! escape hatch that has zero effect on the runtime value, confirmed by
//! `tests/fixtures/node-contract-2c/_questions.md` Q2). This port
//! preserves that runtime value rather than "fixing" it to fit the
//! narrower `GraphMeta` shape.
//!
//! Resolved graph ids are never trusted directly: they come from AGE,
//! which knows nothing about `user_id` (a `:connects` edge can point at
//! another user's chunk). They're applied as an ordinary `ids` filter on
//! [`chunk::ListParams`], ANDed with `chunk::list`/`chunk::count`'s own
//! mandatory `user_id = ..` predicate — never fetched by id in a
//! separate, unscoped query. See `tests/search.rs`'s cross-user graph-edge
//! test for the end-to-end proof.

use fubbik_core::error::AppResult;
use fubbik_db::age;
use fubbik_db::repo::{chunk, connection, requirement, saved_query, tag};
use sqlx::PgPool;

use super::dto::{
    CreateSavedQueryBody, GraphContext, GraphMeta, PathEdgeInfo, SearchQueryBody, SearchResult,
    SearchResultChunk,
};
use super::parser::QueryClause;
use crate::chunks::health_score::{self, ChunkHealthInput};

/// Port of `mapSortParam` (`service.ts:30-34`) fused with `listChunks`'s
/// own `switch (params.sort)` default arm (`packages/db/src/repository/chunk.ts:128-141`):
/// Node's `mapSortParam` turns `"relevance"`/`undefined`/anything
/// unrecognised into `undefined`, and `listChunks` then falls through its
/// `switch`'s `default:` case for an `undefined`/unrecognised sort, which
/// is `"newest"`. Folding both steps into one match reproduces the same
/// end-to-end result without a fake intermediate `undefined` state.
/// Notably, `"alpha"` is not one of `SearchQuery.sort`'s four accepted
/// literals in Node's own type (`types.ts:12`) — the search domain never
/// exposes alphabetical sort, unlike `chunk::Sort::from_loose_str`'s
/// caller in `collections`, which does.
fn map_sort_param(sort: Option<&str>) -> chunk::Sort {
    match sort {
        Some("oldest") => chunk::Sort::Oldest,
        Some("updated") => chunk::Sort::Updated,
        // "newest", "relevance", None, or anything unrecognised.
        _ => chunk::Sort::Newest,
    }
}

/// Port of `buildListChunksParams` (`service.ts:36-75`): folds the
/// non-graph clauses plus the query's own `sort`/`limit`/`offset`/
/// `spaceId` into a `chunk::ListParams`. Later clauses of the same
/// `field` overwrite earlier ones — a plain assignment inside Node's `for`
/// loop, reproduced the same way here.
fn build_list_params(clauses: &[QueryClause], query: &SearchQueryBody) -> chunk::ListParams {
    let mut params = chunk::ListParams {
        sort: map_sort_param(query.sort.as_deref()),
        limit: query.limit.unwrap_or(50),
        offset: query.offset.unwrap_or(0),
        space_id: query.space_id.clone(),
        ..Default::default()
    };

    for clause in clauses {
        match clause.field.as_str() {
            "type" => params.chunk_type = Some(clause.value.clone()),
            "tag" => {
                let tags: Vec<String> = clause
                    .value
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                params.tags = Some(tags);
            }
            "text" => params.search = Some(clause.value.clone()),
            "connections" => {
                // Node: `params.minConnections = Number(clause.value)`,
                // unconditionally — a non-numeric value produces `NaN`.
                // `listChunks`'s own filter guard is `if (params.minConnections
                // && params.minConnections > 0)`, and `NaN` is falsy in JS, so
                // the filter is simply never applied — not "matches nothing",
                // "no filter applied at all". Reproduced by leaving
                // `min_connections` unset (`None`) on a parse failure, the
                // same effective outcome. See the task report for this
                // explicit choice.
                if let Ok(n) = clause.value.parse::<i64>() {
                    params.min_connections = Some(n);
                }
            }
            "updated" => {
                // Node: `params.after = new Date(Date.now() - Number(clause.value)
                // * 86400000)`. A non-numeric value produces an `Invalid Date`,
                // which is still a truthy object in JS — not the same "silently
                // skip the filter" story `connections` has. This port does not
                // attempt to reproduce Node's `Invalid Date` SQL behaviour
                // (unspecified, and not pinned by any captured fixture); a
                // non-numeric `updated:` value is treated the same defensive
                // way `connections:` is: no filter applied, not a crash.
                if let Ok(days) = clause.value.parse::<i64>() {
                    let after = chrono::Utc::now() - chrono::Duration::days(days);
                    params.after = Some(after.naive_utc());
                }
            }
            "origin" => params.origin = Some(clause.value.clone()),
            "review" => params.review_status = Some(clause.value.clone()),
            _ => {}
        }
    }

    params
}

/// `chunk::list` + `chunk::count` together, so a failure in either one
/// degrades the whole page — matching Node's single `listChunks(params)`
/// call, which returns both `chunks` and `total` from one query and is
/// wrapped in one `Effect.orElse`.
async fn list_and_count(
    pool: &PgPool,
    user_id: &str,
    params: &chunk::ListParams,
) -> AppResult<(Vec<chunk::Chunk>, i64)> {
    let chunks = chunk::list(pool, user_id, params).await?;
    let total = chunk::count(pool, user_id, params).await?;
    Ok((chunks, total))
}

/// `graphIds ? graphIds.filter(id => ids.includes(id)) : ids` — Node's
/// intersection rule (`service.ts:93`, `:101`, `:114`, `:122`), reproduced
/// once here since all four graph-clause branches apply it identically. The
/// first graph clause in a query seeds `graph_ids`; every subsequent one
/// narrows it. `O(n*m)` like Node's own `.includes()` in a `.filter()` —
/// result sets here are graph-neighborhood-sized, not full-table scans.
fn intersect_ids(existing: Option<Vec<String>>, ids: Vec<String>) -> Vec<String> {
    match existing {
        Some(current) => current.into_iter().filter(|id| ids.contains(id)).collect(),
        None => ids,
    }
}

/// Output of [`resolve_graph_clauses`]: the intersected id set (`None` if
/// the query had no graph clause at all — distinct from `Some(vec![])`,
/// which means a graph clause resolved to nothing), the `graphMeta` the
/// last-processed graph clause produced, and just enough side data to
/// populate per-chunk `graphContext` afterwards without re-querying AGE.
#[derive(Default)]
struct GraphResolution {
    ids: Option<Vec<String>>,
    meta: Option<GraphMeta>,
    /// `near`'s effective hop count — used as the `hopDistance` fallback
    /// for every resolved id, matching Node's own fallback
    /// (`hopMap.get(id) ?? neighborhoodRef.maxHops`, `service.ts:201`) in
    /// the (here, permanent) case where hop-distance data isn't available.
    near_hops: Option<i64>,
    /// `path`'s full resolved chunk chain (source to target inclusive),
    /// kept separately from `ids` because a later graph clause can narrow
    /// `ids` further — `graphContext.pathPosition` still indexes into the
    /// original chain, matching Node's `graphMeta.pathChunks.forEach(...)`
    /// (`service.ts:193`), not the post-intersection set.
    path_chunks: Option<Vec<String>>,
    /// `similar-to`'s raw query text, for the `matchedRequirement` context
    /// message (`service.ts:209`) — never actually reached today, since
    /// `similar-to` always resolves to zero ids (see the module doc), but
    /// kept for shape parity with Node's structure.
    similar_to_query: Option<String>,
}

/// Resolves every graph clause (`near`/`path`/`affected-by`/`similar-to`)
/// in `clauses` against Apache AGE, matching Node's sequential
/// `for (const clause of graphClauses)` loop (`service.ts:88-127`). Clauses
/// with any other field are ignored here (the same implicit no-op
/// `build_list_params`'s own `_ => {}` arm gives them), so callers can pass
/// a query's *entire* clause list rather than pre-filtering it.
async fn resolve_graph_clauses(pool: &PgPool, clauses: &[QueryClause]) -> GraphResolution {
    let mut out = GraphResolution::default();

    for clause in clauses {
        match clause.field.as_str() {
            "near" => {
                let hops = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("hops"))
                    .and_then(|h| h.parse::<i32>().ok())
                    .unwrap_or(1);
                let resolved = age::get_neighborhood(pool, &clause.value, hops)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(
                            error = %err,
                            "age::get_neighborhood returned Err instead of degrading internally to Ok(vec![]) — this is a bug in age.rs, not expected at this call site"
                        );
                        Vec::new()
                    });
                out.ids = Some(intersect_ids(out.ids, resolved));
                out.meta = Some(GraphMeta {
                    meta_type: "neighborhood".to_string(),
                    reference_chunk: Some(clause.value.clone()),
                    path_chunks: None,
                    path_edges: None,
                    hops: None,
                });
                out.near_hops = Some(hops as i64);
            }
            "path" => {
                // Node's parser (`parser.ts:71-76`) puts the resolved
                // endpoints in `clause.params.from`/`.to`, NOT in
                // `clause.value` split on a comma — `clause.value` is just
                // `from` alone (`tests/fixtures/node-contract-2c/search-parse-path-A-to-B.json`).
                // Node's own `executeSearch` reads `clause.value.split(",")`
                // instead (`service.ts:97`), which can never produce two
                // elements given that shape, making the `path` clause dead
                // code in Node today. This port reads `params.from`/`.to`
                // directly so `path:` actually resolves, rather than
                // reproducing what looks like an unintentional no-op bug —
                // see the task report.
                let from = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("from"))
                    .map(String::as_str)
                    .filter(|v| !v.is_empty());
                let to = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("to"))
                    .map(String::as_str)
                    .filter(|v| !v.is_empty());
                if let (Some(from), Some(to)) = (from, to) {
                    let detail = age::find_shortest_path_with_details(pool, from, to)
                        .await
                        .unwrap_or_else(|err| {
                            tracing::error!(
                                error = %err,
                                "age::find_shortest_path_with_details returned Err instead of degrading internally to Ok(None) — this is a bug in age.rs, not expected at this call site"
                            );
                            None
                        });
                    let resolved = detail
                        .as_ref()
                        .map(|d| d.chunk_ids.clone())
                        .unwrap_or_default();
                    let edges: Vec<PathEdgeInfo> = detail
                        .as_ref()
                        .map(|d| {
                            d.edges
                                .iter()
                                .map(|e| PathEdgeInfo {
                                    source: e.source.clone(),
                                    target: e.target.clone(),
                                    relation: e.relation.clone(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let hop_count = detail.as_ref().map(|d| d.edges.len() as i64).unwrap_or(0);

                    out.ids = Some(intersect_ids(out.ids, resolved.clone()));
                    out.meta = Some(GraphMeta {
                        meta_type: "path".to_string(),
                        reference_chunk: None,
                        path_chunks: Some(resolved.clone()),
                        path_edges: Some(edges),
                        hops: Some(hop_count),
                    });
                    out.path_chunks = Some(resolved);
                }
            }
            "affected-by" => {
                let hops = clause
                    .params
                    .as_ref()
                    .and_then(|p| p.get("hops"))
                    .and_then(|h| h.parse::<i32>().ok())
                    .unwrap_or(2);
                let resolved = age::get_chunks_affected_by_requirement(pool, &clause.value, hops)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!(
                            error = %err,
                            "age::get_chunks_affected_by_requirement returned Err instead of degrading internally to Ok(vec![]) — this is a bug in age.rs, not expected at this call site"
                        );
                        Vec::new()
                    });
                out.ids = Some(intersect_ids(out.ids, resolved));
                out.meta = Some(GraphMeta {
                    meta_type: "requirement-reach".to_string(),
                    reference_chunk: None,
                    path_chunks: None,
                    path_edges: None,
                    hops: None,
                });
            }
            "similar-to" => {
                // No embedding/Ollama pipeline exists anywhere in this Rust
                // port yet, so this always resolves to zero ids — the same
                // degrade-to-empty result Node produces when
                // `generateQueryEmbedding` itself fails
                // (`service.ts:117-121`'s own `Effect.orElse`). `graphMeta.type`
                // still comes back as the literal string `"semantic"`
                // (see the module doc) — that fidelity holds even though
                // the id resolution isn't implemented yet.
                let resolved: Vec<String> = Vec::new();
                out.ids = Some(intersect_ids(out.ids, resolved));
                out.meta = Some(GraphMeta {
                    meta_type: "semantic".to_string(),
                    reference_chunk: Some(clause.value.clone()),
                    path_chunks: None,
                    path_edges: None,
                    hops: None,
                });
                out.similar_to_query = Some(clause.value.clone());
            }
            _ => {}
        }
    }

    out
}

/// Port of `executeSearch` (`service.ts:77-255`). Infallible: see the
/// module doc for why this returns `SearchResult` directly rather than an
/// `AppResult`.
pub async fn execute_search(pool: &PgPool, user_id: &str, query: &SearchQueryBody) -> SearchResult {
    let graph = resolve_graph_clauses(pool, &query.clauses).await;

    // Matches Node's `if (graphIds !== undefined && graphIds.length === 0)
    // return { chunks: [], total: 0, graphMeta }` (`service.ts:130-132`):
    // a graph clause that resolved to nothing short-circuits the whole
    // query, but `graphMeta` still comes back — it's set the moment a
    // graph clause runs, not the moment it finds something.
    if let Some(ids) = &graph.ids
        && ids.is_empty()
    {
        return SearchResult {
            chunks: vec![],
            total: 0,
            graph_meta: graph.meta,
            duplicate_hints: None,
        };
    }

    let mut list_params = build_list_params(&query.clauses, query);
    // Resolved graph ids become an ordinary filter, ANDed with
    // `chunk::list`/`chunk::count`'s own mandatory `user_id = ..`
    // predicate — see the module doc on why this can never leak another
    // user's chunk even though AGE itself knows nothing about ownership.
    if let Some(ids) = &graph.ids {
        list_params.ids = Some(ids.clone());
    }

    let (chunks, total_count) = match list_and_count(pool, user_id, &list_params).await {
        Ok(pair) => pair,
        Err(_) => {
            return SearchResult {
                chunks: vec![],
                total: 0,
                graph_meta: graph.meta,
                duplicate_hints: None,
            };
        }
    };

    // Matches Node's `if (filteredChunks.length === 0) return { chunks: [],
    // total: 0, graphMeta }` (`service.ts:160-162`): an empty *page* forces
    // `total: 0` in the response too, even when `chunk::count` found a
    // nonzero total (e.g. an `offset` past the end of the result set). This
    // looks like it should report the real total; it is Node's behaviour.
    if chunks.is_empty() {
        return SearchResult {
            chunks: vec![],
            total: 0,
            graph_meta: graph.meta,
            duplicate_hints: None,
        };
    }

    let chunk_ids: Vec<String> = chunks.iter().map(|c| c.id.clone()).collect();

    let tag_rows = tag::tags_for_chunks(pool, user_id, &chunk_ids)
        .await
        .unwrap_or_default();
    let mut tags_by_chunk: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in tag_rows {
        tags_by_chunk
            .entry(row.chunk_id)
            .or_default()
            .push(row.tag_name);
    }

    let conn_rows = connection::count_for_chunks(pool, &chunk_ids)
        .await
        .unwrap_or_default();
    let conn_by_chunk: std::collections::HashMap<String, i64> = conn_rows
        .into_iter()
        .map(|r| (r.chunk_id, r.count))
        .collect();

    // Port of Node's `graphContextMap` construction (`service.ts:190-212`):
    // built once per graph-clause type, keyed by chunk id, then looked up
    // per result chunk below.
    let mut graph_context_map: std::collections::HashMap<String, GraphContext> =
        std::collections::HashMap::new();
    if let (Some(ids), Some(meta)) = (&graph.ids, &graph.meta) {
        match meta.meta_type.as_str() {
            "path" => {
                if let Some(path_chunks) = &graph.path_chunks {
                    for (idx, id) in path_chunks.iter().enumerate() {
                        graph_context_map.insert(
                            id.clone(),
                            GraphContext {
                                hop_distance: None,
                                path_position: Some(idx as i64),
                                matched_requirement: None,
                            },
                        );
                    }
                }
            }
            "neighborhood" => {
                for id in ids {
                    graph_context_map.insert(
                        id.clone(),
                        GraphContext {
                            hop_distance: graph.near_hops,
                            path_position: None,
                            matched_requirement: None,
                        },
                    );
                }
            }
            "requirement-reach" => {
                for id in ids {
                    graph_context_map.insert(
                        id.clone(),
                        GraphContext {
                            hop_distance: None,
                            path_position: None,
                            matched_requirement: None,
                        },
                    );
                }
            }
            "semantic" => {
                if let Some(query_text) = &graph.similar_to_query {
                    for id in ids {
                        graph_context_map.insert(
                            id.clone(),
                            GraphContext {
                                hop_distance: None,
                                path_position: None,
                                matched_requirement: Some(format!("similar to \"{query_text}\"")),
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    let result_chunks: Vec<SearchResultChunk> = chunks
        .into_iter()
        .map(|c| {
            let connection_count = conn_by_chunk.get(&c.id).copied().unwrap_or(0);
            let graph_context = graph_context_map.remove(&c.id);
            // Every field besides `content`/`summary`/`connectionCount` is
            // hardcoded in Node's call site too (`service.ts:216-229`):
            // `rationale`/`alternatives`/`consequences: null`,
            // `centralityDegree: 0`, `hasEmbedding: false`,
            // `requirementCount: 0`, `allRequirementsPassing: false`,
            // `referencedInSession: false`. Search results never carry the
            // real embedding/requirement-coverage signal that the chunk
            // detail page's health score eventually will.
            let health = health_score::compute_health_score(&ChunkHealthInput {
                content: &c.content,
                summary: c.summary.as_deref(),
                rationale: None,
                alternatives: None,
                consequences: None,
                connection_count,
                centrality_degree: 0,
                has_embedding: false,
                requirement_count: 0,
                all_requirements_passing: false,
                referenced_in_session: false,
            });
            SearchResultChunk {
                id: c.id.clone(),
                title: c.title,
                chunk_type: c.chunk_type,
                summary: c.summary,
                tags: tags_by_chunk.remove(&c.id).unwrap_or_default(),
                connection_count,
                updated_at: c.updated_at,
                graph_context,
                health_score: health.total,
            }
        })
        .collect();

    // Node: `const total = graphIds !== undefined ? chunks.length :
    // result.total;` (`service.ts:243`) — when a graph clause was present,
    // the reported total is the final filtered page length, not the SQL
    // count (which would already agree here, since the ids filter is
    // applied inside the same scoped query, but this mirrors Node's rule
    // directly rather than relying on that agreement).
    let total = if graph.ids.is_some() {
        result_chunks.len() as i64
    } else {
        total_count
    };

    SearchResult {
        chunks: result_chunks,
        total,
        graph_meta: graph.meta,
        duplicate_hints: None,
    }
}

/// Port of `autocomplete` (`service.ts:257-283`). Infallible — see module
/// doc. `field` outside `{tag, chunk, requirement}` returns `[]`, matching
/// Node's fallthrough `return [];`.
pub async fn autocomplete(pool: &PgPool, user_id: &str, field: &str, prefix: &str) -> Vec<String> {
    match field {
        "tag" => {
            let tags = match tag::list(pool, user_id).await {
                Ok(t) => t,
                Err(_) => return vec![],
            };
            let lower = prefix.to_lowercase();
            tags.into_iter()
                .filter(|t| t.name.to_lowercase().starts_with(&lower))
                .take(10)
                .map(|t| t.name)
                .collect()
        }
        "chunk" => match chunk::search_titles(pool, user_id, prefix, 10).await {
            Ok(rows) => rows.into_iter().map(|r| r.title).collect(),
            Err(_) => vec![],
        },
        "requirement" => match requirement::search_titles(pool, user_id, prefix, 10).await {
            Ok(rows) => rows.into_iter().map(|r| r.title).collect(),
            Err(_) => vec![],
        },
        _ => vec![],
    }
}

/// Thin wrapper over `saved_query::list` — can fail (unlike
/// `execute_search`/`autocomplete`), matching Node's route, which lets
/// `listSavedQueries` propagate rather than wrapping it in `orElse`.
pub async fn list_saved(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<saved_query::SavedQuery>> {
    saved_query::list(pool, user_id, space_id).await
}

/// Thin wrapper over `saved_query::create`. `body.query` is re-serialised
/// to `serde_json::Value` for storage — validated on the way in by
/// `SavedQueryPayload`'s own `Deserialize`, opaque from here on (see
/// `saved_query`'s module doc).
pub async fn create_saved(
    pool: &PgPool,
    user_id: &str,
    body: CreateSavedQueryBody,
) -> AppResult<saved_query::SavedQuery> {
    let query_json = serde_json::to_value(&body.query).unwrap_or(serde_json::Value::Null);
    saved_query::create(
        pool,
        user_id,
        &body.name,
        query_json,
        body.space_id.as_deref(),
    )
    .await
}

/// Thin wrapper over `saved_query::delete`. See that function's doc
/// comment for why this returns `AppResult<()>`, not a `bool`: the route
/// (`routes.rs::delete_saved`) never surfaces a 404 either way, matching
/// Node's `deleteSavedQuery` result being discarded at the route
/// (`packages/api/src/search/routes.ts:105-112`).
pub async fn delete_saved(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    saved_query::delete(pool, user_id, id).await
}
