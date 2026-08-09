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
//! ## The Task 9 seam
//!
//! `near`, `path`, `affected-by`, and `similar-to` clauses need Apache AGE
//! (graph queries) or pgvector-embedding semantic search, neither of which
//! is wired into this port yet. [`is_graph_clause`] detects them; when any
//! are present, [`execute_search`] returns `{chunks: [], total: 0}`
//! immediately — the same result Node produces when its graph resolver
//! comes back with zero ids (`service.ts:130-132`: `if (graphIds !==
//! undefined && graphIds.length === 0) return { chunks: [], total: 0,
//! graphMeta }`). This is a deliberately honest "not implemented yet", not
//! a pretend implementation: it does not attempt neighborhood/path/
//! semantic resolution, does not populate `graphMeta`, and does not
//! silently fall back to treating the clause as a standard filter. Task 9
//! replaces this early return with real AGE-backed resolution.

use fubbik_core::error::AppResult;
use fubbik_db::repo::{chunk, connection, requirement, saved_query, tag};
use sqlx::PgPool;

use super::dto::{CreateSavedQueryBody, SearchQueryBody, SearchResult, SearchResultChunk};
use super::parser::QueryClause;
use crate::chunks::health_score::{self, ChunkHealthInput};

const GRAPH_FIELDS: [&str; 4] = ["near", "path", "affected-by", "similar-to"];

/// See the module doc's "Task 9 seam" section.
fn is_graph_clause(clause: &QueryClause) -> bool {
    GRAPH_FIELDS.contains(&clause.field.as_str())
}

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

/// Port of `executeSearch` (`service.ts:77-255`), minus the graph-clause
/// branches — see the module doc's "Task 9 seam" section. Infallible: see
/// the module doc for why this returns `SearchResult` directly rather than
/// an `AppResult`.
pub async fn execute_search(pool: &PgPool, user_id: &str, query: &SearchQueryBody) -> SearchResult {
    if query.clauses.iter().any(is_graph_clause) {
        return SearchResult::default();
    }

    let list_params = build_list_params(&query.clauses, query);

    let (chunks, total) = match list_and_count(pool, user_id, &list_params).await {
        Ok(pair) => pair,
        Err(_) => return SearchResult::default(),
    };

    // Matches Node's `if (filteredChunks.length === 0) return { chunks: [],
    // total: 0, graphMeta }` (`service.ts:160-162`): an empty *page* forces
    // `total: 0` in the response too, even when `chunk::count` found a
    // nonzero total (e.g. an `offset` past the end of the result set). This
    // looks like it should report the real total; it is Node's behaviour.
    if chunks.is_empty() {
        return SearchResult::default();
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

    let result_chunks: Vec<SearchResultChunk> = chunks
        .into_iter()
        .map(|c| {
            let connection_count = conn_by_chunk.get(&c.id).copied().unwrap_or(0);
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
                graph_context: None,
                health_score: health.total,
            }
        })
        .collect();

    SearchResult {
        chunks: result_chunks,
        total,
        graph_meta: None,
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
