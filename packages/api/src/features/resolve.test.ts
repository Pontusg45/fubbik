import { describe, it, expect } from "vitest";

import { resolveChunk, resolveChunks } from "./resolve";

describe("resolveChunk", () => {
    const baseChunk = {
        id: "chunk-1",
        title: "Base Title",
        content: "Base content",
        type: "note",
        rationale: null,
        summary: null
    };

    it("returns base chunk unchanged when no deltas", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = resolveChunk(baseChunk, []);
        // Then
        expect(result.title).toBe("Base Title");
        expect(result.content).toBe("Base content");
        expect(result._appliedFeatures).toEqual([]);
        expect(result._hasDeltas).toBe(false);
    });

    it("applies a single delta", () => {
        // Given
        const deltas = [{ featureId: "f1", delta: { content: "Feature content" }, priority: 1 }];
        // When
        const result = resolveChunk(baseChunk, deltas);
        // Then
        expect(result.title).toBe("Base Title");
        expect(result.content).toBe("Feature content");
        expect(result._appliedFeatures).toEqual(["f1"]);
        expect(result._hasDeltas).toBe(true);
    });

    it("applies multiple deltas in priority order (higher priority wins)", () => {
        // Given
        const deltas = [
            { featureId: "f2", delta: { content: "High priority content" }, priority: 10 },
            { featureId: "f1", delta: { content: "Low priority content" }, priority: 1 }
        ];
        // When
        const result = resolveChunk(baseChunk, deltas);
        // Then
        expect(result.content).toBe("High priority content");
        expect(result._appliedFeatures).toEqual(["f1", "f2"]);
    });

    it("composes non-overlapping field deltas from multiple features", () => {
        // Given
        const deltas = [
            { featureId: "f1", delta: { content: "New content" }, priority: 1 },
            { featureId: "f2", delta: { title: "New Title" }, priority: 2 }
        ];
        // When
        const result = resolveChunk(baseChunk, deltas);
        // Then
        expect(result.title).toBe("New Title");
        expect(result.content).toBe("New content");
    });

    it("higher priority overwrites lower priority on same field", () => {
        // Given
        const deltas = [
            { featureId: "f1", delta: { title: "Low" }, priority: 1 },
            { featureId: "f2", delta: { title: "High" }, priority: 5 }
        ];
        // When
        const result = resolveChunk(baseChunk, deltas);
        // Then
        expect(result.title).toBe("High");
    });
});

describe("resolveChunks", () => {
    it("returns chunks unchanged when no active feature IDs", () => {
        // Given
        const chunks = [{ id: "c1", title: "T", content: "C" }];
        // When
        const result = resolveChunks(chunks, [], []);
        // Then
        expect(result).toEqual(chunks.map(c => ({ ...c, _appliedFeatures: [], _hasDeltas: false })));
    });

    it("applies deltas to matching chunks only", () => {
        // Given
        const chunks = [
            { id: "c1", title: "Chunk 1", content: "Content 1" },
            { id: "c2", title: "Chunk 2", content: "Content 2" }
        ];
        const deltas = [{ chunkId: "c1", featureId: "f1", delta: { title: "Modified 1" }, priority: 1 }];
        // When
        const result = resolveChunks(chunks, ["f1"], deltas);
        // Then
        expect(result[0]!.title).toBe("Modified 1");
        expect(result[0]!._hasDeltas).toBe(true);
        expect(result[1]!.title).toBe("Chunk 2");
        expect(result[1]!._hasDeltas).toBe(false);
    });
});
