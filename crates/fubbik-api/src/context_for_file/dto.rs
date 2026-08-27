//! Wire DTOs for `GET /api/context/for-file`.
//!
//! Ports the query shape and both response shapes from
//! `packages/api/src/context-for-file/routes.ts`. Node supports three
//! `format` values here, one more than the other three `/api/context/*`
//! routes (`context::dto::ContextFormat`): `json-legacy`, kept "for
//! backwards compatibility" per its own comment
//! (`context-for-file/routes.ts:26`), returns `getContextForFile`'s raw
//! `{ chunks, requirements }` shape directly — untagged, no `format` field
//! on the wire at all (`Effect.map(result => ({ ...result }))`,
//! `routes.ts:29`) — while `structured-md`/`structured-json` share
//! `context::dto::ContextResponse`'s tagged envelope.

use fubbik_core::error::AppResult;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::context::dto::ContextResponse;

/// The `format` query param, one variant wider than `context::dto::ContextFormat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ForFileFormat {
    #[default]
    StructuredMd,
    StructuredJson,
    JsonLegacy,
}

/// Query for `GET /api/context/for-file` (`context-for-file/routes.ts:66-73`).
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ForFileQuery {
    pub path: String,
    pub space_id: Option<String>,
    /// Comma-separated dependency names, parsed by the handler — only ever
    /// read on the `json-legacy` path, matching Node exactly (the
    /// `structured-md`/`structured-json` branch calls `resolveForFiles`,
    /// whose signature carries no `deps` parameter at all).
    pub deps: Option<String>,
    pub max_tokens: Option<String>,
    pub format: Option<ForFileFormat>,
}

/// One strategy's name, exactly as it appears on the wire — Node's
/// `ContextChunk["matchReason"]` union (`context-for-file/service.ts:27`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MatchReason {
    FileRef,
    AppliesTo,
    Dependency,
    Semantic,
    Connected,
}

/// A chunk in `getContextForFile`'s own result shape — ports `ContextChunk`
/// (`context-for-file/service.ts:21-29`). Deliberately a narrower field set
/// than `fubbik_core::format::ChunkWithMetadata`: no `tags`, no
/// `healthScore`/`isStale`/`hasPendingProposal`, because this is the
/// pre-enrichment shape the five strategies themselves produce, not the
/// output of `context::service::enrich_chunks`.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub content: String,
    pub summary: Option<String>,
    pub match_reason: MatchReason,
    pub score: f64,
}

/// One BDD step on a matched requirement — `{keyword, text}` only, matching
/// Node's `.map(s => ({ keyword: s.keyword, text: s.text }))`
/// (`context-for-file/service.ts:316`), which drops `params`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ContextRequirementStep {
    pub keyword: fubbik_db::repo::requirement::StepKeyword,
    pub text: String,
}

/// Ports `ContextRequirement` (`context-for-file/service.ts:31-38`).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextRequirement {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    pub steps: Vec<ContextRequirementStep>,
    pub matched_chunk_ids: Vec<String>,
}

/// Ports `FileContext` (`context-for-file/service.ts:40-43`) — the
/// `json-legacy` response body, and `get_context_for_file`'s own return
/// type.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FileContext {
    pub chunks: Vec<ContextChunk>,
    pub requirements: Vec<ContextRequirement>,
}

/// The full response envelope for `GET /api/context/for-file`, spanning all
/// three `format` values. `#[serde(untagged)]` is load-bearing: it is what
/// keeps `JsonLegacy`'s wire shape untagged (`{chunks, requirements}`, no
/// `format` key), matching Node's `{ ...result }` spread — an internally
/// tagged enum (`context::dto::ContextResponse`'s own `#[serde(tag =
/// "format")]`) would add a `format` field Node never sends here.
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
pub enum ForFileResponse {
    Structured(ContextResponse),
    JsonLegacy(FileContext),
}

/// Parses `deps` the same way Node's route handler does
/// (`ctx.query.deps ? ctx.query.deps.split(",").filter(Boolean) : undefined`,
/// `context-for-file/routes.ts:28`): `None` when the query param is absent,
/// `Some(vec![])` is never produced for an absent param but IS possible for
/// a present-but-empty one (`deps=`) after filtering out empty segments —
/// matching `.filter(Boolean)` dropping empty strings.
pub fn parse_deps(raw: Option<&str>) -> Option<Vec<String>> {
    raw.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    })
}

pub type ForFileResult = AppResult<ForFileResponse>;
