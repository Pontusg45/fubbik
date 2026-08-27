//! Wire DTOs for the `/api/context/*` routes.
//!
//! Ports the three query shapes and shared response envelope from
//! `packages/api/src/context/routes.ts`. `maxTokens` arrives as a string on
//! the wire (Node's `t.Optional(t.String())`), same reasoning as
//! `chunks::dto::SemanticSearchQuery`'s `limit` — parsed rather than typed,
//! to keep the wire contract identical.

use fubbik_core::format::StructuredContext;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// The `format` query param shared by all three routes. Mirrors Node's
/// `t.Union([t.Literal("structured-md"), t.Literal("structured-json")])`
/// (`context/routes.ts:49` and its two siblings) — `kebab-case` so
/// `StructuredMd`/`StructuredJson` serialise/deserialise as exactly those
/// two literals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ContextFormat {
    #[default]
    StructuredMd,
    StructuredJson,
}

pub const DEFAULT_MAX_TOKENS: usize = 4000;

/// Parses `maxTokens`, matching Node's `ctx.query.maxTokens ? Number(...) :
/// DEFAULT_MAX_TOKENS` (`context/routes.ts:20`). An unparsable value falls
/// back to the default rather than erroring — Node's `Number("bogus")` is
/// `NaN`, which `budgetChunks` would then compare against with `used +
/// tokens > NaN` (always `false`), silently admitting every chunk; matching
/// that exactly is not worth reproducing, so this port falls back to the
/// default instead of reproducing the `NaN` footgun.
pub fn parse_max_tokens(raw: Option<&str>) -> usize {
    raw.and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_TOKENS)
}

/// Query for `GET /api/context/for-plan` (`context/routes.ts:14-52`).
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ForPlanQuery {
    pub plan_id: String,
    pub max_tokens: Option<String>,
    pub format: Option<ContextFormat>,
}

/// Query for `GET /api/context/about` (`context/routes.ts:55-93`).
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AboutQuery {
    pub q: String,
    pub max_tokens: Option<String>,
    pub space_id: Option<String>,
    pub format: Option<ContextFormat>,
}

/// Query for `GET /api/context/for-files` (`context/routes.ts:96-142`).
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ForFilesQuery {
    pub paths: String,
    pub max_tokens: Option<String>,
    pub space_id: Option<String>,
    pub format: Option<ContextFormat>,
}

/// Shared response envelope for all three routes
/// (`context/routes.ts:32-39` and its two siblings). Internally tagged on
/// `format` so the wire shape is exactly Node's `{ format: "structured-md",
/// content, totalChunks }` or `{ format: "structured-json", sections,
/// totalChunks }` — never both `content` and `sections` on the same
/// response.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "format")]
pub enum ContextResponse {
    #[serde(rename = "structured-md", rename_all = "camelCase")]
    StructuredMd {
        content: String,
        total_chunks: usize,
    },
    #[serde(rename = "structured-json", rename_all = "camelCase")]
    StructuredJson {
        sections: Vec<fubbik_core::format::ContextSection>,
        total_chunks: usize,
    },
}

impl ContextResponse {
    /// Shared resolve->enrich->score->budget->format tail: takes the
    /// already-budgeted, already-formatted [`StructuredContext`] and
    /// renders it into whichever wire shape `format` selects, matching the
    /// `if (format === "structured-json") {...} else {...}` branch
    /// repeated identically in all three Node handlers.
    pub fn from_structured(structured: StructuredContext, format: ContextFormat) -> Self {
        match format {
            ContextFormat::StructuredJson => ContextResponse::StructuredJson {
                sections: structured.sections,
                total_chunks: structured.total_chunks,
            },
            ContextFormat::StructuredMd => ContextResponse::StructuredMd {
                content: fubbik_core::format::format_structured_markdown(&structured),
                total_chunks: structured.total_chunks,
            },
        }
    }
}
