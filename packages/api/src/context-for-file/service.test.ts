import { Effect } from "effect";
import { describe, expect, it, vi } from "vitest";

vi.mock("@fubbik/db/repository", () => ({
    lookupChunksByFilePath: vi.fn(),
    getChunkById: vi.fn(),
    listChunks: vi.fn(),
    getAppliesToForChunks: vi.fn(),
    getRequirementsForChunks: vi.fn(),
    listCodebases: vi.fn(),
}));

import {
    getAppliesToForChunks,
    listChunks,
    lookupChunksByFilePath,
    getRequirementsForChunks,
} from "@fubbik/db/repository";
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
