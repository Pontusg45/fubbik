//! `GET /api/context/for-file`. Ports
//! `packages/api/src/context-for-file/routes.ts`.
//!
//! Three `format` values, two different pipelines:
//!
//! - `json-legacy` calls [`service::get_context_for_file`] directly and
//!   returns its raw `{chunks, requirements}` shape — each chunk's score
//!   carries exactly ONE strategy's bonus (first-strategy-wins: once a
//!   chunk id is found, later strategies skip it entirely rather than
//!   adding a second bonus — see `service`'s module doc) plus its
//!   `matchReason`. It never fetches governing behaviours (matches Node:
//!   the `json-legacy` branch returns before `getBehaviorsForCodePath` is
//!   even called, `context-for-file/routes.ts:22-29`).
//! - `structured-md`/`structured-json` (default) instead go through
//!   `resolve_for_files` -> `enrich_chunks` -> budget -> format, the same
//!   pipeline every other `/api/context/*` route uses. This means the
//!   "structured" formats do NOT expose strategy bonuses or match reasons
//!   at all — a chunk's score there is purely `context::service`'s
//!   health-based `score_chunk`, matching Node's `context-for-file/
//!   routes.ts:39-46` exactly (only `json-legacy` reaches
//!   `getContextForFile`'s own `score`/`matchReason` fields). This branch
//!   ALSO fetches governing behaviours and folds them in — `structured-json`
//!   gains a `behaviors` array, `structured-md` appends a rendered section.

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use fubbik_core::format::{
    GoverningBehavior, format_behaviors_markdown, format_structured_markdown,
};

use super::dto::{
    ForFileFormat, ForFileQuery, ForFileResponse, ForFileStructuredResponse, parse_deps,
};
use super::service::get_context_for_file;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::context::dto::parse_max_tokens;
use crate::context::resolvers::resolve_for_files;
use crate::context::routes::budget_and_format;
use crate::context::service::enrich_chunks;
use crate::error::ApiResult;
use crate::extract::Query;

/// Maps the repo's reverse-lookup row to the wire/formatter shape. A plain
/// field-by-field copy, not a `From` impl: `fubbik_core` cannot depend on
/// `fubbik_db` (`fubbik_db` already depends on `fubbik_core`, the other
/// direction would be a cycle), so neither crate can own a conversion
/// between the two — this is the same reason `context_for_file::service`
/// builds `ContextChunk`/`ContextRequirement` by hand instead of via
/// `From` from the repo rows it reads.
fn to_governing_behaviors(
    rows: Vec<fubbik_db::repo::behavior_matrix::BehaviorForFile>,
) -> Vec<GoverningBehavior> {
    rows.into_iter()
        .map(|r| GoverningBehavior {
            rule_id: r.rule_id,
            rule_title: r.rule_title,
            description: r.description,
            rationale: r.rationale,
            counterexample: r.counterexample,
            matrix_id: r.matrix_id,
            matrix_name: r.matrix_name,
            layer: r.layer,
            dimension_name: r.dimension_name,
            kind: r.kind,
            code_ref: r.code_ref,
        })
        .collect()
}

/// `GET /api/context/for-file?path=X&spaceId=&deps=&format=&maxTokens=`.
#[utoipa::path(get, path = "/api/context/for-file", params(ForFileQuery),
    responses((status = 200, body = ForFileResponse)))]
pub async fn for_file(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ForFileQuery>,
) -> ApiResult<axum::Json<ForFileResponse>> {
    let format = query.format.unwrap_or_default();

    if format == ForFileFormat::JsonLegacy {
        let deps = parse_deps(query.deps.as_deref());
        let file_context = get_context_for_file(
            &state.pool,
            &state.ai,
            &state.background,
            &user.id,
            &query.path,
            query.space_id.as_deref(),
            deps.as_deref(),
        )
        .await?;
        return Ok(axum::Json(ForFileResponse::JsonLegacy(file_context)));
    }

    let max_tokens = parse_max_tokens(query.max_tokens.as_deref());
    let paths = vec![query.path.clone()];
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

    // Governing behaviours are optional context — a failure here (e.g. a
    // matrix table being empty, or any other lookup error) must never
    // break file context. Matches Node's `Effect.catchAll(() =>
    // Effect.succeed([]))` (`context-for-file/routes.ts:37-38`) exactly:
    // degrade to "no behaviours found", not a 500.
    let governing = to_governing_behaviors(
        crate::matrices::service::behaviors_for_path(&state.pool, &user.id, &query.path)
            .await
            .unwrap_or_default(),
    );

    let response = match format {
        ForFileFormat::StructuredJson => ForFileStructuredResponse::StructuredJson {
            sections: structured.sections,
            total_chunks: structured.total_chunks,
            behaviors: governing,
        },
        // `StructuredMd` is the default arm; `JsonLegacy` already returned above.
        _ => {
            let content_md = format_structured_markdown(&structured);
            let behaviors_md = format_behaviors_markdown(&governing);
            // Append only when non-empty — `format_behaviors_markdown`'s
            // contract is exactly this: an empty string for an empty list
            // so this check, and only this check, decides whether the
            // `\n\n` separator (and the section itself) appears at all.
            let content = if behaviors_md.is_empty() {
                content_md
            } else {
                format!("{content_md}\n\n{behaviors_md}")
            };
            ForFileStructuredResponse::StructuredMd {
                content,
                total_chunks: structured.total_chunks,
            }
        }
    };

    Ok(axum::Json(ForFileResponse::Structured(response)))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/context/for-file", get(for_file))
}
