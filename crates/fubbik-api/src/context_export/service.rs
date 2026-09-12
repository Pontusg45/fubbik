//! `GET /api/chunks/export/context`.
//!
//! Ports `packages/api/src/context-export/service.ts:15-87`: fetches every
//! non-archived chunk in scope (approved chunks first, then everything
//! else, deduplicated), enriches and scores them through the shared
//! `context::service::enrich_chunks` pipeline, optionally boosts chunks
//! relevant to a `forPath`, budgets the result into `maxTokens`, and
//! formats it as either markdown or a bare JSON chunk list.
//!
//! **Fetch width matches Node's, via `chunk::list_internal`.** Node's two
//! `listChunksRepo` calls each request up to 500 rows (`service.ts:19-32`).
//! `chunk::list` (the function every HTTP-facing chunk listing goes
//! through) clamps to 100 — deliberately, to protect `GET /api/chunks` and
//! `POST /api/search/query` — so this internal caller uses
//! `chunk::list_internal` instead, requesting exactly `500` to match
//! Node's own width rather than being silently narrowed by a clamp meant
//! for a different pair of endpoints. See `chunk::list_internal`'s own doc
//! comment for why that function exists rather than widening the shared
//! clamp.

use std::collections::HashSet;

use fubbik_core::error::AppResult;
use fubbik_core::format::{
    ChunkWithMetadata, format_chunk_text, format_structured, format_structured_markdown,
};
use fubbik_core::tokens::estimate_tokens;
use fubbik_db::repo::chunk;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::context::resolvers::resolve_for_files;
use crate::context::routes::budget_metadata;
use crate::context::service::enrich_chunks;

/// Node's `query.maxTokens ?? 4000` (`service.ts:16`).
pub const DEFAULT_MAX_TOKENS: usize = 4000;

/// The `forPath` relevance boost — Node adds `15` flat to `item.score` for
/// every chunk `resolveForFiles` resolves for the given path
/// (`service.ts:44-51`).
const FOR_PATH_BONUS: f64 = 15.0;

/// The `format` query param. Mirrors Node's
/// `t.Union([t.Literal("markdown"), t.Literal("json")])`
/// (`context-export/routes.ts:27`) — deliberately NOT the
/// `structured-md`/`structured-json` vocabulary `context::dto::
/// ContextFormat` uses; this is a distinct, narrower export endpoint with
/// its own two literal values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[default]
    Markdown,
    Json,
}

pub struct ExportContextParams<'a> {
    pub space_id: Option<&'a str>,
    pub max_tokens: usize,
    pub format: ExportFormat,
    pub for_path: Option<&'a str>,
}

/// One chunk in the `format=json` response, matching Node's inline
/// `{title, content, type, tags}` projection (`service.ts:63-68`).
#[derive(Debug, Serialize, ToSchema)]
pub struct ExportedChunk {
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub tags: Vec<String>,
}

/// Internally tagged on `format`, matching Node's `{format: "json", ...}` /
/// `{format: "markdown", ...}` response shapes (`service.ts:56-81`) — never
/// both `content` and `chunks` on the same response.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum ExportContextResponse {
    Markdown {
        tokens: usize,
        content: String,
    },
    Json {
        tokens: usize,
        chunks: Vec<ExportedChunk>,
    },
}

/// Ports `exportContext` (`service.ts:15-87`) end to end.
pub async fn export_context(
    pool: &PgPool,
    ai: &fubbik_ai::OllamaClient,
    background: &crate::background::BackgroundRuntime,
    user_id: &str,
    params: ExportContextParams<'_>,
) -> AppResult<ExportContextResponse> {
    // Matches Node's `listChunksRepo({ ..., limit: 500, offset: 0 })` width
    // exactly (`service.ts:19-32`) — see the module doc for why this is
    // `list_internal`, not `list`.
    const FETCH_LIMIT: i64 = 500;

    let approved = chunk::list_internal(
        pool,
        user_id,
        &chunk::ListParams {
            review_status: Some("approved".to_string()),
            space_id: params.space_id.map(str::to_string),
            ..Default::default()
        },
        FETCH_LIMIT,
    )
    .await?;

    let all = chunk::list_internal(
        pool,
        user_id,
        &chunk::ListParams {
            space_id: params.space_id.map(str::to_string),
            ..Default::default()
        },
        FETCH_LIMIT,
    )
    .await?;

    // Approved chunks first, then every other chunk not already counted —
    // matches Node's `[...approved.chunks, ...otherChunks]`
    // (`service.ts:36-38`).
    let approved_ids: HashSet<String> = approved.iter().map(|c| c.id.clone()).collect();
    let mut chunk_ids: Vec<String> = approved.iter().map(|c| c.id.clone()).collect();
    for c in &all {
        if !approved_ids.contains(&c.id) {
            chunk_ids.push(c.id.clone());
        }
    }

    let mut enriched = enrich_chunks(pool, user_id, &chunk_ids).await?;

    if let Some(for_path) = params.for_path {
        let file_ids = resolve_for_files(
            pool,
            ai,
            background,
            user_id,
            &[for_path.to_string()],
            params.space_id,
        )
        .await?;
        let file_id_set: HashSet<String> = file_ids.into_iter().collect();
        for item in enriched.iter_mut() {
            if file_id_set.contains(&item.chunk.id) {
                item.chunk.score += FOR_PATH_BONUS;
            }
        }
    }

    let budgeted: Vec<ChunkWithMetadata> = budget_metadata(enriched, params.max_tokens);

    Ok(match params.format {
        ExportFormat::Json => {
            let mut tokens = estimate_tokens("# Project Context\n\n");
            let mut chunks = Vec::with_capacity(budgeted.len());
            for c in &budgeted {
                tokens += estimate_tokens(&format_chunk_text(&c.chunk));
                chunks.push(ExportedChunk {
                    title: c.chunk.title.clone(),
                    content: c.chunk.content.clone(),
                    chunk_type: c.chunk.chunk_type.clone(),
                    tags: c.chunk.tags.clone(),
                });
            }
            ExportContextResponse::Json { tokens, chunks }
        }
        ExportFormat::Markdown => {
            let structured = format_structured(budgeted);
            let content = format_structured_markdown(&structured);
            let tokens = estimate_tokens(&content);
            ExportContextResponse::Markdown { tokens, content }
        }
    })
}
