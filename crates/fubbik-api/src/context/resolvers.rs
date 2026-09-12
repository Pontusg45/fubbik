//! Ports `packages/api/src/context/resolvers.ts`'s three input resolvers
//! (`resolveForPlan`, `resolveForConcept`, `resolveForFiles`). Each
//! produces **candidate chunk ids only** — enrichment
//! (`super::service::enrich_chunks`) is a separate step, which is what
//! lets all three input sources share one pipeline instead of three.
//!
//! None of the three ever fails in Node — every sub-effect is wrapped in
//! `Effect.catchAll(() => Effect.succeed([]))`, so the resolver's error
//! channel is literally `never`. `resolve_for_concept` and
//! `resolve_for_files` reproduce that: every fallible step degrades to "no
//! contribution from this step" rather than failing the whole call.
//! `resolve_for_plan` is the one exception — see its own doc comment.

use std::collections::HashSet;

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{chunk, plan, requirement, semantic};
use sqlx::PgPool;

/// Appends `id` to `ids` the first time it's seen, using `seen` purely for
/// O(1) membership tracking — `seen` is never iterated, only indexed into,
/// so its `HashSet` randomized iteration order never leaks into the
/// result. `ids`' order is exactly first-encounter order across however
/// many sources call this in sequence.
///
/// All three resolvers below need this: Node's equivalent dedup is
/// `new Set<string>()` + `[...ids]`, and JS `Set` iterates in insertion
/// order by spec, so Node returns a deterministic sequence following the
/// order its sources are queried. A bare `HashSet<String>` collected with
/// `.into_iter().collect()` does not have that property — its default
/// hasher is randomly seeded per process, so the same insertions produce a
/// different order on nearly every construction (confirmed experimentally
/// before this fix: five inserts into a fresh `HashSet`, printed across
/// five constructions in one process, gave five different orders). This
/// mirrors `fubbik_core::format::format_structured`'s `Vec`-plus-
/// membership-set shape, reviewed and approved in Task 4 for the identical
/// reason (JS `Map` iteration is also insertion-ordered) — one pattern for
/// "preserve first-encounter order," not two.
fn push_unique(id: String, ids: &mut Vec<String>, seen: &mut HashSet<String>) {
    if seen.insert(id.clone()) {
        ids.push(id);
    }
}

// ---------------------------------------------------------------------------
// resolve_for_plan
// ---------------------------------------------------------------------------

/// Ports `resolveForPlan` (`resolvers.ts:139-177`): collects chunk ids from
/// a plan's `chunk`-kind analyze items, its linked requirements' chunks,
/// and its tasks' linked chunks.
///
/// **Divergence from Node, by deliberate ruling — read before "restoring
/// parity".** Node's `resolveForPlan(planId)` takes no `user_id` at all and
/// performs no ownership check: it queries `plan_analyze_item`/
/// `plan_requirement`/`plan_task` straight from `planId`
/// (`resolvers.ts:139-177`), relying entirely on `enrichChunks(ids,
/// userId)`'s per-chunk `getChunkById(id, userId)` filter three layers
/// downstream (`context/routes.ts:26-28`) to keep a cross-user request from
/// ever seeing another user's *chunk*. Nothing stops it from seeing another
/// user's *plan structure* — task titles, analyze-item text, which
/// requirements are linked — on the way there.
///
/// This port checks plan ownership up front instead, via `plan::find_by_id`
/// (the same guard `plans::service::get_plan` uses, `plans/service.rs:76`),
/// returning `NotFound` for a plan that isn't `user_id`'s. This mirrors
/// `fubbik_db::repo::plan`'s own module doc (divergence #13): "Every
/// function below takes `user_id` and filters on it in SQL — never left to
/// a caller-side check a future refactor could accidentally skip." Every
/// repo call this function makes (`list_analyze_items`, `list_requirements`,
/// `list_tasks`, `list_task_chunks_with_titles`, `requirement::get_chunks`)
/// already carries that same-shaped `EXISTS` ownership guard in SQL, so the
/// `find_by_id` check below is *also* redundant with each of them
/// individually — deliberately: this is defence in depth, not the only
/// layer. The chunk-level filter in `service::enrich_chunks` stays too.
/// **Do not delete either layer to "match `resolvers.ts`" — that is the
/// divergence, not a bug.**
///
/// **Second, separate divergence: this function propagates repository
/// errors with `?` instead of swallowing them.** The module doc above
/// states Node's resolver error channel is `never` — every sub-effect
/// wrapped in `Effect.catchAll(() => Effect.succeed([]))` — and that
/// `resolve_for_concept`/`resolve_for_files` reproduce that. This function
/// does not: `plan::find_by_id` and every `plan::list_*`/
/// `requirement::get_chunks` call below is propagated via `?`, so a
/// transient `DatabaseError` on any one of them fails the whole plan
/// lookup rather than silently contributing zero ids for that step. This
/// is deliberate, not an oversight — for a plan-scoped lookup, "the plan
/// has fewer chunks than it should because one query failed" is a worse
/// failure mode to hand back silently than a 500: the caller asked for
/// *this specific plan's* context, and a partial, uncommunicated result
/// would be actively misleading in a way that "no results for a fuzzy
/// concept search" is not.
pub async fn resolve_for_plan(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<String>> {
    plan::find_by_id(pool, user_id, plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))?;

    let mut ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1. plan_analyze_item where kind = "chunk"
    let analyze_items = plan::list_analyze_items(pool, user_id, plan_id).await?;
    for item in analyze_items {
        if item.kind == "chunk"
            && let Some(chunk_id) = item.chunk_id
        {
            push_unique(chunk_id, &mut ids, &mut seen);
        }
    }

    // 2. plan_requirement -> requirement_chunk
    let plan_reqs = plan::list_requirements(pool, user_id, plan_id).await?;
    for pr in plan_reqs {
        let chunks = requirement::get_chunks(pool, user_id, &pr.requirement_id).await?;
        for c in chunks {
            push_unique(c.id, &mut ids, &mut seen);
        }
    }

    // 3. plan_task -> plan_task_chunk
    let tasks = plan::list_tasks(pool, user_id, plan_id).await?;
    for t in tasks {
        let task_chunks = plan::list_task_chunks_with_titles(pool, user_id, plan_id, &t.id).await?;
        for tc in task_chunks {
            push_unique(tc.chunk_id, &mut ids, &mut seen);
        }
    }

    Ok(ids)
}

// ---------------------------------------------------------------------------
// resolve_for_concept
// ---------------------------------------------------------------------------

/// Ports `resolveForConcept` (`resolvers.ts:183-210`): semantic search plus
/// a text search, unioned. Never fails — see the module doc.
pub async fn resolve_for_concept(
    pool: &PgPool,
    ai: &fubbik_ai::OllamaClient,
    user_id: &str,
    query: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<String>> {
    let mut ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Semantic search — requires Ollama; falls back silently if the
    // embedding call or the search itself fails, matching Node's
    // `.pipe(Effect.catchAll(() => Effect.succeed([])))` at both steps.
    if let Ok(embedding) = ai.embed_query(query).await
        && let Ok(hits) =
            semantic::semantic_search(pool, &embedding, Some(user_id), &[], None, 20).await
    {
        for h in hits {
            push_unique(h.id, &mut ids, &mut seen);
        }
    }

    // Text search over title/content.
    let params = chunk::ListParams {
        search: Some(query.to_string()),
        space_id: space_id.map(str::to_string),
        limit: 20,
        offset: 0,
        ..Default::default()
    };
    if let Ok(rows) = chunk::list(pool, user_id, &params).await {
        for c in rows {
            push_unique(c.id, &mut ids, &mut seen);
        }
    }

    Ok(ids)
}

// ---------------------------------------------------------------------------
// resolve_for_files
// ---------------------------------------------------------------------------

/// Ports `resolveForFiles` (`resolvers.ts:216-235`): for each path,
/// delegates to the full five-strategy `getContextForFile` — file-ref
/// (+20), applies-to glob (+10), dependency (+3), semantic (+5, needs
/// Ollama), connected (+2) — and unions the matched chunk ids, exactly as
/// Node does (`resolvers.ts:220-224`).
///
/// **This function used to implement only two of the five strategies
/// (file-ref, applies-to) directly**, because `get_context_for_file` did
/// not exist yet at the time it was ported (Task 5, ahead of Task 7's
/// `context_for_file::service`). That was a deliberate, documented scope
/// narrowing at the time — see git history for the original doc comment —
/// but it meant this function silently under-returned against Node for any
/// path whose only matching chunks came from the dependency, semantic, or
/// connected strategies. Task 7 rewires it to delegate to the real
/// `get_context_for_file`, closing that gap.
///
/// `deps` is never passed (`None`) on this path, matching Node's own call
/// site exactly (`getContextForFile(userId, path, spaceId)` — no fourth
/// argument, `resolvers.ts:222`); only the `json-legacy` HTTP handler
/// threads a caller-supplied `deps` list through to `get_context_for_file`
/// directly.
///
/// Runs the per-path calls at concurrency 5, matching Node's
/// `Effect.all(paths.map(...), { concurrency: 5 })`
/// (`resolvers.ts:220`) — and, just as importantly, **preserves path
/// order** in the result the same way `Effect.all` does: results are
/// collected by original index, not by completion order, before their
/// chunk ids are folded into `ids` via `push_unique`. A `JoinSet`'s
/// completion order is not the input order, so completion-order folding
/// would reintroduce exactly the nondeterminism this module's other two
/// resolvers were fixed to avoid (see `push_unique`'s doc comment).
///
/// Never fails — see the module doc. A path whose `get_context_for_file`
/// call somehow does return an error contributes no chunks (Node's
/// `Effect.catchAll(() => Effect.succeed({chunks: [], requirements: []}))`,
/// `resolvers.ts:221`) rather than failing the whole request.
pub async fn resolve_for_files(
    pool: &PgPool,
    ai: &fubbik_ai::OllamaClient,
    background: &crate::background::BackgroundRuntime,
    user_id: &str,
    paths: &[String],
    space_id: Option<&str>,
) -> AppResult<Vec<String>> {
    let mut ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
    let mut tasks = tokio::task::JoinSet::new();
    for (idx, path) in paths.iter().enumerate() {
        let pool = pool.clone();
        let ai = ai.clone();
        let background = background.clone();
        let user_id = user_id.to_string();
        let path = path.clone();
        let space_id = space_id.map(str::to_string);
        let permits = permits.clone();
        tasks.spawn(async move {
            let _permit = permits.acquire().await.expect("semaphore never closed");
            let result = crate::context_for_file::service::get_context_for_file(
                &pool,
                &ai,
                &background,
                &user_id,
                &path,
                space_id.as_deref(),
                None,
            )
            .await
            .unwrap_or_else(|_| crate::context_for_file::dto::FileContext {
                chunks: vec![],
                requirements: vec![],
            });
            (idx, result)
        });
    }

    let mut by_index: Vec<Option<crate::context_for_file::dto::FileContext>> =
        (0..paths.len()).map(|_| None).collect();
    while let Some(joined) = tasks.join_next().await {
        let (idx, file_context) = joined.expect("get_context_for_file task should not panic");
        by_index[idx] = Some(file_context);
    }

    for file_context in by_index.into_iter().flatten() {
        for c in file_context.chunks {
            push_unique(c.id, &mut ids, &mut seen);
        }
    }

    Ok(ids)
}
