//! Direct port of `packages/api/src/chunks/health-score.ts`'s
//! `computeHealthScore`. A pure function — no I/O, no `AppResult` — so
//! every call site (currently just `search::service`) is free to call it
//! synchronously.
//!
//! `updated_at` is deliberately **not** a field on [`ChunkHealthInput`]:
//! Node's `ChunkHealthInput.updatedAt` is accepted but never read —
//! `freshness` is hardcoded to `20` (see the comment above that field in
//! `health-score.ts`, "no longer age-penalised"). Carrying an unused
//! parameter through the Rust port would either be dead code or need an
//! `#[allow(unused)]`; dropping it is the faithful port of what the
//! function actually *does*, not what its TS signature merely accepts.

/// Mirrors `ChunkHealthInput` minus `updatedAt` (see module doc).
pub struct ChunkHealthInput<'a> {
    pub content: &'a str,
    pub summary: Option<&'a str>,
    pub rationale: Option<&'a str>,
    pub alternatives: Option<&'a [String]>,
    pub consequences: Option<&'a str>,
    pub connection_count: i64,
    pub centrality_degree: i64,
    pub has_embedding: bool,
    pub requirement_count: i64,
    pub all_requirements_passing: bool,
    pub referenced_in_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthScoreBreakdown {
    pub freshness: i64,
    pub completeness: i64,
    pub richness: i64,
    pub connectivity: i64,
    pub coverage: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthScore {
    pub total: i64,
    pub breakdown: HealthScoreBreakdown,
    pub issues: Vec<String>,
}

/// Port of `computeHealthScore` (`packages/api/src/chunks/health-score.ts:28-103`).
/// Every branch below is a direct translation — see that file for the
/// category-by-category rationale (freshness's age-penalty removal,
/// richness's content-length tiers, connectivity's centrality bonus,
/// coverage's requirement-backing tiers).
pub fn compute_health_score(input: &ChunkHealthInput) -> HealthScore {
    let mut issues: Vec<String> = Vec::new();

    // Freshness (0-20): never age-penalised — see module doc.
    let freshness = 20;

    // Completeness (0-20): base 8 for content, +4 each for
    // rationale/alternatives/consequences.
    let mut completeness = if !input.content.is_empty() { 8 } else { 0 };
    if input.rationale.is_some_and(|r| !r.is_empty()) {
        completeness += 4;
    }
    if input.alternatives.is_some_and(|a| !a.is_empty()) {
        completeness += 4;
    }
    if input.consequences.is_some_and(|c| !c.is_empty()) {
        completeness += 4;
    }

    // Richness (0-20): content length + summary + embedding.
    let mut richness = 0;
    let content_len = input.content.chars().count() as i64;
    if content_len >= 200 {
        richness += 8;
    } else if content_len >= 100 {
        richness += 4;
    }
    if content_len < 100 {
        issues.push("Content is thin (less than 100 characters)".to_string());
    }
    if input.summary.is_some_and(|s| !s.is_empty()) {
        richness += 6;
    } else {
        issues.push("Missing AI summary".to_string());
    }
    if input.has_embedding {
        richness += 6;
    } else {
        issues.push("Missing embedding for semantic search".to_string());
    }

    // Connectivity (0-20): base from connection count + centrality bonus.
    let connectivity = if input.connection_count == 0 {
        issues.push("Orphan chunk with no connections".to_string());
        0
    } else {
        let base = if input.connection_count >= 3 { 12 } else { 8 };
        let centrality_bonus = (input.centrality_degree / 2).min(8);
        (base + centrality_bonus).min(20)
    };

    // Coverage (0-20): requirement backing.
    let coverage = if input.requirement_count == 0 {
        issues.push("No requirements linked".to_string());
        0
    } else if !input.all_requirements_passing {
        10
    } else if !input.referenced_in_session {
        15
    } else {
        20
    };

    let total = freshness + completeness + richness + connectivity + coverage;

    HealthScore {
        total,
        breakdown: HealthScoreBreakdown {
            freshness,
            completeness,
            richness,
            connectivity,
            coverage,
        },
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_input(content: &str) -> ChunkHealthInput<'_> {
        ChunkHealthInput {
            content,
            summary: Some("A summary"),
            rationale: Some("Some rationale"),
            alternatives: Some(&[]),
            consequences: Some("Some consequences"),
            connection_count: 3,
            centrality_degree: 16,
            has_embedding: true,
            requirement_count: 1,
            all_requirements_passing: true,
            referenced_in_session: true,
        }
    }

    #[test]
    fn full_marks_are_100() {
        let content = "A".repeat(200);
        let alts = vec!["alt1".to_string(), "alt2".to_string()];
        let input = ChunkHealthInput {
            alternatives: Some(&alts),
            ..full_input(&content)
        };
        let score = compute_health_score(&input);
        assert_eq!(score.total, 100);
        assert_eq!(score.breakdown.freshness, 20);
        assert_eq!(score.breakdown.completeness, 20);
        assert_eq!(score.breakdown.richness, 20);
        assert_eq!(score.breakdown.connectivity, 20);
        assert_eq!(score.breakdown.coverage, 20);
        assert!(score.issues.is_empty());
    }

    #[test]
    fn thin_content_is_penalized() {
        let input = ChunkHealthInput {
            content: "Short",
            ..full_input("Short")
        };
        let score = compute_health_score(&input);
        assert!(score.total < 100);
        assert!(score.breakdown.richness < 20);
        assert!(
            score
                .issues
                .contains(&"Content is thin (less than 100 characters)".to_string())
        );
    }

    #[test]
    fn missing_enrichment_is_penalized() {
        let content = "A".repeat(200);
        let input = ChunkHealthInput {
            summary: None,
            has_embedding: false,
            ..full_input(&content)
        };
        let score = compute_health_score(&input);
        assert!(score.total < 90);
        assert!(score.issues.contains(&"Missing AI summary".to_string()));
        assert!(
            score
                .issues
                .contains(&"Missing embedding for semantic search".to_string())
        );
    }

    #[test]
    fn orphan_chunks_are_penalized() {
        let content = "A".repeat(200);
        let input = ChunkHealthInput {
            connection_count: 0,
            ..full_input(&content)
        };
        let score = compute_health_score(&input);
        assert!(score.total < 90);
        assert_eq!(score.breakdown.connectivity, 0);
        assert!(
            score
                .issues
                .contains(&"Orphan chunk with no connections".to_string())
        );
    }

    #[test]
    fn base_connectivity_without_centrality() {
        let content = "A".repeat(200);
        let input = ChunkHealthInput {
            connection_count: 2,
            centrality_degree: 0,
            ..full_input(&content)
        };
        assert_eq!(compute_health_score(&input).breakdown.connectivity, 8);
    }

    #[test]
    fn connectivity_boosted_by_centrality() {
        let content = "A".repeat(200);
        let input = ChunkHealthInput {
            connection_count: 2,
            centrality_degree: 10,
            ..full_input(&content)
        };
        // base 8 + min(floor(10/2), 8) = 8 + 5 = 13
        assert_eq!(compute_health_score(&input).breakdown.connectivity, 13);
    }

    #[test]
    fn medium_content_gets_partial_richness() {
        let content = "A".repeat(150);
        let input = ChunkHealthInput {
            content: &content,
            ..full_input(&content)
        };
        assert_eq!(compute_health_score(&input).breakdown.richness, 16); // 4 + 6 + 6
    }

    #[test]
    fn coverage_tiers() {
        let content = "A".repeat(200);

        let no_reqs = ChunkHealthInput {
            requirement_count: 0,
            ..full_input(&content)
        };
        let score = compute_health_score(&no_reqs);
        assert_eq!(score.breakdown.coverage, 0);
        assert!(score.issues.contains(&"No requirements linked".to_string()));

        let not_passing = ChunkHealthInput {
            requirement_count: 2,
            all_requirements_passing: false,
            ..full_input(&content)
        };
        assert_eq!(compute_health_score(&not_passing).breakdown.coverage, 10);

        let not_referenced = ChunkHealthInput {
            requirement_count: 1,
            all_requirements_passing: true,
            referenced_in_session: false,
            ..full_input(&content)
        };
        assert_eq!(compute_health_score(&not_referenced).breakdown.coverage, 15);

        let full = ChunkHealthInput {
            requirement_count: 1,
            all_requirements_passing: true,
            referenced_in_session: true,
            ..full_input(&content)
        };
        assert_eq!(compute_health_score(&full).breakdown.coverage, 20);
    }
}
