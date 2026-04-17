export interface ChunkHealthInput {
    content: string;
    updatedAt: Date;
    summary: string | null;
    rationale: string | null;
    alternatives: string[] | null;
    consequences: string | null;
    connectionCount: number;
    hasEmbedding: boolean;
    requirementCount: number;
    allRequirementsPassing: boolean;
    referencedInSession: boolean;
}

export interface HealthScore {
    total: number; // 0-100
    breakdown: {
        freshness: number; // 0-20
        completeness: number; // 0-20
        richness: number; // 0-20
        connectivity: number; // 0-20
        coverage: number; // 0-20
    };
    issues: string[];
}

export function computeHealthScore(input: ChunkHealthInput): HealthScore {
    const issues: string[] = [];

    // Freshness (0-20): no longer age-penalised. Old chunks are not inherently
    // worse — a 2-year-old architecture decision can still be correct. Age-based
    // signals live in the separate `chunk_staleness` feature (file_changed,
    // age, diverged_duplicate), which the user can dismiss per flag.
    // The category stays in the breakdown for backwards-compat with callers
    // that render a 5-bar widget.
    const freshness = 20;

    // Completeness (0-20): Base 8 for content, +4 each for rationale/alternatives/consequences
    let completeness = input.content.length > 0 ? 8 : 0;
    if (input.rationale) completeness += 4;
    if (input.alternatives && input.alternatives.length > 0) completeness += 4;
    if (input.consequences) completeness += 4;

    // Richness (0-20): content length + summary + embedding
    let richness = 0;
    if (input.content.length >= 200) {
        richness += 8;
    } else if (input.content.length >= 100) {
        richness += 4;
    }
    if (input.content.length < 100) {
        issues.push("Content is thin (less than 100 characters)");
    }
    if (input.summary) {
        richness += 6;
    } else {
        issues.push("Missing AI summary");
    }
    if (input.hasEmbedding) {
        richness += 6;
    } else {
        issues.push("Missing embedding for semantic search");
    }

    // Connectivity (0-20): 20 for 3+, 12 for 1-2, 0 for orphans
    let connectivity: number;
    if (input.connectionCount >= 3) {
        connectivity = 20;
    } else if (input.connectionCount >= 1) {
        connectivity = 12;
    } else {
        connectivity = 0;
        issues.push("Orphan chunk with no connections");
    }

    // Coverage (0-20): requirement backing
    let coverage: number;
    if (input.requirementCount === 0) {
        coverage = 0;
        issues.push("No requirements linked");
    } else if (!input.allRequirementsPassing) {
        coverage = 10;
    } else if (!input.referencedInSession) {
        coverage = 15;
    } else {
        coverage = 20;
    }

    const total = freshness + completeness + richness + connectivity + coverage;

    return {
        total,
        breakdown: {
            freshness,
            completeness,
            richness,
            connectivity,
            coverage
        },
        issues
    };
}
