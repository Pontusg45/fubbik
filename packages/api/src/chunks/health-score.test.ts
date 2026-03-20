import { describe, expect, it } from "vitest";

import { computeHealthScore, type ChunkHealthInput } from "./health-score";

function makeInput(overrides: Partial<ChunkHealthInput> = {}): ChunkHealthInput {
    return {
        content: "A".repeat(200),
        updatedAt: new Date(), // just now
        summary: "A summary",
        rationale: "Some rationale",
        alternatives: ["alt1", "alt2"],
        consequences: "Some consequences",
        connectionCount: 3,
        hasEmbedding: true,
        ...overrides
    };
}

describe("computeHealthScore", () => {
    it("returns 100 for a fully complete, fresh chunk with 3+ connections", () => {
        const score = computeHealthScore(makeInput());
        expect(score.total).toBe(100);
        expect(score.breakdown.freshness).toBe(25);
        expect(score.breakdown.completeness).toBe(25);
        expect(score.breakdown.richness).toBe(25);
        expect(score.breakdown.connectivity).toBe(25);
        expect(score.issues).toHaveLength(0);
    });

    it("penalizes stale chunks (45 days old)", () => {
        const score = computeHealthScore(
            makeInput({ updatedAt: new Date(Date.now() - 45 * 86400000) })
        );
        expect(score.total).toBeLessThan(90);
        expect(score.breakdown.freshness).toBeLessThan(25);
        expect(score.issues).toContain("Chunk has not been updated in over 30 days");
    });

    it("penalizes thin content", () => {
        const score = computeHealthScore(makeInput({ content: "Short" }));
        expect(score.total).toBeLessThan(100);
        expect(score.breakdown.richness).toBeLessThan(25);
        expect(score.issues).toContain("Content is thin (less than 100 characters)");
    });

    it("penalizes missing enrichment", () => {
        const score = computeHealthScore(
            makeInput({ summary: null, hasEmbedding: false })
        );
        expect(score.total).toBeLessThan(90);
        expect(score.issues).toContain("Missing AI summary");
        expect(score.issues).toContain("Missing embedding for semantic search");
    });

    it("penalizes orphan chunks (0 connections)", () => {
        const score = computeHealthScore(makeInput({ connectionCount: 0 }));
        expect(score.total).toBeLessThan(90);
        expect(score.breakdown.connectivity).toBe(0);
        expect(score.issues).toContain("Orphan chunk with no connections");
    });

    it("gives partial connectivity for 1-2 connections", () => {
        const score = computeHealthScore(makeInput({ connectionCount: 2 }));
        expect(score.breakdown.connectivity).toBe(15);
    });

    it("gives partial richness for medium content (100-199 chars)", () => {
        const score = computeHealthScore(makeInput({ content: "A".repeat(150) }));
        expect(score.breakdown.richness).toBe(20); // 5 + 8 + 7
    });

    it("returns 0 freshness at 90+ days", () => {
        const score = computeHealthScore(
            makeInput({ updatedAt: new Date(Date.now() - 100 * 86400000) })
        );
        expect(score.breakdown.freshness).toBe(0);
    });
});
