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
        requirementCount: 1,
        allRequirementsPassing: true,
        referencedInSession: true,
        ...overrides
    };
}

describe("computeHealthScore", () => {
    it("returns 100 for a fully complete, fresh chunk with 3+ connections and passing requirements", () => {
        const score = computeHealthScore(makeInput());
        expect(score.total).toBe(100);
        expect(score.breakdown.freshness).toBe(20);
        expect(score.breakdown.completeness).toBe(20);
        expect(score.breakdown.richness).toBe(20);
        expect(score.breakdown.connectivity).toBe(20);
        expect(score.breakdown.coverage).toBe(20);
        expect(score.issues).toHaveLength(0);
    });

    it("penalizes stale chunks (45 days old)", () => {
        const score = computeHealthScore(
            makeInput({ updatedAt: new Date(Date.now() - 45 * 86400000) })
        );
        expect(score.total).toBeLessThan(90);
        expect(score.breakdown.freshness).toBeLessThan(20);
        expect(score.issues).toContain("Chunk has not been updated in over 30 days");
    });

    it("penalizes thin content", () => {
        const score = computeHealthScore(makeInput({ content: "Short" }));
        expect(score.total).toBeLessThan(100);
        expect(score.breakdown.richness).toBeLessThan(20);
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
        expect(score.breakdown.connectivity).toBe(12);
    });

    it("gives partial richness for medium content (100-199 chars)", () => {
        const score = computeHealthScore(makeInput({ content: "A".repeat(150) }));
        expect(score.breakdown.richness).toBe(16); // 4 + 6 + 6
    });

    it("returns 0 freshness at 90+ days", () => {
        const score = computeHealthScore(
            makeInput({ updatedAt: new Date(Date.now() - 100 * 86400000) })
        );
        expect(score.breakdown.freshness).toBe(0);
    });

    it("gives 0 coverage when no requirements linked", () => {
        const score = computeHealthScore(makeInput({ requirementCount: 0 }));
        expect(score.breakdown.coverage).toBe(0);
        expect(score.issues).toContain("No requirements linked");
    });

    it("gives 10 coverage when requirements linked but not all passing", () => {
        const score = computeHealthScore(makeInput({ requirementCount: 2, allRequirementsPassing: false }));
        expect(score.breakdown.coverage).toBe(10);
    });

    it("gives 15 coverage when all requirements pass but not referenced in session", () => {
        const score = computeHealthScore(makeInput({ requirementCount: 1, allRequirementsPassing: true, referencedInSession: false }));
        expect(score.breakdown.coverage).toBe(15);
    });

    it("gives 20 coverage when all requirements pass and referenced in session", () => {
        const score = computeHealthScore(makeInput({ requirementCount: 1, allRequirementsPassing: true, referencedInSession: true }));
        expect(score.breakdown.coverage).toBe(20);
    });
});
