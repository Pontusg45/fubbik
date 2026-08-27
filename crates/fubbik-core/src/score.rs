//! Chunk scoring and token budgeting.
//!
//! Ports `packages/api/src/context/utils.ts:39-76`.
//!
//! `budgetChunksWithCoverage` (`utils.ts:81`) is deliberately NOT ported:
//! it has zero callers, and its only source of `communityId` is
//! `packages/api/src/graph/community-analysis.ts`, which Phase 4a
//! established has no clients either. Porting it would mean writing
//! untestable code for a path that cannot execute.
use crate::format::format_chunk_text;
use crate::health::HealthScore;
use crate::tokens::estimate_tokens;

/// Node seeds the running total with this header's own token count
/// (`utils.ts:66`).
const HEADER_SEED: &str = "# Project Context\n\n";

#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub id: String,
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
    pub tags: Vec<String>,
    pub score: f64,
}

pub struct ScoreInput<'a> {
    pub chunk_type: &'a str,
    pub rationale: Option<&'a str>,
    pub review_status: &'a str,
    pub connection_count: i64,
    pub health: &'a HealthScore,
}

/// Sum of five terms. Health contributes `total / 10` and there is
/// deliberately **no separate freshness term** — freshness is already
/// inside `compute_health_score`, and adding one here would double-count
/// it. Node carries the same warning as a comment at `utils.ts:59`.
pub fn score_chunk(input: &ScoreInput) -> f64 {
    let health_points = input.health.total as f64 / 10.0;
    let type_points = match input.chunk_type {
        "document" => 3.0,
        "note" => 1.0,
        _ => 2.0,
    };
    let rationale_points = if input.rationale.is_some() { 2.0 } else { 0.0 };
    let connection_points = (input.connection_count * 2).min(10) as f64;
    let review_points = match input.review_status {
        "approved" => 2.0,
        "reviewed" => 1.0,
        _ => 0.0,
    };
    health_points + type_points + rationale_points + connection_points + review_points
}

/// Greedily fills a token budget, highest score first.
///
/// A chunk that would exceed the budget is **skipped**, not a stopping
/// point: Node uses `continue`, so one oversized chunk does not truncate
/// the export while smaller lower-scored chunks still fit.
pub fn budget_chunks(chunks: Vec<ScoredChunk>, max_tokens: usize) -> Vec<ScoredChunk> {
    let mut sorted = chunks;
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::new();
    let mut used = estimate_tokens(HEADER_SEED);

    for chunk in sorted {
        let tokens = estimate_tokens(&format_chunk_text(&chunk));
        if used + tokens > max_tokens {
            continue;
        }
        used += tokens;
        selected.push(chunk);
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ChunkHealthInput, compute_health_score};

    fn health_for(
        content: &str,
        rationale: Option<&str>,
        connections: i64,
    ) -> crate::health::HealthScore {
        compute_health_score(&ChunkHealthInput {
            content,
            summary: None,
            rationale,
            alternatives: None,
            consequences: None,
            connection_count: connections,
            centrality_degree: 0,
            has_embedding: false,
            requirement_count: 0,
            all_requirements_passing: false,
            referenced_in_session: false,
        })
    }

    fn scored(id: &str, score: f64, content: &str) -> ScoredChunk {
        ScoredChunk {
            id: id.into(),
            title: "T".into(),
            content: content.into(),
            chunk_type: "note".into(),
            rationale: None,
            tags: vec![],
            score,
        }
    }

    #[test]
    fn type_points_are_three_one_two() {
        let h = health_for("x", None, 0);
        let base = |t: &str| {
            score_chunk(&ScoreInput {
                chunk_type: t,
                rationale: None,
                review_status: "draft",
                connection_count: 0,
                health: &h,
            })
        };
        assert_eq!(base("document") - base("reference"), 1.0);
        assert_eq!(base("reference") - base("note"), 1.0);
    }

    #[test]
    fn rationale_adds_exactly_two() {
        let h = health_for("x", None, 0);
        let without = score_chunk(&ScoreInput {
            chunk_type: "note",
            rationale: None,
            review_status: "draft",
            connection_count: 0,
            health: &h,
        });
        let with = score_chunk(&ScoreInput {
            chunk_type: "note",
            rationale: Some("because"),
            review_status: "draft",
            connection_count: 0,
            health: &h,
        });
        assert_eq!(with - without, 2.0);
    }

    #[test]
    fn connection_points_cap_at_ten() {
        let h = health_for("x", None, 0);
        let at = |n: i64| {
            score_chunk(&ScoreInput {
                chunk_type: "note",
                rationale: None,
                review_status: "draft",
                connection_count: n,
                health: &h,
            })
        };
        assert_eq!(at(5) - at(0), 10.0);
        assert_eq!(
            at(50),
            at(5),
            "connection points must cap at 10, not keep growing"
        );
    }

    #[test]
    fn review_points_are_two_one_zero() {
        let h = health_for("x", None, 0);
        let at = |s: &str| {
            score_chunk(&ScoreInput {
                chunk_type: "note",
                rationale: None,
                review_status: s,
                connection_count: 0,
                health: &h,
            })
        };
        assert_eq!(at("approved") - at("draft"), 2.0);
        assert_eq!(at("reviewed") - at("draft"), 1.0);
    }

    /// Freshness lives inside compute_health_score and contributes through
    /// `health.total / 10`. A separate freshness term would double-count it.
    /// This pins the health contribution to exactly that ratio: if someone
    /// adds a freshness bonus on top, the difference stops matching.
    #[test]
    fn health_contributes_exactly_total_over_ten_and_nothing_else() {
        let lean = health_for("x", None, 0);
        let rich = health_for(&"y".repeat(2000), Some("because"), 0);
        assert_ne!(
            lean.total, rich.total,
            "fixture must produce differing health totals"
        );

        let at = |h: &crate::health::HealthScore| {
            score_chunk(&ScoreInput {
                chunk_type: "note",
                rationale: None,
                review_status: "draft",
                connection_count: 0,
                health: h,
            })
        };

        let expected = (rich.total as f64 / 10.0) - (lean.total as f64 / 10.0);
        assert!(
            (at(&rich) - at(&lean) - expected).abs() < f64::EPSILON,
            "health must contribute exactly total/10 — a separate freshness term would break this"
        );
    }

    /// The budgeter SKIPS an oversized chunk and keeps going; it does not
    /// stop at the first one that will not fit. A test asserting only "the
    /// result fits the budget" passes with a `break` in place of `continue`.
    #[test]
    fn budget_skips_an_oversized_chunk_rather_than_truncating() {
        let huge = scored("huge", 100.0, &"word ".repeat(5000));
        let small_a = scored("a", 50.0, "small");
        let small_b = scored("b", 40.0, "small");

        let kept = budget_chunks(vec![huge, small_a, small_b], 200);
        let ids: Vec<&str> = kept.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.contains(&"huge"),
            "the oversized chunk must be skipped"
        );
        assert_eq!(
            ids,
            vec!["a", "b"],
            "lower-scored chunks that fit must still be selected"
        );
    }

    #[test]
    fn budget_returns_highest_scored_first() {
        let kept = budget_chunks(
            vec![
                scored("low", 1.0, "x"),
                scored("high", 99.0, "x"),
                scored("mid", 50.0, "x"),
            ],
            10_000,
        );
        assert_eq!(
            kept.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["high", "mid", "low"]
        );
    }

    /// The running total is seeded with the header's own token count, so a
    /// budget smaller than the header admits nothing.
    #[test]
    fn budget_accounts_for_the_header_seed() {
        let kept = budget_chunks(vec![scored("a", 1.0, "x")], 1);
        assert!(
            kept.is_empty(),
            "a budget below the header's own cost admits nothing"
        );
    }
}
