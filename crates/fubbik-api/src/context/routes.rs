//! `GET /api/context/for-plan`, `/api/context/about`, `/api/context/for-files`.
//!
//! Ports `packages/api/src/context/routes.ts`. All three handlers share one
//! shape — resolve candidate ids, enrich them into scored/health-annotated
//! chunks, budget them into a token limit, then format the result —
//! differing only in which resolver runs and which query parameters feed
//! it. `maxTokens` stays string-typed on the wire (parsed here rather than
//! declared as a number), matching Node's `t.Optional(t.String())` schema.

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use fubbik_core::format::format_structured;
use fubbik_core::score::budget_chunks;

use super::dto::{AboutQuery, ContextResponse, ForFilesQuery, ForPlanQuery, parse_max_tokens};
use super::resolvers::{resolve_for_concept, resolve_for_files, resolve_for_plan};
use super::service::enrich_chunks;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::Query;
use fubbik_core::error::AppError;

/// `GET /api/context/for-plan?planId=X&maxTokens=N&format=structured-md`.
///
/// `resolve_for_plan` returns `NotFound` for a plan the caller doesn't own
/// (a deliberate tightening over Node — see the resolver's own doc comment
/// for the full ruling); this handler just propagates it via `?`, it does
/// not add a second check.
#[utoipa::path(get, path = "/api/context/for-plan", params(ForPlanQuery),
    responses((status = 200, body = ContextResponse), (status = 404)))]
pub async fn for_plan(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ForPlanQuery>,
) -> ApiResult<axum::Json<ContextResponse>> {
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let format = query.format.unwrap_or_default();

    let ids = resolve_for_plan(&state.pool, &user.id, &query.plan_id).await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// `GET /api/context/about?q=auth&maxTokens=N&spaceId=X&format=structured-md`.
#[utoipa::path(get, path = "/api/context/about", params(AboutQuery),
    responses((status = 200, body = ContextResponse), (status = 400)))]
pub async fn about(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<AboutQuery>,
) -> ApiResult<axum::Json<ContextResponse>> {
    // Matches Node's `if (!ctx.query.q)` exactly (`context/routes.ts:62-64`):
    // JS falsy-checks a string, so only an exactly-empty `q=` 400s. A
    // whitespace-only `q` (e.g. `q=%20`) is truthy in JS and reaches the
    // resolver unchanged — `.trim().is_empty()` here would reject a request
    // Node accepts, so this deliberately checks emptiness, not blankness.
    if query.q.is_empty() {
        return Err(ApiError::from(AppError::Validation("q is required".into())));
    }
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let format = query.format.unwrap_or_default();

    let ids = resolve_for_concept(
        &state.pool,
        &state.ai,
        &user.id,
        &query.q,
        query.space_id.as_deref(),
    )
    .await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// `GET /api/context/for-files?paths=a.ts,b.ts&maxTokens=N&spaceId=X&format=structured-md`.
#[utoipa::path(get, path = "/api/context/for-files", params(ForFilesQuery),
    responses((status = 200, body = ContextResponse), (status = 400)))]
pub async fn for_files(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ForFilesQuery>,
) -> ApiResult<axum::Json<ContextResponse>> {
    let paths: Vec<String> = query
        .paths
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if paths.is_empty() {
        return Err(ApiError::from(AppError::Validation(
            "paths must contain at least one path".into(),
        )));
    }
    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let format = query.format.unwrap_or_default();

    let ids = resolve_for_files(&state.pool, &user.id, &paths, query.space_id.as_deref()).await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// Shared resolve->enrich tail: budgets the enriched chunks into
/// `max_tokens`, then groups the survivors into sections. `budget_chunks`
/// operates on bare `ScoredChunk`s and Node's own `budgetChunks` takes and
/// returns the full `ChunkWithMetadata[]` (`utils.ts:39`) — this port's
/// `budget_chunks` signature (Task 4b/5) only knows about `ScoredChunk`, so
/// budgeting here strips the metadata, budgets, then re-attaches it by id.
///
/// **Must preserve `chunks`' original order end to end.** An earlier draft
/// routed the re-pairing through a `HashMap<String, ChunkWithMetadata>` and
/// iterated `.values()` to build the `Vec<ScoredChunk>` fed to
/// `budget_chunks`. `HashMap` iteration order is not stable across
/// constructions, so on a tie between two chunks' scores — `budget_chunks`'s
/// `sort_by` is stable, but stability only preserves whatever order it's
/// given — which chunk lands on the budget boundary could differ between
/// two calls of the very same request. Node's array preserves
/// `enrichChunks`' order deterministically, so ties there always break the
/// same way. This version never moves `chunks` into a map: it clones the
/// `ScoredChunk` half (in `chunks`' order) for `budget_chunks` to sort and
/// trim, collects the *surviving ids* into a `HashSet` (membership only,
/// order-independent by construction), then filters the original `chunks`
/// `Vec` by that set — so the final order is exactly enrichment order,
/// every time, regardless of how many chunks tie on score.
fn budget_and_format(
    chunks: Vec<fubbik_core::format::ChunkWithMetadata>,
    max_tokens: usize,
) -> fubbik_core::format::StructuredContext {
    use std::collections::HashSet;

    let scored: Vec<_> = chunks.iter().map(|c| c.chunk.clone()).collect();

    let budgeted_ids: HashSet<String> = budget_chunks(scored, max_tokens)
        .into_iter()
        .map(|c| c.id)
        .collect();

    let budgeted: Vec<fubbik_core::format::ChunkWithMetadata> = chunks
        .into_iter()
        .filter(|c| budgeted_ids.contains(&c.chunk.id))
        .collect();

    format_structured(budgeted)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/context/for-plan", get(for_plan))
        .route("/api/context/about", get(about))
        .route("/api/context/for-files", get(for_files))
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage for `budget_and_format`'s order-preservation
    //! contract, deliberately NOT routed through an HTTP call.
    //!
    //! All three resolvers (`resolve_for_plan`/`resolve_for_files`/
    //! `resolve_for_concept`) collect candidate ids into a `HashSet<String>`
    //! and return `ids.into_iter().collect()` — confirmed experimentally
    //! (see the task report) to already randomize order on every single
    //! call, independent of anything in this file. An HTTP-level "repeated
    //! calls return the same order" test would therefore be testing two
    //! stacked sources of nondeterminism at once and could fail even on a
    //! correctly fixed `budget_and_format`, or pass by coincidence on a
    //! broken one. Testing `budget_and_format` directly, with a
    //! hand-ordered `Vec<ChunkWithMetadata>` standing in for
    //! `enrich_chunks`' output, isolates exactly the seam Finding 1 was
    //! about and gives a deterministic, reproducible assertion.
    use fubbik_core::format::ChunkWithMetadata;
    use fubbik_core::score::ScoredChunk;

    use super::budget_and_format;

    fn tied_chunk(id: &str) -> ChunkWithMetadata {
        ChunkWithMetadata {
            chunk: ScoredChunk {
                id: id.to_string(),
                title: format!("Title {id}"),
                content: "short body".to_string(),
                chunk_type: "note".to_string(),
                rationale: None,
                tags: vec![],
                // Identical score for every chunk: this is the exact
                // condition under which the old `HashMap`-based re-pairing
                // could reorder the result relative to enrichment order.
                score: 5.0,
            },
            health_score: 50,
            is_stale: false,
            has_pending_proposal: false,
        }
    }

    /// Six chunks, all tied on score, all small enough that a generous
    /// budget admits every one of them (so this test is purely about
    /// order, not about which chunks survive budgeting — that's already
    /// covered by `for_plan_returns_the_plans_chunks_within_budget` in
    /// `tests/context.rs`). The output order must equal the input order
    /// exactly; six ties make a coincidental match after re-scrambling
    /// through a `HashMap` a 1-in-720 event, not something that could pass
    /// by chance on a rerun of the old, buggy implementation.
    #[test]
    fn budget_and_format_preserves_enrichment_order_among_tied_scores() {
        let ids = ["c0", "c1", "c2", "c3", "c4", "c5"];
        let input: Vec<ChunkWithMetadata> = ids.iter().map(|id| tied_chunk(id)).collect();

        let structured = budget_and_format(input, 100_000);

        let all_chunks: Vec<&ChunkWithMetadata> = structured
            .sections
            .iter()
            .flat_map(|s| s.chunks.iter())
            .collect();

        assert_eq!(
            all_chunks.len(),
            ids.len(),
            "a 100,000-token budget must admit every tied chunk, not drop any"
        );

        let returned_order: Vec<&str> = all_chunks.iter().map(|c| c.chunk.id.as_str()).collect();
        assert_eq!(
            returned_order,
            ids.to_vec(),
            "budget_and_format must preserve enrichment order among chunks tied on score, \
             not scramble it through an intermediate HashMap"
        );
    }
}
