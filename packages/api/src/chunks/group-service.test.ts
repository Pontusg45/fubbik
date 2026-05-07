import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

// Mock the repository module
vi.mock("@fubbik/db/repository", () => ({
    getGroupedCounts: vi.fn(),
    getChunksInGroup: vi.fn(),
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
        vi.mocked(getGroupedCounts).mockReturnValue(
            Effect.succeed([
                { groupName: "note", count: 5 },
                { groupName: "document", count: 3 },
            ])
        );

        const result = await Effect.runPromise(
            listGroupedCounts(userId, { groupBy: "type" })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({ groupBy: "type", userId })
        );
        expect(result).toEqual({
            groups: [
                { groupName: "note", count: 5 },
                { groupName: "document", count: 3 },
            ],
            totalGroups: 2,
        });
    });

    it("parses CSV tags into array", async () => {
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        await Effect.runPromise(
            listGroupedCounts(userId, { groupBy: "type", tags: "foo,bar,baz" })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({ tags: ["foo", "bar", "baz"] })
        );
    });

    it("parses global string to boolean", async () => {
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        await Effect.runPromise(
            listGroupedCounts(userId, { groupBy: "status", global: "true" })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({ globalOnly: true })
        );
    });

    it("defaults groupBy to 'type' for unknown values", async () => {
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        await Effect.runPromise(
            listGroupedCounts(userId, { groupBy: "unknown" as any })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({ groupBy: "type" })
        );
    });

    it("recognises 'tagtype:abc' format and extracts tagTypeId", async () => {
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        await Effect.runPromise(
            listGroupedCounts(userId, { groupBy: "tagtype:abc" as any })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({ groupBy: "tagtype", tagTypeId: "abc" })
        );
    });

    it("passes through codebaseId and workspaceId", async () => {
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        await Effect.runPromise(
            listGroupedCounts(userId, {
                groupBy: "type",
                codebaseId: "cb-1",
                workspaceId: "ws-1",
            })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({
                codebaseId: "cb-1",
                workspaceId: "ws-1",
            })
        );
    });

    it("passes filter params: type, origin, reviewStatus, tagMode", async () => {
        vi.mocked(getGroupedCounts).mockReturnValue(Effect.succeed([]));

        await Effect.runPromise(
            listGroupedCounts(userId, {
                groupBy: "freshness",
                type: "document",
                origin: "ai",
                reviewStatus: "approved",
                tagMode: "all",
                tags: "a,b",
            })
        );

        expect(getGroupedCounts).toHaveBeenCalledWith(
            expect.objectContaining({
                type: "document",
                origin: "ai",
                reviewStatus: "approved",
                tagMode: "all",
                tags: ["a", "b"],
            })
        );
    });
});

// ---------------------------------------------------------------------------
// listGroupChunks
// ---------------------------------------------------------------------------
describe("listGroupChunks", () => {
    it("calls repository with parsed params and returns paginated result", async () => {
        const mockChunks = [
            { id: "c-1", title: "Chunk 1" },
            { id: "c-2", title: "Chunk 2" },
        ];
        vi.mocked(getChunksInGroup).mockReturnValue(
            Effect.succeed({ chunks: mockChunks, total: 10 }) as any
        );

        const result = await Effect.runPromise(
            listGroupChunks(userId, "note", {
                groupBy: "type",
                limit: "20",
                offset: "5",
            })
        );

        expect(getChunksInGroup).toHaveBeenCalledWith(
            expect.objectContaining({
                groupBy: "type",
                groupName: "note",
                userId,
                limit: 20,
                offset: 5,
            })
        );
        expect(result).toEqual({
            chunks: mockChunks,
            total: 10,
            limit: 20,
            offset: 5,
        });
    });

    it("defaults limit to 50 and offset to 0", async () => {
        vi.mocked(getChunksInGroup).mockReturnValue(
            Effect.succeed({ chunks: [], total: 0 }) as any
        );

        const result = await Effect.runPromise(
            listGroupChunks(userId, "ai", { groupBy: "origin" })
        );

        expect(getChunksInGroup).toHaveBeenCalledWith(
            expect.objectContaining({ limit: 50, offset: 0 })
        );
        expect(result).toEqual({ chunks: [], total: 0, limit: 50, offset: 0 });
    });

    it("caps limit at 100", async () => {
        vi.mocked(getChunksInGroup).mockReturnValue(
            Effect.succeed({ chunks: [], total: 0 }) as any
        );

        await Effect.runPromise(
            listGroupChunks(userId, "note", { groupBy: "type", limit: "500" })
        );

        expect(getChunksInGroup).toHaveBeenCalledWith(
            expect.objectContaining({ limit: 100 })
        );
    });

    it("parses CSV tags and passes filter params", async () => {
        vi.mocked(getChunksInGroup).mockReturnValue(
            Effect.succeed({ chunks: [], total: 0 }) as any
        );

        await Effect.runPromise(
            listGroupChunks(userId, "This week", {
                groupBy: "freshness",
                tags: "alpha,beta",
                tagMode: "all",
                codebaseId: "cb-1",
                global: "false",
            })
        );

        expect(getChunksInGroup).toHaveBeenCalledWith(
            expect.objectContaining({
                tags: ["alpha", "beta"],
                tagMode: "all",
                codebaseId: "cb-1",
                globalOnly: false,
            })
        );
    });
});
