import { Effect } from "effect";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DatabaseError } from "../errors";

// ---------------------------------------------------------------------------
// Mock db + dbEffect so we don't need a live database
// ---------------------------------------------------------------------------

const mockSelect = vi.fn();
const mockFrom = vi.fn();
const mockWhere = vi.fn();
const mockGroupBy = vi.fn();
const mockInnerJoin = vi.fn();
const mockOrderBy = vi.fn();
const mockLimit = vi.fn();
const mockOffset = vi.fn();

const chainMethods = {
    select: mockSelect,
    from: mockFrom,
    where: mockWhere,
    groupBy: mockGroupBy,
    innerJoin: mockInnerJoin,
    orderBy: mockOrderBy,
    limit: mockLimit,
    offset: mockOffset,
};

// Each chain method returns the chain so they're composable
for (const fn of Object.values(chainMethods)) {
    fn.mockReturnValue(chainMethods);
}

vi.mock("../index", () => ({
    db: chainMethods,
    dbEffect: <T>(fn: () => Promise<T>) =>
        Effect.tryPromise({
            try: fn,
            catch: (cause) => new DatabaseError({ cause }),
        }),
}));

// Also mock the AGE sync module since chunk.ts schema imports may reference it indirectly
vi.mock("../age/sync", () => ({
    ensureVertex: vi.fn(),
    deleteVertex: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Import after mocks are set up
// ---------------------------------------------------------------------------

const { getGroupedCounts, getChunksInGroup } = await import("./chunk-groups");

describe("getGroupedCounts", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        // Re-set chain returns after clearAllMocks
        for (const fn of Object.values(chainMethods)) {
            fn.mockReturnValue(chainMethods);
        }
    });

    it("returns grouped counts for groupBy: type", async () => {
        const mockData = [
            { groupName: "note", count: 5 },
            { groupName: "document", count: 3 },
        ];
        // The terminal call in the chain resolves to mockData
        mockGroupBy.mockResolvedValueOnce(mockData);

        const result = await Effect.runPromise(
            getGroupedCounts({
                groupBy: "type",
                userId: "user-1",
            })
        );

        expect(result).toEqual(mockData);
        expect(result).toHaveLength(2);
        expect(result[0]).toHaveProperty("groupName");
        expect(result[0]).toHaveProperty("count");
        expect(mockSelect).toHaveBeenCalled();
        expect(mockFrom).toHaveBeenCalled();
    });

    it("returns grouped counts for groupBy: status", async () => {
        const mockData = [
            { groupName: "approved", count: 10 },
            { groupName: "draft", count: 2 },
        ];
        mockGroupBy.mockResolvedValueOnce(mockData);

        const result = await Effect.runPromise(
            getGroupedCounts({
                groupBy: "status",
                userId: "user-1",
            })
        );

        expect(result).toEqual(mockData);
    });

    it("returns grouped counts for groupBy: origin", async () => {
        const mockData = [
            { groupName: "human", count: 7 },
            { groupName: "ai", count: 4 },
        ];
        mockGroupBy.mockResolvedValueOnce(mockData);

        const result = await Effect.runPromise(
            getGroupedCounts({
                groupBy: "origin",
                userId: "user-1",
            })
        );

        expect(result).toEqual(mockData);
    });

    it("returns grouped counts for groupBy: freshness", async () => {
        const mockData = [
            { groupName: "This week", count: 3 },
            { groupName: "This month", count: 5 },
            { groupName: "Last 3 months", count: 8 },
            { groupName: "Older", count: 2 },
        ];
        mockGroupBy.mockResolvedValueOnce(mockData);

        const result = await Effect.runPromise(
            getGroupedCounts({
                groupBy: "freshness",
                userId: "user-1",
            })
        );

        expect(result).toEqual(mockData);
    });

    it("returns grouped counts for groupBy: tagtype with tagTypeId", async () => {
        const mockData = [
            { groupName: "frontend", count: 4 },
            { groupName: "backend", count: 6 },
        ];
        mockGroupBy.mockResolvedValueOnce(mockData);

        const result = await Effect.runPromise(
            getGroupedCounts({
                groupBy: "tagtype",
                tagTypeId: "tag-type-1",
                userId: "user-1",
            })
        );

        expect(result).toEqual(mockData);
    });

    it("returns an empty array when no data", async () => {
        mockGroupBy.mockResolvedValueOnce([]);

        const result = await Effect.runPromise(
            getGroupedCounts({
                groupBy: "type",
                userId: "user-1",
            })
        );

        expect(result).toEqual([]);
    });

    it("wraps database errors as DatabaseError", async () => {
        mockGroupBy.mockRejectedValueOnce(new Error("connection refused"));

        const result = await Effect.runPromiseExit(
            getGroupedCounts({
                groupBy: "type",
                userId: "user-1",
            })
        );

        expect(result._tag).toBe("Failure");
    });
});

describe("getChunksInGroup", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        for (const fn of Object.values(chainMethods)) {
            fn.mockReturnValue(chainMethods);
        }
    });

    it("returns chunks and total for a type group", async () => {
        const mockChunks = [
            { id: "c1", title: "Chunk 1", type: "note" },
            { id: "c2", title: "Chunk 2", type: "note" },
        ];

        // getChunksInGroup performs two sequential queries:
        // 1. select().from().where().orderBy().limit().offset() -> chunks
        // 2. select().from().where() -> [{ count }]
        // Since both start from db.select(), we chain the calls:
        // First offset() call returns the chunks
        mockOffset.mockResolvedValueOnce(mockChunks);
        // After the first query completes, the second query starts with
        // select().from().where() — the where() call must resolve with count.
        // But since the chain is shared, we override where to return differently
        // on the second pass by making it resolve on the 4th call (3 from first query + 1 from second).
        // Instead, use a counter approach:
        let whereCallCount = 0;
        mockWhere.mockImplementation((..._args: unknown[]) => {
            whereCallCount++;
            // The second call to where() (from the count query) should resolve to the count
            if (whereCallCount === 2) {
                return Promise.resolve([{ count: 5 }]);
            }
            return chainMethods;
        });

        const result = await Effect.runPromise(
            getChunksInGroup({
                groupBy: "type",
                groupName: "note",
                userId: "user-1",
                limit: 20,
                offset: 0,
            })
        );

        expect(result).toHaveProperty("chunks");
        expect(result).toHaveProperty("total");
        expect(result.chunks).toEqual(mockChunks);
        expect(result.total).toBe(5);
    });

    it("wraps database errors as DatabaseError", async () => {
        // Make the first query (offset) throw
        mockOffset.mockRejectedValueOnce(new Error("connection refused"));

        const result = await Effect.runPromiseExit(
            getChunksInGroup({
                groupBy: "type",
                groupName: "note",
                userId: "user-1",
                limit: 20,
                offset: 0,
            })
        );

        expect(result._tag).toBe("Failure");
    });
});
