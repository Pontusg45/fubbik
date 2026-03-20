export interface ChunkHealthInput {
    content: string;
    updatedAt: Date;
    summary: string | null;
    rationale: string | null;
    alternatives: string[] | null;
    consequences: string | null;
    connectionCount: number;
    hasEmbedding: boolean;
}

export interface HealthScore {
    total: number; // 0-100
    breakdown: {
        freshness: number; // 0-25
        completeness: number; // 0-25
        richness: number; // 0-25
        connectivity: number; // 0-25
    };
    issues: string[];
}

export function computeHealthScore(input: ChunkHealthInput): HealthScore {
    const issues: string[] = [];

    // Freshness (0-25): Full at <7 days, degrades to 0 at 90 days
    const daysSinceUpdate = (Date.now() - input.updatedAt.getTime()) / (1000 * 60 * 60 * 24);
    let freshness: number;
    if (daysSinceUpdate < 7) {
        freshness = 25;
    } else if (daysSinceUpdate >= 90) {
        freshness = 0;
    } else {
        freshness = Math.round(25 * (1 - (daysSinceUpdate - 7) / (90 - 7)));
    }
    if (daysSinceUpdate >= 30) {
        issues.push("Chunk has not been updated in over 30 days");
    }

    // Completeness (0-25): Base 10 for content, +5 each for rationale/alternatives/consequences
    let completeness = input.content.length > 0 ? 10 : 0;
    if (input.rationale) completeness += 5;
    if (input.alternatives && input.alternatives.length > 0) completeness += 5;
    if (input.consequences) completeness += 5;

    // Richness (0-25): content length + summary + embedding
    let richness = 0;
    if (input.content.length >= 200) {
        richness += 10;
    } else if (input.content.length >= 100) {
        richness += 5;
    }
    if (input.content.length < 100) {
        issues.push("Content is thin (less than 100 characters)");
    }
    if (input.summary) {
        richness += 8;
    } else {
        issues.push("Missing AI summary");
    }
    if (input.hasEmbedding) {
        richness += 7;
    } else {
        issues.push("Missing embedding for semantic search");
    }

    // Connectivity (0-25): 25 for 3+, 15 for 1-2, 0 for orphans
    let connectivity: number;
    if (input.connectionCount >= 3) {
        connectivity = 25;
    } else if (input.connectionCount >= 1) {
        connectivity = 15;
    } else {
        connectivity = 0;
        issues.push("Orphan chunk with no connections");
    }

    const total = freshness + completeness + richness + connectivity;

    return {
        total,
        breakdown: {
            freshness,
            completeness,
            richness,
            connectivity
        },
        issues
    };
}
