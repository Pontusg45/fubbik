import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

// Mock the repository module
vi.mock("@fubbik/db/repository", () => ({
    createFeature: vi.fn(),
    deleteFeature: vi.fn(),
    featureNameConflict: vi.fn(),
    getActiveFeatureIds: vi.fn(),
    getCodebasesForFeature: vi.fn(),
    getFeatureById: vi.fn(),
    getMaxPriority: vi.fn(),
    listFeatures: vi.fn(),
    setActiveFeatures: vi.fn(),
    setFeatureCodebases: vi.fn(),
    shiftPriorities: vi.fn(),
    updateFeature: vi.fn(),
    batchFetchDeltas: vi.fn(),
    deleteDeltasForFeature: vi.fn(),
    getDeltasForChunk: vi.fn(),
    getDeltasForFeature: vi.fn(),
    mergeFeatureDeltas: vi.fn(),
    upsertDelta: vi.fn(),
    deleteDelta: vi.fn(),
    getChunkById: vi.fn(),
    getNextVersionNumber: vi.fn(),
    createVersion: vi.fn(),
    updateChunk: vi.fn(),
}));

// Mock the enrich service to prevent real Ollama calls
vi.mock("../enrich/service", () => ({
    enrichChunk: vi.fn(() => Effect.succeed(undefined)),
}));

// Mock the logger
vi.mock("../logger", () => ({
    logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

// Import AFTER mock declarations
import {
    createFeature as createFeatureRepo,
    deleteFeature as deleteFeatureRepo,
    getFeatureById,
    getMaxPriority,
    listFeatures as listFeaturesRepo,
    setActiveFeatures as setActiveFeaturesRepo,
    setFeatureCodebases,
    getDeltasForFeature as getDeltasForFeatureRepo,
    mergeFeatureDeltas,
    upsertDelta as upsertDeltaRepo,
    getChunkById,
} from "@fubbik/db/repository";

import {
    createFeature,
    setActiveFeatures,
    mergeFeature,
    deleteFeatureService,
    upsertDelta,
} from "./service";

const userId = "user-1";
const featureId = "feature-1";
const chunkId = "chunk-1";

function mockFeature(overrides?: Record<string, unknown>) {
    return {
        id: featureId,
        name: "My Feature",
        description: null,
        priority: 1,
        status: "inactive",
        color: null,
        userId,
        ...overrides,
    };
}

function mockChunk(id = chunkId) {
    return {
        id,
        title: "Test Chunk",
        content: "Content",
        type: "note",
        userId,
    };
}

beforeEach(() => {
    vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// createFeature
// ---------------------------------------------------------------------------
describe("createFeature", () => {
    it("auto-assigns priority when not provided (calls getMaxPriority)", async () => {
        vi.mocked(getMaxPriority).mockReturnValue(Effect.succeed(3));
        vi.mocked(createFeatureRepo).mockReturnValue(Effect.succeed(mockFeature({ priority: 4 })));

        const result = await Effect.runPromise(
            createFeature(userId, { name: "My Feature" })
        );

        expect(getMaxPriority).toHaveBeenCalledWith(userId);
        expect(createFeatureRepo).toHaveBeenCalledWith(
            expect.objectContaining({ priority: 4, name: "My Feature", userId })
        );
        expect(result).toMatchObject({ priority: 4 });
    });

    it("uses provided priority and skips getMaxPriority", async () => {
        vi.mocked(createFeatureRepo).mockReturnValue(Effect.succeed(mockFeature({ priority: 10 })));

        await Effect.runPromise(
            createFeature(userId, { name: "My Feature", priority: 10 })
        );

        expect(getMaxPriority).not.toHaveBeenCalled();
        expect(createFeatureRepo).toHaveBeenCalledWith(
            expect.objectContaining({ priority: 10 })
        );
    });

    it("sets codebase associations when codebaseIds provided", async () => {
        vi.mocked(getMaxPriority).mockReturnValue(Effect.succeed(0));
        vi.mocked(createFeatureRepo).mockReturnValue(Effect.succeed(mockFeature()));
        vi.mocked(setFeatureCodebases).mockReturnValue(Effect.succeed(undefined as any));

        await Effect.runPromise(
            createFeature(userId, { name: "My Feature", codebaseIds: ["cb-1", "cb-2"] })
        );

        expect(setFeatureCodebases).toHaveBeenCalledWith(
            expect.any(String),
            ["cb-1", "cb-2"]
        );
    });

    it("does not call setFeatureCodebases when codebaseIds is empty", async () => {
        vi.mocked(getMaxPriority).mockReturnValue(Effect.succeed(0));
        vi.mocked(createFeatureRepo).mockReturnValue(Effect.succeed(mockFeature()));

        await Effect.runPromise(
            createFeature(userId, { name: "My Feature", codebaseIds: [] })
        );

        expect(setFeatureCodebases).not.toHaveBeenCalled();
    });
});

// ---------------------------------------------------------------------------
// setActiveFeatures
// ---------------------------------------------------------------------------
describe("setActiveFeatures", () => {
    it("empty array clears active features without validation", async () => {
        vi.mocked(setActiveFeaturesRepo).mockReturnValue(Effect.succeed(undefined as any));

        await Effect.runPromise(setActiveFeatures(userId, []));

        expect(listFeaturesRepo).not.toHaveBeenCalled();
        expect(setActiveFeaturesRepo).toHaveBeenCalledWith(userId, []);
    });

    it("rejects feature IDs that don't belong to the user (ValidationError)", async () => {
        vi.mocked(listFeaturesRepo).mockReturnValue(
            Effect.succeed([mockFeature({ id: "feature-owned" })])
        );

        await expect(
            Effect.runPromise(setActiveFeatures(userId, ["feature-owned", "feature-foreign"]))
        ).rejects.toThrow();

        expect(setActiveFeaturesRepo).not.toHaveBeenCalled();
    });

    it("accepts valid owned feature IDs", async () => {
        vi.mocked(listFeaturesRepo).mockReturnValue(
            Effect.succeed([
                mockFeature({ id: "feature-a" }),
                mockFeature({ id: "feature-b" }),
            ])
        );
        vi.mocked(setActiveFeaturesRepo).mockReturnValue(Effect.succeed(undefined as any));

        await Effect.runPromise(setActiveFeatures(userId, ["feature-a", "feature-b"]));

        expect(setActiveFeaturesRepo).toHaveBeenCalledWith(userId, ["feature-a", "feature-b"]);
    });

    it("rejects when all provided IDs are foreign", async () => {
        vi.mocked(listFeaturesRepo).mockReturnValue(Effect.succeed([]));

        await expect(
            Effect.runPromise(setActiveFeatures(userId, ["foreign-1"]))
        ).rejects.toThrow();
    });
});

// ---------------------------------------------------------------------------
// mergeFeature
// ---------------------------------------------------------------------------
describe("mergeFeature", () => {
    it("fails with ValidationError if feature is already merged", async () => {
        vi.mocked(getFeatureById).mockReturnValue(
            Effect.succeed(mockFeature({ status: "merged" }))
        );

        await expect(
            Effect.runPromise(mergeFeature(featureId, userId))
        ).rejects.toThrow();
    });

    it("merges empty feature (no deltas) — just sets status to merged", async () => {
        vi.mocked(getFeatureById).mockReturnValue(
            Effect.succeed(mockFeature({ status: "active" }))
        );
        vi.mocked(getDeltasForFeatureRepo).mockReturnValue(Effect.succeed([]));
        vi.mocked(mergeFeatureDeltas).mockReturnValue(Effect.succeed([] as any));

        // updateFeature is called when deltas.length === 0
        const { updateFeature: updateFeatureRepo } = await import("@fubbik/db/repository");
        vi.mocked(updateFeatureRepo).mockReturnValue(Effect.succeed(mockFeature({ status: "merged" })));

        await Effect.runPromise(mergeFeature(featureId, userId));

        expect(mergeFeatureDeltas).not.toHaveBeenCalled();
        expect(updateFeatureRepo).toHaveBeenCalledWith(featureId, userId, { status: "merged" });
    });

    it("merges feature with deltas — calls mergeFeatureDeltas", async () => {
        vi.mocked(getFeatureById).mockReturnValue(
            Effect.succeed(mockFeature({ status: "active" }))
        );
        vi.mocked(getDeltasForFeatureRepo).mockReturnValue(
            Effect.succeed([
                { chunkId: "chunk-1", featureId, delta: { title: "New Title" } },
                { chunkId: "chunk-2", featureId, delta: { content: "New Content" } },
            ] as any)
        );
        vi.mocked(mergeFeatureDeltas).mockReturnValue(Effect.succeed(["chunk-1", "chunk-2"]));

        await Effect.runPromise(mergeFeature(featureId, userId));

        expect(mergeFeatureDeltas).toHaveBeenCalledWith(
            featureId,
            userId,
            [
                { chunkId: "chunk-1", delta: { title: "New Title" } },
                { chunkId: "chunk-2", delta: { content: "New Content" } },
            ]
        );
    });

    it("fails with NotFoundError if feature does not exist", async () => {
        vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(null as any));

        await expect(
            Effect.runPromise(mergeFeature(featureId, userId))
        ).rejects.toThrow();
    });
});

// ---------------------------------------------------------------------------
// deleteFeatureService
// ---------------------------------------------------------------------------
describe("deleteFeatureService", () => {
    it("fails with NotFoundError if feature doesn't exist", async () => {
        vi.mocked(deleteFeatureRepo).mockReturnValue(Effect.succeed(null as any));

        await expect(
            Effect.runPromise(deleteFeatureService(featureId, userId))
        ).rejects.toThrow();
    });

    it("succeeds when feature exists", async () => {
        vi.mocked(deleteFeatureRepo).mockReturnValue(
            Effect.succeed(mockFeature() as any)
        );

        const result = await Effect.runPromise(deleteFeatureService(featureId, userId));

        expect(deleteFeatureRepo).toHaveBeenCalledWith(featureId, userId);
        expect(result).toMatchObject({ id: featureId });
    });
});

// ---------------------------------------------------------------------------
// upsertDelta (validates delta, checks feature + chunk existence)
// ---------------------------------------------------------------------------
describe("upsertDelta", () => {
    describe("validateDelta (via upsertDelta)", () => {
        it("rejects empty delta", async () => {
            vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
            vi.mocked(getChunkById).mockReturnValue(Effect.succeed(mockChunk()));

            await expect(
                Effect.runPromise(upsertDelta(chunkId, featureId, userId, {}))
            ).rejects.toThrow();
        });

        it("rejects invalid fields (tags)", async () => {
            vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
            vi.mocked(getChunkById).mockReturnValue(Effect.succeed(mockChunk()));

            await expect(
                Effect.runPromise(upsertDelta(chunkId, featureId, userId, { tags: ["foo"] } as any))
            ).rejects.toThrow();
        });

        it("rejects invalid fields (embedding)", async () => {
            vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
            vi.mocked(getChunkById).mockReturnValue(Effect.succeed(mockChunk()));

            await expect(
                Effect.runPromise(upsertDelta(chunkId, featureId, userId, { embedding: "vec" } as any))
            ).rejects.toThrow();
        });

        it("accepts all valid allowed fields", async () => {
            vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
            vi.mocked(getChunkById).mockReturnValue(Effect.succeed(mockChunk()));
            vi.mocked(upsertDeltaRepo).mockReturnValue(Effect.succeed({ id: "delta-1" } as any));

            const validDelta = {
                title: "New Title",
                content: "New content",
                type: "document",
                rationale: "Because",
                alternatives: "Alt",
                consequences: "Cons",
                summary: "Sum",
            };

            await Effect.runPromise(upsertDelta(chunkId, featureId, userId, validDelta));

            expect(upsertDeltaRepo).toHaveBeenCalledWith(
                expect.objectContaining({ chunkId, featureId, delta: validDelta })
            );
        });

        it("accepts a subset of valid fields", async () => {
            vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
            vi.mocked(getChunkById).mockReturnValue(Effect.succeed(mockChunk()));
            vi.mocked(upsertDeltaRepo).mockReturnValue(Effect.succeed({ id: "delta-1" } as any));

            await Effect.runPromise(
                upsertDelta(chunkId, featureId, userId, { title: "Just a title" })
            );

            expect(upsertDeltaRepo).toHaveBeenCalledWith(
                expect.objectContaining({ delta: { title: "Just a title" } })
            );
        });
    });

    it("fails with NotFoundError if feature doesn't exist", async () => {
        vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(null as any));

        await expect(
            Effect.runPromise(upsertDelta(chunkId, featureId, userId, { title: "X" }))
        ).rejects.toThrow();

        expect(getChunkById).not.toHaveBeenCalled();
        expect(upsertDeltaRepo).not.toHaveBeenCalled();
    });

    it("fails with NotFoundError if chunk doesn't exist", async () => {
        vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
        vi.mocked(getChunkById).mockReturnValue(Effect.succeed(null as any));

        await expect(
            Effect.runPromise(upsertDelta(chunkId, featureId, userId, { title: "X" }))
        ).rejects.toThrow();

        expect(upsertDeltaRepo).not.toHaveBeenCalled();
    });

    it("succeeds when both feature and chunk exist and delta is valid", async () => {
        vi.mocked(getFeatureById).mockReturnValue(Effect.succeed(mockFeature()));
        vi.mocked(getChunkById).mockReturnValue(Effect.succeed(mockChunk()));
        vi.mocked(upsertDeltaRepo).mockReturnValue(Effect.succeed({ id: "delta-1" } as any));

        const result = await Effect.runPromise(
            upsertDelta(chunkId, featureId, userId, { content: "Updated" })
        );

        expect(upsertDeltaRepo).toHaveBeenCalledTimes(1);
        expect(result).toMatchObject({ id: "delta-1" });
    });
});
