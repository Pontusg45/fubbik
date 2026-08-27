//! The chunk endpoint that needs an embedding.
//!
//! Ports the embedding-dependent half of
//! `packages/api/src/chunks/chunk-search.ts` (`semanticSearch`,
//! `chunk-search.ts:59-73`) plus its route registration
//! (`packages/api/src/chunks/routes.ts:192-206`).
use fubbik_ai::OllamaClient;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::semantic::{self, SemanticHit};
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
    let exclude: Vec<String> = exclude
        .map(|raw| raw.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::{clamp_limit, parse_scope};

    #[test]
    fn clamp_limit_defaults_to_five() {
        assert_eq!(clamp_limit(None), 5);
    }

    #[test]
    fn clamp_limit_caps_at_twenty() {
        assert_eq!(clamp_limit(Some(100)), 20);
    }

    #[test]
    fn clamp_limit_passes_through_values_under_the_cap() {
        assert_eq!(clamp_limit(Some(3)), 3);
    }

    #[test]
    fn parse_scope_builds_an_object_from_well_formed_pairs() {
        let scope = parse_scope("env:prod,tier:gold").unwrap();
        assert_eq!(scope, serde_json::json!({ "env": "prod", "tier": "gold" }));
    }

    /// `"a:b:c"` splits into 3 parts on `:`, not 2 — Node's `.filter(p =>
    /// p.length === 2)` discards it, so the whole thing yields `None`
    /// (nothing else was in the string).
    #[test]
    fn parse_scope_a_b_c_yields_none() {
        assert_eq!(parse_scope("a:b:c"), None);
    }

    /// `"x:1,bad"` mixes one well-formed pair with one malformed entry
    /// (`"bad"` has no colon at all). The malformed entry is dropped; the
    /// well-formed one still applies.
    #[test]
    fn parse_scope_x_1_bad_keeps_only_x() {
        let scope = parse_scope("x:1,bad").unwrap();
        assert_eq!(scope, serde_json::json!({ "x": "1" }));
    }

    #[test]
    fn parse_scope_all_malformed_yields_none() {
        assert_eq!(parse_scope("bare,a:b:c"), None);
    }

    #[test]
    fn parse_scope_empty_string_yields_none() {
        assert_eq!(parse_scope(""), None);
    }
}
