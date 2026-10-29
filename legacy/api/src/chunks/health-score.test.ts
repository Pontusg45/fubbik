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
        centralityDegree: 16,
        hasEmbedding: true,
        requirementCount: 1,
        allRequirementsPassing: true,
        referencedInSession: true,
        ...overrides
    };
}

describe("computeHealthScore", () => {
    it("returns 100 for a fully complete, fresh chunk with 3+ connections and passing requirements", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput());
        // Then
        expect(score.total).toBe(100);
        expect(score.breakdown.freshness).toBe(20);
        expect(score.breakdown.completeness).toBe(20);
        expect(score.breakdown.richness).toBe(20);
        expect(score.breakdown.connectivity).toBe(20);
        expect(score.breakdown.coverage).toBe(20);
        expect(score.issues).toHaveLength(0);
    });

    it("does not penalize age — a 45-day-old chunk keeps full freshness", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ updatedAt: new Date(Date.now() - 45 * 86400000) }));
        // Then
        expect(score.total).toBe(100);
        expect(score.breakdown.freshness).toBe(20);
        expect(score.issues).not.toContain("Chunk has not been updated in over 30 days");
    });

    it("penalizes thin content", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ content: "Short" }));
        // Then
        expect(score.total).toBeLessThan(100);
        expect(score.breakdown.richness).toBeLessThan(20);
        expect(score.issues).toContain("Content is thin (less than 100 characters)");
    });

    it("penalizes missing enrichment", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ summary: null, hasEmbedding: false }));
        // Then
        expect(score.total).toBeLessThan(90);
        expect(score.issues).toContain("Missing AI summary");
        expect(score.issues).toContain("Missing embedding for semantic search");
    });

    it("penalizes orphan chunks (0 connections)", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ connectionCount: 0 }));
        // Then
        expect(score.total).toBeLessThan(90);
        expect(score.breakdown.connectivity).toBe(0);
        expect(score.issues).toContain("Orphan chunk with no connections");
    });

    it("gives base connectivity for 1-2 connections without centrality", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ connectionCount: 2, centralityDegree: 0 }));
        // Then
        expect(score.breakdown.connectivity).toBe(8);
    });

    it("boosts connectivity with high centrality degree", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ connectionCount: 2, centralityDegree: 10 }));
        // Then
        // base 8 + min(floor(10/2), 8) = 8 + 5 = 13
        expect(score.breakdown.connectivity).toBe(13);
    });

    it("gives partial richness for medium content (100-199 chars)", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ content: "A".repeat(150) }));
        // Then
        expect(score.breakdown.richness).toBe(16); // 4 + 6 + 6
    });

    it("keeps freshness at 20 even for very old chunks (100+ days)", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ updatedAt: new Date(Date.now() - 100 * 86400000) }));
        // Then
        expect(score.breakdown.freshness).toBe(20);
    });

    it("gives 0 coverage when no requirements linked", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ requirementCount: 0 }));
        // Then
        expect(score.breakdown.coverage).toBe(0);
        expect(score.issues).toContain("No requirements linked");
    });

    it("gives 10 coverage when requirements linked but not all passing", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ requirementCount: 2, allRequirementsPassing: false }));
        // Then
        expect(score.breakdown.coverage).toBe(10);
    });

    it("gives 15 coverage when all requirements pass but not referenced in session", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ requirementCount: 1, allRequirementsPassing: true, referencedInSession: false }));
        // Then
        expect(score.breakdown.coverage).toBe(15);
    });

    it("gives 20 coverage when all requirements pass and referenced in session", () => {
        // Given the inline inputs and test fixtures.
        // When
        const score = computeHealthScore(makeInput({ requirementCount: 1, allRequirementsPassing: true, referencedInSession: true }));
        // Then
        expect(score.breakdown.coverage).toBe(20);
    });
});
