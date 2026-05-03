import { getTableColumns } from "drizzle-orm";
import { eq } from "drizzle-orm";
import { Effect } from "effect";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { db } from "../index";
import * as chunkDeltaRepo from "../repository/chunk-feature-delta";
import * as featureRepo from "../repository/feature";
import { user } from "../schema/auth";
import { chunk } from "../schema/chunk";
import { chunkFeatureDelta, feature, featureCodebase, userActiveFeature } from "../schema/feature";

// ---------------------------------------------------------------------------
// 1. Schema validation
// ---------------------------------------------------------------------------

describe("feature table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(feature);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("name");
        expect(columns).toHaveProperty("description");
        expect(columns).toHaveProperty("priority");
        expect(columns).toHaveProperty("status");
        expect(columns).toHaveProperty("color");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("featureCodebase table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(featureCodebase);
        expect(columns).toHaveProperty("featureId");
        expect(columns).toHaveProperty("codebaseId");
    });
});

describe("chunkFeatureDelta table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkFeatureDelta);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("featureId");
        expect(columns).toHaveProperty("delta");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("userActiveFeature table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(userActiveFeature);
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("featureId");
    });
});

// ---------------------------------------------------------------------------
// Shared DB test setup helpers
// ---------------------------------------------------------------------------

async function createTestUser(suffix?: string) {
    const id = crypto.randomUUID();
    await db.insert(user).values({
        id,
        email: `test-${suffix ?? Date.now()}@example.com`,
        name: "Test",
        emailVerified: false,
        createdAt: new Date(),
        updatedAt: new Date(),
    });
    return id;
}

async function createTestFeature(userId: string, overrides?: Partial<{ name: string; priority: number; color: string }>) {
    const id = crypto.randomUUID();
    const created = await Effect.runPromise(
        featureRepo.createFeature({
            id,
            name: overrides?.name ?? `Feature-${id.slice(0, 8)}`,
            priority: overrides?.priority ?? 1,
            color: overrides?.color,
            userId,
        }),
    );
    return created;
}

async function createTestChunk(userId: string) {
    const id = crypto.randomUUID();
    await db.insert(chunk).values({
        id,
        title: "Test Chunk",
        content: "content",
        type: "note",
        userId,
    });
    return id;
}

// ---------------------------------------------------------------------------
// 2. Feature CRUD
// ---------------------------------------------------------------------------

describe("Feature CRUD", () => {
    let testUserId: string;

    beforeEach(async () => {
        testUserId = await createTestUser();
    });

    afterEach(async () => {
        await db.delete(user).where(eq(user.id, testUserId));
    });

    it("creates a feature and reads it back", async () => {
        const created = await createTestFeature(testUserId, { name: "My Feature", priority: 1 });

        expect(created.id).toBeDefined();
        expect(created.name).toBe("My Feature");
        expect(created.userId).toBe(testUserId);
        expect(created.status).toBe("inactive");
        expect(created.priority).toBe(1);

        const found = await Effect.runPromise(featureRepo.getFeatureById(created.id, testUserId));
        expect(found).not.toBeNull();
        expect(found!.id).toBe(created.id);
        expect(found!.name).toBe("My Feature");
    });

    it("returns null for non-existent feature", async () => {
        const found = await Effect.runPromise(featureRepo.getFeatureById("does-not-exist", testUserId));
        expect(found).toBeNull();
    });

    it("updates a feature", async () => {
        const created = await createTestFeature(testUserId, { name: "Original", priority: 1 });

        const updated = await Effect.runPromise(
            featureRepo.updateFeature(created.id, testUserId, {
                name: "Updated",
                status: "active",
                color: "#ff0000",
            }),
        );

        expect(updated).not.toBeNull();
        expect(updated!.name).toBe("Updated");
        expect(updated!.status).toBe("active");
        expect(updated!.color).toBe("#ff0000");
    });

    it("deletes a feature", async () => {
        const created = await createTestFeature(testUserId, { priority: 1 });

        const deleted = await Effect.runPromise(featureRepo.deleteFeature(created.id, testUserId));
        expect(deleted).not.toBeNull();
        expect(deleted!.id).toBe(created.id);

        const found = await Effect.runPromise(featureRepo.getFeatureById(created.id, testUserId));
        expect(found).toBeNull();
    });

    it("listFeatures returns deltaCount", async () => {
        const f = await createTestFeature(testUserId, { name: "List Test", priority: 1 });
        const chunkId = await createTestChunk(testUserId);

        // No deltas yet
        const listBefore = await Effect.runPromise(featureRepo.listFeatures(testUserId));
        const rowBefore = listBefore.find(r => r.id === f.id);
        expect(rowBefore).toBeDefined();
        expect(rowBefore!.deltaCount).toBe(0);

        // Add a delta
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: crypto.randomUUID(),
                chunkId,
                featureId: f.id,
                delta: { title: "Overridden" },
            }),
        );

        const listAfter = await Effect.runPromise(featureRepo.listFeatures(testUserId));
        const rowAfter = listAfter.find(r => r.id === f.id);
        expect(rowAfter).toBeDefined();
        expect(rowAfter!.deltaCount).toBe(1);
    });

    it("featureNameConflict detects a duplicate name for a different feature id", async () => {
        const f1 = await createTestFeature(testUserId, { name: "Shared Name", priority: 1 });
        const f2 = await createTestFeature(testUserId, { name: "Other Name", priority: 2 });

        // Checking whether f2 would conflict if renamed to f1's name
        const conflict = await Effect.runPromise(featureRepo.featureNameConflict(f2.id, testUserId, f1.name));
        expect(conflict).toBe(true);
    });

    it("featureNameConflict returns false for same feature id", async () => {
        const f = await createTestFeature(testUserId, { name: "Unique Name", priority: 1 });

        const conflict = await Effect.runPromise(featureRepo.featureNameConflict(f.id, testUserId, f.name));
        expect(conflict).toBe(false);
    });

    it("featureNameConflict returns false when no conflict", async () => {
        const f = await createTestFeature(testUserId, { name: "Some Feature", priority: 1 });

        const conflict = await Effect.runPromise(featureRepo.featureNameConflict(f.id, testUserId, "Totally Different"));
        expect(conflict).toBe(false);
    });
});

// ---------------------------------------------------------------------------
// 3. Delta operations
// ---------------------------------------------------------------------------

describe("Delta operations", () => {
    let testUserId: string;
    let testChunkId: string;
    let testFeatureId: string;

    beforeEach(async () => {
        testUserId = await createTestUser();
        testChunkId = await createTestChunk(testUserId);
        const f = await createTestFeature(testUserId, { priority: 1 });
        testFeatureId = f.id;
    });

    afterEach(async () => {
        await db.delete(user).where(eq(user.id, testUserId));
    });

    it("upserts a delta (create) and reads it back via getDeltasForChunk", async () => {
        const deltaId = crypto.randomUUID();
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: deltaId,
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { title: "Feature Title" },
            }),
        );

        const deltas = await Effect.runPromise(chunkDeltaRepo.getDeltasForChunk(testChunkId));
        expect(deltas).toHaveLength(1);
        expect(deltas[0]!.chunkId).toBe(testChunkId);
        expect(deltas[0]!.featureId).toBe(testFeatureId);
        expect(deltas[0]!.delta).toEqual({ title: "Feature Title" });
    });

    it("upsert same chunk+feature overwrites the delta (update)", async () => {
        const deltaId = crypto.randomUUID();
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: deltaId,
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { title: "Initial Title" },
            }),
        );

        // Upsert again with same chunk+feature (different id, same conflict target)
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: crypto.randomUUID(),
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { title: "Updated Title", content: "New content" },
            }),
        );

        const deltas = await Effect.runPromise(chunkDeltaRepo.getDeltasForChunk(testChunkId));
        // Still only one row (upsert merged into existing)
        expect(deltas).toHaveLength(1);
        expect(deltas[0]!.delta).toEqual({ title: "Updated Title", content: "New content" });
    });

    it("getDeltasForFeature returns chunk titles", async () => {
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: crypto.randomUUID(),
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { content: "Different content" },
            }),
        );

        const deltas = await Effect.runPromise(chunkDeltaRepo.getDeltasForFeature(testFeatureId));
        expect(deltas).toHaveLength(1);
        expect(deltas[0]!.featureId).toBe(testFeatureId);
        expect(deltas[0]!.chunkId).toBe(testChunkId);
        expect(deltas[0]!.chunkTitle).toBe("Test Chunk");
    });

    it("batchFetchDeltas filters by chunkIds AND featureIds", async () => {
        // Create a second chunk and feature
        const chunkId2 = await createTestChunk(testUserId);
        const f2 = await createTestFeature(testUserId, { priority: 2 });

        // Insert deltas for all 4 combinations
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({ id: crypto.randomUUID(), chunkId: testChunkId, featureId: testFeatureId, delta: { title: "c1f1" } }),
        );
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({ id: crypto.randomUUID(), chunkId: testChunkId, featureId: f2.id, delta: { title: "c1f2" } }),
        );
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({ id: crypto.randomUUID(), chunkId: chunkId2, featureId: testFeatureId, delta: { title: "c2f1" } }),
        );
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({ id: crypto.randomUUID(), chunkId: chunkId2, featureId: f2.id, delta: { title: "c2f2" } }),
        );

        // Only fetch for testChunkId + testFeatureId
        const result = await Effect.runPromise(
            chunkDeltaRepo.batchFetchDeltas([testChunkId], [testFeatureId]),
        );
        expect(result).toHaveLength(1);
        expect(result[0]!.chunkId).toBe(testChunkId);
        expect(result[0]!.featureId).toBe(testFeatureId);
        expect((result[0]!.delta as Record<string, unknown>).title).toBe("c1f1");
    });

    it("batchFetchDeltas returns empty for empty inputs", async () => {
        const result1 = await Effect.runPromise(chunkDeltaRepo.batchFetchDeltas([], [testFeatureId]));
        expect(result1).toHaveLength(0);

        const result2 = await Effect.runPromise(chunkDeltaRepo.batchFetchDeltas([testChunkId], []));
        expect(result2).toHaveLength(0);
    });

    it("deleteDelta removes a specific delta", async () => {
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: crypto.randomUUID(),
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { title: "To be deleted" },
            }),
        );

        const deleted = await Effect.runPromise(
            chunkDeltaRepo.deleteDelta(testChunkId, testFeatureId),
        );
        expect(deleted).not.toBeNull();

        const deltas = await Effect.runPromise(chunkDeltaRepo.getDeltasForChunk(testChunkId));
        expect(deltas).toHaveLength(0);
    });

    it("deleting a feature cascades to its deltas", async () => {
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: crypto.randomUUID(),
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { title: "Cascade test" },
            }),
        );

        // Delete the feature — cascade should remove deltas
        await Effect.runPromise(featureRepo.deleteFeature(testFeatureId, testUserId));

        const remaining = await db
            .select()
            .from(chunkFeatureDelta)
            .where(eq(chunkFeatureDelta.featureId, testFeatureId));
        expect(remaining).toHaveLength(0);
    });

    it("deleting a chunk cascades to its deltas", async () => {
        await Effect.runPromise(
            chunkDeltaRepo.upsertDelta({
                id: crypto.randomUUID(),
                chunkId: testChunkId,
                featureId: testFeatureId,
                delta: { title: "Chunk cascade test" },
            }),
        );

        // Delete the chunk — cascade should remove deltas
        await db.delete(chunk).where(eq(chunk.id, testChunkId));

        const remaining = await db
            .select()
            .from(chunkFeatureDelta)
            .where(eq(chunkFeatureDelta.chunkId, testChunkId));
        expect(remaining).toHaveLength(0);
    });
});

// ---------------------------------------------------------------------------
// 4. Active features
// ---------------------------------------------------------------------------

describe("Active features", () => {
    let testUserId: string;

    beforeEach(async () => {
        testUserId = await createTestUser();
    });

    afterEach(async () => {
        await db.delete(user).where(eq(user.id, testUserId));
    });

    it("sets and gets active features", async () => {
        const f1 = await createTestFeature(testUserId, { priority: 1 });
        const f2 = await createTestFeature(testUserId, { priority: 2 });

        await Effect.runPromise(featureRepo.setActiveFeatures(testUserId, [f1.id, f2.id]));

        const rows = await Effect.runPromise(featureRepo.getActiveFeatureIds(testUserId));
        const ids = rows.map(r => r.featureId);
        expect(ids).toContain(f1.id);
        expect(ids).toContain(f2.id);
        expect(ids).toHaveLength(2);
    });

    it("replaces active features on subsequent set", async () => {
        const f1 = await createTestFeature(testUserId, { priority: 1 });
        const f2 = await createTestFeature(testUserId, { priority: 2 });

        await Effect.runPromise(featureRepo.setActiveFeatures(testUserId, [f1.id, f2.id]));
        // Now switch to only f2
        await Effect.runPromise(featureRepo.setActiveFeatures(testUserId, [f2.id]));

        const rows = await Effect.runPromise(featureRepo.getActiveFeatureIds(testUserId));
        const ids = rows.map(r => r.featureId);
        expect(ids).not.toContain(f1.id);
        expect(ids).toContain(f2.id);
        expect(ids).toHaveLength(1);
    });

    it("clears active features with an empty array", async () => {
        const f1 = await createTestFeature(testUserId, { priority: 1 });

        await Effect.runPromise(featureRepo.setActiveFeatures(testUserId, [f1.id]));
        await Effect.runPromise(featureRepo.setActiveFeatures(testUserId, []));

        const rows = await Effect.runPromise(featureRepo.getActiveFeatureIds(testUserId));
        expect(rows).toHaveLength(0);
    });

    it("returns empty list when no active features set", async () => {
        const rows = await Effect.runPromise(featureRepo.getActiveFeatureIds(testUserId));
        expect(rows).toHaveLength(0);
    });
});

// ---------------------------------------------------------------------------
// 5. Priority management
// ---------------------------------------------------------------------------

describe("Priority management", () => {
    let testUserId: string;

    beforeEach(async () => {
        testUserId = await createTestUser();
    });

    afterEach(async () => {
        await db.delete(user).where(eq(user.id, testUserId));
    });

    it("getMaxPriority returns 0 when no features exist", async () => {
        const max = await Effect.runPromise(featureRepo.getMaxPriority(testUserId));
        expect(max).toBe(0);
    });

    it("getMaxPriority returns correct value after creating features", async () => {
        await createTestFeature(testUserId, { priority: 1 });
        await createTestFeature(testUserId, { priority: 5 });
        await createTestFeature(testUserId, { priority: 3 });

        const max = await Effect.runPromise(featureRepo.getMaxPriority(testUserId));
        expect(max).toBe(5);
    });

    it("shiftPriorities shifts existing features up correctly", async () => {
        // Create features at priorities 2 and 4
        const f1 = await createTestFeature(testUserId, { priority: 2 });
        const f2 = await createTestFeature(testUserId, { priority: 4 });

        // Shift up all features at priority >= 2 to make room for a new one at priority 2
        await Effect.runPromise(featureRepo.shiftPriorities(testUserId, 2, "up"));

        const [updatedF1] = await db.select().from(feature).where(eq(feature.id, f1.id));
        const [updatedF2] = await db.select().from(feature).where(eq(feature.id, f2.id));

        expect(updatedF1!.priority).toBe(3); // shifted from 2 → 3
        expect(updatedF2!.priority).toBe(5); // shifted from 4 → 5
    });

    it("shiftPriorities shifts features down correctly", async () => {
        // Create features at priorities 3 and 5
        const f1 = await createTestFeature(testUserId, { priority: 3 });
        const f2 = await createTestFeature(testUserId, { priority: 5 });

        // Shift down all features at priority <= 5
        await Effect.runPromise(featureRepo.shiftPriorities(testUserId, 5, "down"));

        const [updatedF1] = await db.select().from(feature).where(eq(feature.id, f1.id));
        const [updatedF2] = await db.select().from(feature).where(eq(feature.id, f2.id));

        expect(updatedF1!.priority).toBe(2); // shifted from 3 → 2
        expect(updatedF2!.priority).toBe(4); // shifted from 5 → 4
    });
});
