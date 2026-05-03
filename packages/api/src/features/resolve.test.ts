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
        const result = resolveChunk(baseChunk, []);
        expect(result.title).toBe("Base Title");
        expect(result.content).toBe("Base content");
        expect(result._appliedFeatures).toEqual([]);
        expect(result._hasDeltas).toBe(false);
    });

    it("applies a single delta", () => {
        const deltas = [
            { featureId: "f1", delta: { content: "Feature content" }, priority: 1 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.title).toBe("Base Title");
        expect(result.content).toBe("Feature content");
        expect(result._appliedFeatures).toEqual(["f1"]);
        expect(result._hasDeltas).toBe(true);
    });

    it("applies multiple deltas in priority order (higher priority wins)", () => {
        const deltas = [
            { featureId: "f2", delta: { content: "High priority content" }, priority: 10 },
            { featureId: "f1", delta: { content: "Low priority content" }, priority: 1 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.content).toBe("High priority content");
        expect(result._appliedFeatures).toEqual(["f1", "f2"]);
    });

    it("composes non-overlapping field deltas from multiple features", () => {
        const deltas = [
            { featureId: "f1", delta: { content: "New content" }, priority: 1 },
            { featureId: "f2", delta: { title: "New Title" }, priority: 2 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.title).toBe("New Title");
        expect(result.content).toBe("New content");
    });

    it("higher priority overwrites lower priority on same field", () => {
        const deltas = [
            { featureId: "f1", delta: { title: "Low" }, priority: 1 },
            { featureId: "f2", delta: { title: "High" }, priority: 5 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.title).toBe("High");
    });
});

describe("resolveChunks", () => {
    it("returns chunks unchanged when no active feature IDs", () => {
        const chunks = [{ id: "c1", title: "T", content: "C" }];
        const result = resolveChunks(chunks, [], []);
        expect(result).toEqual(chunks.map(c => ({ ...c, _appliedFeatures: [], _hasDeltas: false })));
    });

    it("applies deltas to matching chunks only", () => {
        const chunks = [
            { id: "c1", title: "Chunk 1", content: "Content 1" },
            { id: "c2", title: "Chunk 2", content: "Content 2" }
        ];
        const deltas = [
            { chunkId: "c1", featureId: "f1", delta: { title: "Modified 1" }, priority: 1 }
        ];
        const result = resolveChunks(chunks, ["f1"], deltas);
        expect(result[0]!.title).toBe("Modified 1");
        expect(result[0]!._hasDeltas).toBe(true);
        expect(result[1]!.title).toBe("Chunk 2");
        expect(result[1]!._hasDeltas).toBe(false);
    });
});
