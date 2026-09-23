//! The chunk endpoint that needs an embedding.
//!
//! Ports the embedding-dependent half of
//! `packages/api/src/chunks/chunk-search.ts` (`semanticSearch`,
//! `chunk-search.ts:59-73`) plus its route registration
//! (`packages/api/src/chunks/routes.ts:192-206`).
use fubbik_ai::OllamaClient;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::semantic::{self, SemanticHit};
use fubbik_db::repo::similarity::{self, SimilarChunk};
use sqlx::PgPool;

/// `Math.min(Number(query.limit ?? 5), 20)` (`chunk-search.ts:60`).
///
/// Node's `Math.min` has no floor — this deliberately does not add one
/// either. A `limit=0` or negative value is passed straight through to the
/// SQL `LIMIT` clause exactly as Node would pass it to Drizzle's `.limit()`,
/// rather than silently being bumped up to something "sensible".
fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(5).min(20)
}

/// `"a, b"` -> `["a", "b"]`. Node's `query.exclude.split(",").map(s =>
/// s.trim())` (`chunk-search.ts:61`) — each comma-separated term is
/// trimmed, and a single term with no comma is a one-element vec.
fn parse_exclude(raw: &str) -> Vec<String> {
    raw.split(',').map(|s| s.trim().to_string()).collect()
}

/// `"a:1,b:2"` -> `{"a":"1","b":"2"}`. Node splits each comma-separated
/// entry on `:` and keeps only pairs of length exactly 2
/// (`chunk-search.ts:62-69`), so `"a:b:c"` (3 parts) and `"bare"` (1 part)
/// are both discarded rather than erroring — not just the malformed entry,
/// the whole result is `None` if nothing well-formed remains.
fn parse_scope(raw: &str) -> Option<serde_json::Value> {
    let map: serde_json::Map<String, serde_json::Value> = raw
        .split(',')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.trim().split(':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), serde_json::Value::from(parts[1])))
            } else {
                None
            }
        })
        .collect();
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// No availability probe here, unlike `enrich`
/// (`crates/fubbik-api/src/enrich/service.rs`). Node calls
/// `generateQueryEmbedding` directly (`chunk-search.ts:71`), so an
/// unreachable Ollama surfaces as an `AiError` -> **502**, not as an empty
/// result set. Adding a probe-and-degrade step here, to make this endpoint
/// "consistent" with enrich, would silently turn a hard failure into an
/// empty results page — worse than an error, because the caller cannot
/// tell "no matches" from "the search never ran".
pub async fn semantic_search(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    q: &str,
    limit: Option<i64>,
    exclude: Option<&str>,
    scope: Option<&str>,
) -> AppResult<Vec<SemanticHit>> {
    let embedding = ai.embed_query(q).await.map_err(AppError::from)?;
    let exclude: Vec<String> = exclude.map(parse_exclude).unwrap_or_default();
    let scope = scope.and_then(parse_scope);

    semantic::semantic_search(
        pool,
        &embedding,
        Some(user_id),
        &exclude,
        scope.as_ref(),
        clamp_limit(limit),
    )
    .await
}

/// Node's `checkSimilar` call-site constants (`similarity.ts:13-15`) —
/// threshold 0.75 and limit 3, both overriding the repository defaults of
/// 0.7 and 5.
const CHECK_SIMILAR_THRESHOLD: f64 = 0.75;
const CHECK_SIMILAR_LIMIT: i64 = 3;

/// Probes availability and returns `[]` when Ollama is down — the opposite
/// of [`semantic_search`], which 502s. The asymmetry is Node's
/// (`similarity.ts:9` has the probe; `chunk-search.ts:71` does not) and is
/// preserved deliberately.
///
/// Why the two endpoints diverge: `checkSimilar` fires on every keystroke
/// pause while a user is typing in the create-chunk form (see
/// `apps/web/src/features/chunks/similar-chunks-warning.tsx:23`) — a hard
/// 502 there would surface as an error toast on a screen where the user is
/// not asking for a search, just typing, and there is nothing actionable
/// for them to do about a downed Ollama mid-keystroke. Degrading to "no
/// similar chunks found" is a safe, silent no-op.
///
/// `GET /api/chunks/search/semantic` does the opposite because an empty
/// result *is* the user's answer there: they explicitly ran a search, so
/// an empty page must mean "no matches", not silently swallow "the search
/// never ran". Adding a probe there would make a broken backend
/// indistinguishable from a true empty result. Do not "fix" this asymmetry
/// by making the two endpoints consistent — that would break one of them.
pub async fn check_similar(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    title: &str,
    content: &str,
    exclude_id: Option<&str>,
) -> AppResult<Vec<SimilarChunk>> {
    if !ai.is_available().await {
        return Ok(Vec::new());
    }
    let embedding = ai
        .embed_document(title, None, content)
        .await
        .map_err(AppError::from)?;
    similarity::find_similar_by_embedding(
        pool,
        &embedding,
        user_id,
        exclude_id,
        CHECK_SIMILAR_THRESHOLD,
        CHECK_SIMILAR_LIMIT,
    )
    .await
}

/// Node's graph bonus (`semantic.ts:105`).
const GRAPH_BONUS: f64 = 0.15;
/// Node's `graphHops` default (`semantic.ts:99`).
const GRAPH_HOPS: i32 = 2;

/// [`fubbik_db::repo::semantic::NeighborRow`] plus the hybrid-scoring
/// fields Node's `findRelatedChunksHybrid` adds
/// (`packages/db/src/repository/semantic.ts:106-115`).
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NeighborItem {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub distance: f64,
    pub embedding_similarity: f64,
    pub graph_connected: bool,
    pub combined_score: f64,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NeighborsResponse {
    pub neighbors: Vec<NeighborItem>,
    pub note: Option<String>,
}

/// Never calls Ollama: the source chunk's *stored* embedding drives this,
/// so a chunk that has never been enriched gets a note rather than a
/// freshly generated vector (`chunk-search.ts:31-45`).
pub async fn neighbors(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    k: i64,
) -> AppResult<NeighborsResponse> {
    let source = fubbik_db::repo::chunk::find_by_id(pool, user_id, chunk_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Chunk".into()))?;

    if source.embedding.is_none() {
        return Ok(NeighborsResponse {
            neighbors: Vec::new(),
            note: Some("Chunk has no embedding — run enrichment first.".to_string()),
        });
    }

    // Node over-fetches `k * 2` so the graph bonus has room to reorder
    // before the final truncation to `k` (`semantic.ts:101`).
    let rows = semantic::find_neighbors_by_chunk_id(pool, chunk_id, user_id, k * 2).await?;

    // AGE failure degrades to "no bonus", matching Node's `Effect.catchAll`
    // (`semantic.ts:103`) — a missing graph must not fail the endpoint.
    let graph_ids: std::collections::HashSet<String> =
        fubbik_db::age::get_neighborhood(pool, chunk_id, GRAPH_HOPS)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let mut scored: Vec<NeighborItem> = rows
        .into_iter()
        .map(|r| {
            let graph_connected = graph_ids.contains(&r.id);
            let embedding_similarity = 1.0 - r.distance;
            NeighborItem {
                id: r.id,
                title: r.title,
                summary: r.summary,
                chunk_type: r.chunk_type,
                distance: r.distance,
                embedding_similarity,
                graph_connected,
                combined_score: embedding_similarity
                    + if graph_connected { GRAPH_BONUS } else { 0.0 },
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(k as usize);

    Ok(NeighborsResponse {
        neighbors: scored,
        note: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{clamp_limit, parse_exclude, parse_scope};

    #[test]
    fn clamp_limit_defaults_to_five() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(clamp_limit(None), 5);
    }

    #[test]
    fn clamp_limit_caps_at_twenty() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(clamp_limit(Some(100)), 20);
    }

    #[test]
    fn clamp_limit_passes_through_values_under_the_cap() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(clamp_limit(Some(3)), 3);
    }

    #[test]
    fn parse_exclude_splits_and_trims_whitespace() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            parse_exclude("a, b"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn parse_exclude_single_term_with_no_comma() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_exclude("billing"), vec!["billing".to_string()]);
    }

    #[test]
    fn parse_scope_builds_an_object_from_well_formed_pairs() {
        // Given the inline inputs and test fixtures.
        // When
        let scope = parse_scope("env:prod,tier:gold").unwrap();
        // Then
        assert_eq!(scope, serde_json::json!({ "env": "prod", "tier": "gold" }));
    }

    /// `"a:b:c"` splits into 3 parts on `:`, not 2 — Node's `.filter(p =>
    /// p.length === 2)` discards it, so the whole thing yields `None`
    /// (nothing else was in the string).
    #[test]
    fn parse_scope_a_b_c_yields_none() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_scope("a:b:c"), None);
    }

    /// `"x:1,bad"` mixes one well-formed pair with one malformed entry
    /// (`"bad"` has no colon at all). The malformed entry is dropped; the
    /// well-formed one still applies.
    #[test]
    fn parse_scope_x_1_bad_keeps_only_x() {
        // Given the inline inputs and test fixtures.
        // When
        let scope = parse_scope("x:1,bad").unwrap();
        // Then
        assert_eq!(scope, serde_json::json!({ "x": "1" }));
    }

    #[test]
    fn parse_scope_all_malformed_yields_none() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_scope("bare,a:b:c"), None);
    }

    #[test]
    fn parse_scope_empty_string_yields_none() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_scope(""), None);
    }
}
