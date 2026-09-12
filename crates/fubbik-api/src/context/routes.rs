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

    let ids = resolve_for_files(
        &state.pool,
        &state.ai,
        &state.background,
        &user.id,
        &paths,
        query.space_id.as_deref(),
    )
    .await?;
    let chunks = enrich_chunks(&state.pool, &user.id, &ids).await?;
    let structured = budget_and_format(chunks, max_tokens);
    Ok(axum::Json(ContextResponse::from_structured(
        structured, format,
    )))
}

/// Shared resolve->enrich tail: budgets the enriched chunks into
/// `max_tokens`, then groups the survivors into sections. See
/// `budget_metadata`'s doc comment for the ordering contract this delegates
/// to.
pub(crate) fn budget_and_format(
    chunks: Vec<fubbik_core::format::ChunkWithMetadata>,
    max_tokens: usize,
) -> fubbik_core::format::StructuredContext {
    format_structured(budget_metadata(chunks, max_tokens))
}

/// The order-preserving resolve->budget tail shared by `budget_and_format`
/// (which formats the survivors into sections) and
/// `snapshot::create_snapshot` (which freezes the survivors verbatim into
/// `context_snapshot.chunks` instead of formatting them). Extracted out of
/// `budget_and_format` rather than duplicated so the ordering contract below
/// has exactly one implementation for both callers to share.
///
/// **Output order is `budget_chunks`' score-descending order, not
/// `chunks`' input order.** Node's `budgetChunks` (`utils.ts:63-76`) sorts a
/// clone of its input by score descending and pushes survivors into
/// `selected` while walking that sorted copy — so its return value is
/// score-descending, and `formatStructured(budgeted)` derives both section
/// order and within-section order from exactly that. This port must match:
/// `budget_chunks` operates on bare `ScoredChunk`s while
/// `ChunkWithMetadata` carries more than `budget_chunks` (Task 4b/5) knows
/// about, so budgeting here strips the metadata, budgets, then re-attaches
/// it by id — but the id *order* returned by `budget_chunks` must drive the
/// final `Vec`, not the id order of `chunks`. Concretely: collect
/// `budget_chunks`' output ids into a `Vec` (preserving its score-descending
/// order), index the original `chunks` into a `HashMap<String,
/// ChunkWithMetadata>` (indexed into by id, never iterated — so its
/// unordered iteration can't leak into the result), then walk the id `Vec`
/// removing each match from the map. Tie-breaking is still deterministic:
/// `budget_chunks`' `sort_by` is a stable sort over whatever order it's
/// given, and it's given `chunks` in enrichment order, so ties still break
/// by enrichment order — `budget_and_format_preserves_enrichment_order_among_tied_scores`
/// (below) covers exactly this and needs no change. What changes is chunks
/// that are *not* tied: those must come back sorted by score, highest
/// first, which is what a relevance-ordered export means.
pub(crate) fn budget_metadata(
    chunks: Vec<fubbik_core::format::ChunkWithMetadata>,
    max_tokens: usize,
) -> Vec<fubbik_core::format::ChunkWithMetadata> {
    use std::collections::HashMap;

    let scored: Vec<_> = chunks.iter().map(|c| c.chunk.clone()).collect();

    let order: Vec<String> = budget_chunks(scored, max_tokens)
        .into_iter()
        .map(|c| c.id)
        .collect();

    let mut by_id: HashMap<String, fubbik_core::format::ChunkWithMetadata> = chunks
        .into_iter()
        .map(|c| (c.chunk.id.clone(), c))
        .collect();

    order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/context/for-plan", get(for_plan))
        .route("/api/context/about", get(about))
        .route("/api/context/for-files", get(for_files))
        .merge(super::snapshot::router())
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage for `budget_and_format`'s ordering contract,
    //! deliberately NOT routed through an HTTP call.
    //!
    //! All three resolvers (`resolve_for_plan`/`resolve_for_files`/
    //! `resolve_for_concept`) now push into an order-preserving
    //! `push_unique` collector rather than a `HashSet`, so their output
    //! order is deterministic. Testing `budget_and_format` directly, with a
    //! hand-ordered `Vec<ChunkWithMetadata>` standing in for
    //! `enrich_chunks`' output, isolates exactly the seam these findings are
    //! about (tie-breaking order, and score-descending order for non-tied
    //! chunks) and gives a deterministic, reproducible assertion without
    //! depending on the resolvers or the database at all.
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

    fn scored_chunk(id: &str, score: f64) -> ChunkWithMetadata {
        ChunkWithMetadata {
            chunk: ScoredChunk {
                id: id.to_string(),
                title: format!("Title {id}"),
                content: "short body".to_string(),
                chunk_type: "note".to_string(),
                rationale: None,
                tags: vec![],
                score,
            },
            health_score: 50,
            is_stale: false,
            has_pending_proposal: false,
        }
    }

    /// Finding 1: `budget_and_format`'s output must be score-descending,
    /// matching Node's `budgetChunks` (`utils.ts:63-76`), which pushes into
    /// `selected` while walking a `sorted` (score-descending) copy of its
    /// input. Here the enrichment order (`c0, c1, c2, c3`) is deliberately
    /// the *reverse* of score order, so a bug that returns enrichment order
    /// instead of score order cannot pass by coincidence.
    #[test]
    fn budget_and_format_orders_survivors_by_score_descending() {
        let input = vec![
            scored_chunk("c0", 1.0),
            scored_chunk("c1", 2.0),
            scored_chunk("c2", 3.0),
            scored_chunk("c3", 4.0),
        ];

        let structured = budget_and_format(input, 100_000);

        let all_chunks: Vec<&ChunkWithMetadata> = structured
            .sections
            .iter()
            .flat_map(|s| s.chunks.iter())
            .collect();

        assert_eq!(
            all_chunks.len(),
            4,
            "a 100,000-token budget must admit every chunk, not drop any"
        );

        let returned_order: Vec<&str> = all_chunks.iter().map(|c| c.chunk.id.as_str()).collect();
        assert_eq!(
            returned_order,
            vec!["c3", "c2", "c1", "c0"],
            "budget_and_format must return survivors in score-descending order, \
             matching Node's budgetChunks output order — not enrichment/input order"
        );
    }
}
