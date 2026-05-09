import { Effect } from "effect";
import { describe, expect, it, vi } from "vitest";

vi.mock("@fubbik/db/repository", () => ({
    lookupChunksByFilePath: vi.fn(),
    getChunkById: vi.fn(),
    listChunks: vi.fn(),
    getAppliesToForChunks: vi.fn(),
    getRequirementsForChunks: vi.fn(),
    listCodebases: vi.fn(),
    getConnectionsForChunks: vi.fn(),
    getConnectionDegrees: vi.fn().mockReturnValue(Effect.succeed(new Map())),
    getGraphProximityBoost: vi.fn().mockReturnValue(Effect.succeed(new Map())),
    semanticSearch: vi.fn(),
}));

vi.mock("../ollama/client", () => ({
    generateQueryEmbedding: vi.fn(),
}));

import {
    getAppliesToForChunks,
    getConnectionsForChunks,
    listChunks,
    lookupChunksByFilePath,
    getRequirementsForChunks,
    semanticSearch,
} from "@fubbik/db/repository";
import { generateQueryEmbedding } from "../ollama/client";
import { getContextForFile } from "./service";

function makeChunk(id: string, title: string) {
    return {
        id,
        title,
        content: `Content for ${title}`,
        type: "note",
        summary: null,
    };
}

describe("getContextForFile", () => {
    it("uses batch getAppliesToForChunks instead of per-chunk queries", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(
            Effect.succeed({
                chunks: [
                    makeChunk("c1", "Chunk 1"),
                    makeChunk("c2", "Chunk 2"),
                    makeChunk("c3", "Chunk 3"),
                ],
                total: 3,
            })
        );

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(
            Effect.succeed([
                { chunkId: "c1", pattern: "src/**/*.ts", note: null },
                { chunkId: "c3", pattern: "lib/**/*.ts", note: null },
            ])
        );

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.fail(new Error("no ollama")));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/service.ts")
        );

        // Should have called batch function exactly once
        expect(batchMock).toHaveBeenCalledTimes(1);
        expect(batchMock).toHaveBeenCalledWith(["c1", "c2", "c3"]);

        // c1 matches src/**/*.ts, c3 does not match (lib/**/*.ts)
        expect(result.chunks).toHaveLength(1);
        expect(result.chunks[0]!.id).toBe("c1");
        expect(result.chunks[0]!.matchReason).toBe("applies-to");
    });
});

describe("getContextForFile scoring", () => {
    it("returns chunks with score property and sorted by score descending", async () => {
        const now = new Date();

        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(
            Effect.succeed({
                chunks: [
                    {
                        ...makeChunk("c1", "Simple Note"),
                        type: "note",
                        rationale: null,
                        alternatives: null,
                        consequences: null,
                        embedding: null,
                        reviewStatus: null,
                        updatedAt: now,
                    },
                    {
                        ...makeChunk("c2", "Important Doc"),
                        type: "document",
                        rationale: "Well-reasoned",
                        alternatives: null,
                        consequences: null,
                        embedding: null,
                        reviewStatus: "approved",
                        updatedAt: now,
                    },
                ],
                total: 2,
            })
        );

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(
            Effect.succeed([
                { chunkId: "c1", pattern: "src/**/*.ts", note: null },
                { chunkId: "c2", pattern: "src/**/*.ts", note: null },
            ])
        );

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.fail(new Error("no ollama")));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/service.ts")
        );

        expect(result.chunks).toHaveLength(2);

        // Both chunks should have a numeric score
        for (const chunk of result.chunks) {
            expect(typeof chunk.score).toBe("number");
            expect(chunk.score).toBeGreaterThan(0);
        }

        // c2 (document type + rationale + approved review) should score higher than c1 (note, no rationale)
        expect(result.chunks[0]!.id).toBe("c2");
        expect(result.chunks[1]!.id).toBe("c1");
        expect(result.chunks[0]!.score).toBeGreaterThanOrEqual(result.chunks[1]!.score);
    });

    it("gives file-ref matches a higher strategy bonus than applies-to", async () => {
        const now = new Date();
        const baseFields = {
            rationale: null,
            alternatives: null,
            consequences: null,
            embedding: null,
            reviewStatus: null,
            updatedAt: now,
        };

        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(
            Effect.succeed([{ chunkId: "c-ref" }])
        );

        const { getChunkById } = await import("@fubbik/db/repository");
        const getByIdMock = getChunkById as ReturnType<typeof vi.fn>;
        getByIdMock.mockReturnValue(
            Effect.succeed({
                ...makeChunk("c-ref", "File Ref Chunk"),
                ...baseFields,
            })
        );

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(
            Effect.succeed({
                chunks: [
                    {
                        ...makeChunk("c-glob", "Glob Chunk"),
                        ...baseFields,
                    },
                ],
                total: 1,
            })
        );

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(
            Effect.succeed([
                { chunkId: "c-glob", pattern: "src/**/*.ts", note: null },
            ])
        );

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.fail(new Error("no ollama")));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/service.ts")
        );

        expect(result.chunks).toHaveLength(2);

        const fileRefChunk = result.chunks.find(c => c.id === "c-ref")!;
        const globChunk = result.chunks.find(c => c.id === "c-glob")!;

        // file-ref bonus (20) > applies-to bonus (10)
        expect(fileRefChunk.score).toBeGreaterThan(globChunk.score);
        // file-ref chunk should be first (sorted)
        expect(result.chunks[0]!.id).toBe("c-ref");
    });
});

describe("getContextForFile semantic strategy", () => {
    it("adds semantic matches when Ollama is available", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(Effect.succeed({ chunks: [], total: 0 }));

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(Effect.succeed([]));

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.succeed([0.1, 0.2, 0.3]));

        const semanticMock = semanticSearch as ReturnType<typeof vi.fn>;
        semanticMock.mockReturnValue(Effect.succeed([
            { id: "s1", title: "Auth Middleware", type: "document", content: "auth content", summary: null, similarity: 0.85 },
        ]));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/middleware.ts")
        );

        expect(result.chunks).toHaveLength(1);
        expect(result.chunks[0]!.id).toBe("s1");
        expect(result.chunks[0]!.matchReason).toBe("semantic");
    });

    it("skips semantic search silently when Ollama is down", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(Effect.succeed({ chunks: [], total: 0 }));

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(Effect.succeed([]));

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.fail(new Error("Ollama unavailable")));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/middleware.ts")
        );

        expect(result.chunks).toHaveLength(0);
    });
});
