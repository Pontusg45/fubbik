import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { bulkUpdate } from "./bulk-service";

// Mock the repository module
vi.mock("@fubbik/db/repository", () => ({
    getChunkById: vi.fn(),
    findOrCreateTag: vi.fn(),
    getTagsForChunks: vi.fn(),
    setChunkTags: vi.fn(),
    updateManyChunks: vi.fn(),
    setChunkCodebases: vi.fn(),
    archiveMany: vi.fn(),
    deleteMany: vi.fn(),
}));

import {
    getChunkById,
    findOrCreateTag,
    getTagsForChunks,
    setChunkTags,
    updateManyChunks,
    setChunkCodebases,
    archiveMany,
    deleteMany,
} from "@fubbik/db/repository";

const userId = "user-1";
const chunkIds = ["chunk-1", "chunk-2"];

function mockChunk(id: string) {
    return { id, title: "Test", userId };
}

beforeEach(() => {
    vi.clearAllMocks();
    // Default: ownership check passes
    vi.mocked(getChunkById).mockImplementation((id: string) =>
        Effect.succeed(mockChunk(id))
    );
});

describe("bulkUpdate", () => {
    describe("ownership validation", () => {
        it("fails with AuthError when a chunk is not found", async () => {
            vi.mocked(getChunkById).mockImplementation((id: string) =>
                id === "chunk-2" ? Effect.succeed(null as any) : Effect.succeed(mockChunk(id))
            );

            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "set_type", value: "note" }))
            ).rejects.toThrow();
        });
    });

    describe("add_tags", () => {
        it("adds tags to chunks, merging with existing", async () => {
            vi.mocked(findOrCreateTag).mockImplementation((name: string) =>
                Effect.succeed({ id: `tag-${name}`, name })
            );
            vi.mocked(getTagsForChunks).mockReturnValue(
                Effect.succeed([
                    { chunkId: "chunk-1", tagId: "tag-existing" },
                ])
            );
            vi.mocked(setChunkTags).mockReturnValue(Effect.succeed(undefined as any));

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "add_tags", value: "foo, bar" })
            );

            expect(result).toEqual({ updated: 2 });
            expect(findOrCreateTag).toHaveBeenCalledTimes(2);
            expect(findOrCreateTag).toHaveBeenCalledWith("foo", userId);
            expect(findOrCreateTag).toHaveBeenCalledWith("bar", userId);
            expect(setChunkTags).toHaveBeenCalledTimes(2);
            // chunk-1 should have existing + new tags
            const chunk1Call = vi.mocked(setChunkTags).mock.calls.find(c => c[0] === "chunk-1");
            expect(chunk1Call![1]).toContain("tag-existing");
            expect(chunk1Call![1]).toContain("tag-foo");
            expect(chunk1Call![1]).toContain("tag-bar");
        });

        it("fails with ValidationError when value is missing", async () => {
            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "add_tags" }))
            ).rejects.toThrow();
        });
    });

    describe("remove_tags", () => {
        it("removes specified tags from chunks", async () => {
            vi.mocked(findOrCreateTag).mockImplementation((name: string) =>
                Effect.succeed({ id: `tag-${name}`, name })
            );
            vi.mocked(getTagsForChunks).mockReturnValue(
                Effect.succeed([
                    { chunkId: "chunk-1", tagId: "tag-foo" },
                    { chunkId: "chunk-1", tagId: "tag-keep" },
                    { chunkId: "chunk-2", tagId: "tag-foo" },
                ])
            );
            vi.mocked(setChunkTags).mockReturnValue(Effect.succeed(undefined as any));

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "remove_tags", value: "foo" })
            );

            expect(result).toEqual({ updated: 2 });
            // chunk-1: keep only "tag-keep"
            const chunk1Call = vi.mocked(setChunkTags).mock.calls.find(c => c[0] === "chunk-1");
            expect(chunk1Call![1]).toEqual(["tag-keep"]);
            // chunk-2: no tags left
            const chunk2Call = vi.mocked(setChunkTags).mock.calls.find(c => c[0] === "chunk-2");
            expect(chunk2Call![1]).toEqual([]);
        });

        it("fails with ValidationError when value is missing", async () => {
            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "remove_tags" }))
            ).rejects.toThrow();
        });
    });

    describe("set_type", () => {
        it("updates type on all chunks", async () => {
            vi.mocked(updateManyChunks).mockReturnValue(
                Effect.succeed([{ id: "chunk-1" }, { id: "chunk-2" }] as any)
            );

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "set_type", value: "document" })
            );

            expect(result).toEqual({ updated: 2 });
            expect(updateManyChunks).toHaveBeenCalledWith(chunkIds, userId, { type: "document" });
        });

        it("fails with ValidationError when value is missing", async () => {
            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "set_type" }))
            ).rejects.toThrow();
        });
    });

    describe("set_codebase", () => {
        it("sets codebase on all chunks", async () => {
            vi.mocked(setChunkCodebases).mockReturnValue(Effect.succeed(undefined as any));

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "set_codebase", value: "codebase-1" })
            );

            expect(result).toEqual({ updated: 2 });
            expect(setChunkCodebases).toHaveBeenCalledWith("chunk-1", ["codebase-1"]);
            expect(setChunkCodebases).toHaveBeenCalledWith("chunk-2", ["codebase-1"]);
        });

        it("clears codebase when value is null", async () => {
            vi.mocked(setChunkCodebases).mockReturnValue(Effect.succeed(undefined as any));

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "set_codebase", value: null })
            );

            expect(result).toEqual({ updated: 2 });
            expect(setChunkCodebases).toHaveBeenCalledWith("chunk-1", []);
            expect(setChunkCodebases).toHaveBeenCalledWith("chunk-2", []);
        });
    });

    describe("set_review_status", () => {
        it("updates review status on all chunks", async () => {
            vi.mocked(updateManyChunks).mockReturnValue(
                Effect.succeed([{ id: "chunk-1" }, { id: "chunk-2" }] as any)
            );

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "set_review_status", value: "approved" })
            );

            expect(result).toEqual({ updated: 2 });
            expect(updateManyChunks).toHaveBeenCalledWith(chunkIds, userId, { reviewStatus: "approved" });
        });

        it("fails with ValidationError for invalid review status", async () => {
            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "set_review_status", value: "invalid" }))
            ).rejects.toThrow();
        });

        it("fails with ValidationError when value is missing", async () => {
            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "set_review_status" }))
            ).rejects.toThrow();
        });
    });

    describe("archive", () => {
        it("archives all chunks", async () => {
            vi.mocked(archiveMany).mockReturnValue(
                Effect.succeed([{ id: "chunk-1" }, { id: "chunk-2" }] as any)
            );

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "archive" })
            );

            expect(result).toEqual({ updated: 2 });
            expect(archiveMany).toHaveBeenCalledWith(chunkIds, userId);
        });
    });

    describe("delete", () => {
        it("deletes all chunks", async () => {
            vi.mocked(deleteMany).mockReturnValue(
                Effect.succeed([{ id: "chunk-1" }, { id: "chunk-2" }] as any)
            );

            const result = await Effect.runPromise(
                bulkUpdate(userId, { ids: chunkIds, action: "delete" })
            );

            expect(result).toEqual({ updated: 2 });
            expect(deleteMany).toHaveBeenCalledWith(chunkIds, userId);
        });
    });

    describe("unknown action", () => {
        it("fails with ValidationError", async () => {
            await expect(
                Effect.runPromise(bulkUpdate(userId, { ids: chunkIds, action: "nope" as any }))
            ).rejects.toThrow();
        });
    });
});
