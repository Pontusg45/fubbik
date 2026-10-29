import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

// Mock the repository module
vi.mock("@fubbik/db/repository", () => ({
    getGroupedCounts: vi.fn(),
    getChunksInGroup: vi.fn()
}));

import { getGroupedCounts, getChunksInGroup } from "@fubbik/db/repository";

import { listGroupedCounts, listGroupChunks } from "./group-service";

const userId = "user-1";

beforeEach(() => {
    vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// listGroupedCounts
// ---------------------------------------------------------------------------
describe("listGroupedCounts", () => {
    it("passes groupBy='type' and userId to repository", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(
            Effect.succeed([
                { groupName: "note", count: 5 },
                { groupName: "document", count: 3 }
            ])
        );

        // When
        const result = await Effect.runPromise(listGroupedCounts(userId, { groupBy: "type" }));

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(expect.objectContaining({ groupBy: "type", userId }));
        expect(result).toEqual({
            groups: [
                { groupName: "note", count: 5 },
                { groupName: "document", count: 3 }
            ],
            totalGroups: 2
        });
    });

    it("parses CSV tags into array", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        // When
        await Effect.runPromise(listGroupedCounts(userId, { groupBy: "type", tags: "foo,bar,baz" }));

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(expect.objectContaining({ tags: ["foo", "bar", "baz"] }));
    });

    it("parses global string to boolean", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        // When
        await Effect.runPromise(listGroupedCounts(userId, { groupBy: "status", global: "true" }));

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(expect.objectContaining({ globalOnly: true }));
    });

    it("defaults groupBy to 'type' for unknown values", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        // When
        await Effect.runPromise(listGroupedCounts(userId, { groupBy: "unknown" as any }));

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(expect.objectContaining({ groupBy: "type" }));
    });

    it("recognises 'tagtype:abc' format and extracts tagTypeId", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        // When
        await Effect.runPromise(listGroupedCounts(userId, { groupBy: "tagtype:abc" as any }));

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(expect.objectContaining({ groupBy: "tagtype", tagTypeId: "abc" }));
    });

    it("passes through codebaseId and workspaceId", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        // When
        await Effect.runPromise(
            listGroupedCounts(userId, {
                groupBy: "type",
                codebaseId: "cb-1",
                workspaceId: "ws-1"
            })
        );

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({
                codebaseId: "cb-1",
                workspaceId: "ws-1"
            })
        );
    });

    it("passes filter params: type, origin, reviewStatus, tagMode", async () => {
        // Given
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        // When
        await Effect.runPromise(
            listGroupedCounts(userId, {
                groupBy: "freshness",
                type: "document",
                origin: "ai",
                reviewStatus: "approved",
                tagMode: "all",
                tags: "a,b"
            })
        );

        // Then
        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({
                type: "document",
                origin: "ai",
                reviewStatus: "approved",
                tagMode: "all",
                tags: ["a", "b"]
            })
        );
    });
});

// ---------------------------------------------------------------------------
// listGroupChunks
// ---------------------------------------------------------------------------
describe("listGroupChunks", () => {
    it("calls repository with parsed params and returns paginated result", async () => {
        // Given
        const mockChunks = [
            { id: "c-1", title: "Chunk 1" },
            { id: "c-2", title: "Chunk 2" }
        ];
        vi.mocked(getChunksInGroup).mockReturnValue(Effect.succeed({ chunks: mockChunks, total: 10 }) as any);

        // When
        const result = await Effect.runPromise(
            listGroupChunks(userId, "note", {
                groupBy: "type",
                limit: "20",
                offset: "5"
            })
        );

        // Then
        expect(getChunksInGroup).toHaveBeenCalledWith(
            expect.objectContaining({
                groupBy: "type",
                groupName: "note",
                userId,
                limit: 20,
                offset: 5
            })
        );
        expect(result).toEqual({
            chunks: mockChunks,
            total: 10,
            limit: 20,
            offset: 5
        });
    });

    it("defaults limit to 50 and offset to 0", async () => {
        // Given
        vi.mocked(getChunksInGroup).mockReturnValue(Effect.succeed({ chunks: [], total: 0 }) as any);

        // When
        const result = await Effect.runPromise(listGroupChunks(userId, "ai", { groupBy: "origin" }));

        // Then
        expect(getChunksInGroup).toHaveBeenCalledWith(expect.objectContaining({ limit: 50, offset: 0 }));
        expect(result).toEqual({ chunks: [], total: 0, limit: 50, offset: 0 });
    });

    it("caps limit at 100", async () => {
        // Given
        vi.mocked(getChunksInGroup).mockReturnValue(Effect.succeed({ chunks: [], total: 0 }) as any);

        // When
        await Effect.runPromise(listGroupChunks(userId, "note", { groupBy: "type", limit: "500" }));

        // Then
        expect(getChunksInGroup).toHaveBeenCalledWith(expect.objectContaining({ limit: 100 }));
    });

    it("parses CSV tags and passes filter params", async () => {
        // Given
        vi.mocked(getChunksInGroup).mockReturnValue(Effect.succeed({ chunks: [], total: 0 }) as any);

        // When
        await Effect.runPromise(
            listGroupChunks(userId, "This week", {
                groupBy: "freshness",
                tags: "alpha,beta",
                tagMode: "all",
                codebaseId: "cb-1",
                global: "false"
            })
        );

        // Then
        expect(getChunksInGroup).toHaveBeenCalledWith(
            expect.objectContaining({
                tags: ["alpha", "beta"],
                tagMode: "all",
                codebaseId: "cb-1",
                globalOnly: false
            })
        );
    });
});
