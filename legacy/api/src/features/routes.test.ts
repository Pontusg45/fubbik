import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

// Auth is globally bypassed — every request resolves to DEV_SESSION
// (userId: "dev-user"). The API connects to the real test database.

// ---------------------------------------------------------------------------
// Shared state for tests that depend on created resources
// ---------------------------------------------------------------------------
let featureId: string;
let chunkId: string;
const featuresToCleanup: string[] = [];
const chunksToCleanup: string[] = [];

beforeAll(async () => {
    // Create a feature to use across tests
    const { data, status } = await client.api.features.post({
        name: `test-feature-${Date.now()}`,
        description: "created for route tests"
    });
    expect(status, "beforeAll: feature creation must succeed").toBe(201);
    featureId = (data as any).id;
    featuresToCleanup.push(featureId);

    // Create a chunk to use for delta tests
    const { data: chunkData, status: chunkStatus } = await client.api.chunks.post({
        title: `test-chunk-${Date.now()}`,
        content: "content for delta tests"
    });
    expect([200, 201], "beforeAll: chunk creation must succeed").toContain(chunkStatus);
    chunkId = (chunkData as any).id;
    chunksToCleanup.push(chunkId);
});

afterAll(async () => {
    // Delete all features created during tests (ignore errors — may already be deleted)
    for (const id of featuresToCleanup) {
        await (client.api.features({ id }) as any).delete().catch(() => {});
    }
    // Delete all chunks created during tests
    for (const id of chunksToCleanup) {
        await (client.api.chunks({ id }) as any).delete().catch(() => {});
    }
});

// ---------------------------------------------------------------------------
// 1. Feature CRUD
// ---------------------------------------------------------------------------
describe("Feature CRUD", () => {
    it("GET /api/features — returns 200 and an array", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await client.api.features.get({ query: {} });
        // Then
        expect(status).toBe(200);
        expect(Array.isArray(data)).toBe(true);
    });

    it("POST /api/features — creates a feature and returns 201", async () => {
        // Given
        const name = `test-feature-create-${Date.now()}`;
        // When
        const { status, data } = await client.api.features.post({
            name,
            description: "a test feature"
        });
        // Then
        expect(status).toBe(201);
        expect((data as any).id).toBeDefined();
        expect((data as any).name).toBe(name);
        featuresToCleanup.push((data as any).id);
    });

    it("GET /api/features/:id — returns feature detail with spaces and deltas arrays", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.features({ id: featureId }) as any).get();
        // Then
        expect(status).toBe(200);
        const detail = data as any;
        expect(detail.feature).toBeDefined();
        expect(detail.feature.id).toBe(featureId);
        expect(Array.isArray(detail.spaces)).toBe(true);
        expect(Array.isArray(detail.deltas)).toBe(true);
    });

    it("PATCH /api/features/:id — updates feature name and description", async () => {
        // Given
        const updatedName = `updated-feature-${Date.now()}`;
        // When
        const { status, data } = await (client.api.features({ id: featureId }) as any).patch({
            name: updatedName,
            description: "updated description"
        });
        // Then
        expect(status).toBe(200);
        expect((data as any).name).toBe(updatedName);
    });

    it("DELETE /api/features/:id — deletes a feature and returns 200", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { data: created, status: createStatus } = await client.api.features.post({
            name: `test-feature-to-delete-${Date.now()}`
        });
        // Then
        expect(createStatus).toBe(201);
        const idToDelete = (created as any).id;

        const { status } = await (client.api.features({ id: idToDelete }) as any).delete();
        expect(status).toBe(200);
    });

    it("POST /api/features — returns 422 when name is missing (empty body)", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.features as any).post({});
        // Then
        expect(status).toBe(422);
    });
});

// ---------------------------------------------------------------------------
// 2. Feature activation
// ---------------------------------------------------------------------------
describe("Feature activation", () => {
    it("GET /api/features/active — returns 200 and an array", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.features.active as any).get();
        // Then
        expect(status).toBe(200);
        expect(Array.isArray(data)).toBe(true);
    });

    it("PUT /api/features/active — sets active features and returns 200", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.features.active as any).put({
            featureIds: [featureId]
        });
        // Then
        expect(status).toBe(200);
        expect((data as any).message).toBeDefined();
    });

    it("PUT /api/features/active — empty array clears active features", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.features.active as any).put({
            featureIds: []
        });
        // Then
        expect(status).toBe(200);
    });

    it("PUT /api/features/active — invalid IDs return 400", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.features.active as any).put({
            featureIds: ["nonexistent-feature-id-xyz"]
        });
        // Then
        expect(status).toBe(400);
    });
});

// ---------------------------------------------------------------------------
// 3. Feature lifecycle — merge and reorder
// ---------------------------------------------------------------------------
describe("Feature lifecycle", () => {
    it("POST /api/features/:id/merge — merges an empty feature (no deltas) successfully", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { data: created, status: createStatus } = await client.api.features.post({
            name: `test-feature-merge-${Date.now()}`
        });
        // Then
        expect(createStatus).toBe(201);
        const mergeId = (created as any).id;
        featuresToCleanup.push(mergeId);

        const { status, data } = await (client.api.features({ id: mergeId }) as any).merge.post({});
        expect(status).toBe(200);
        expect((data as any).message).toBe("Feature merged");
    });

    it("POST /api/features/:id/merge — returns 400 when feature is already merged", async () => {
        // Given
        // Create and merge a feature
        const { data: created } = await client.api.features.post({
            name: `test-feature-already-merged-${Date.now()}`
        });
        const mergeId = (created as any).id;
        featuresToCleanup.push(mergeId);

        await (client.api.features({ id: mergeId }) as any).merge.post({});

        // When
        // Attempt to merge again — should fail
        const { status } = await (client.api.features({ id: mergeId }) as any).merge.post({});
        // Then
        expect(status).toBe(400);
    });

    it("POST /api/features/:id/reorder — changes feature priority", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.features({ id: featureId }) as any).reorder.post({
            priority: 99
        });
        // Then
        expect(status).toBe(200);
        expect((data as any).id).toBe(featureId);
    });
});

// ---------------------------------------------------------------------------
// 4. Delta operations
// ---------------------------------------------------------------------------
describe("Delta operations", () => {
    it("PUT /api/chunks/:id/deltas/:featureId — creates a delta", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.chunks({ id: chunkId }) as any)
            .deltas({ featureId })
            .put({ delta: { content: "feature overlay content" } });
        // Then
        expect(status).toBe(200);
        expect((data as any).chunkId).toBe(chunkId);
        expect((data as any).featureId).toBe(featureId);
    });

    it("GET /api/chunks/:id/deltas — lists deltas for a chunk", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.chunks({ id: chunkId }) as any).deltas.get();
        // Then
        expect(status).toBe(200);
        expect(Array.isArray(data)).toBe(true);
        const delta = (data as any[]).find((d: any) => d.featureId === featureId);
        expect(delta).toBeDefined();
    });

    it("GET /api/features/:id/deltas — lists deltas for a feature", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status, data } = await (client.api.features({ id: featureId }) as any).deltas.get();
        // Then
        expect(status).toBe(200);
        expect(Array.isArray(data)).toBe(true);
        const delta = (data as any[]).find((d: any) => d.chunkId === chunkId);
        expect(delta).toBeDefined();
    });

    it("PUT /api/chunks/:id/deltas/:featureId — rejects invalid delta fields with 400", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.chunks({ id: chunkId }) as any)
            .deltas({ featureId })
            .put({ delta: { unknownField: "bad", anotherBadField: 123 } });
        // Then
        expect(status).toBe(400);
    });

    it("PUT /api/chunks/:id/deltas/:featureId — rejects empty delta with 400", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.chunks({ id: chunkId }) as any).deltas({ featureId }).put({ delta: {} });
        // Then
        expect(status).toBe(400);
    });

    it("DELETE /api/chunks/:id/deltas/:featureId — removes the delta", async () => {
        // Given
        // First ensure a delta exists
        await (client.api.chunks({ id: chunkId }) as any).deltas({ featureId }).put({ delta: { content: "to be deleted" } });

        // When
        const { status } = await (client.api.chunks({ id: chunkId }) as any).deltas({ featureId }).delete();
        // Then
        expect(status).toBe(200);
    });
});

// ---------------------------------------------------------------------------
// 5. Not found cases
// ---------------------------------------------------------------------------
describe("Not found cases", () => {
    it("GET /api/features/nonexistent — returns 404", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.features({ id: "nonexistent-feature-id" }) as any).get();
        // Then
        expect(status).toBe(404);
    });

    it("DELETE /api/features/nonexistent — returns 404", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await (client.api.features({ id: "nonexistent-feature-id" }) as any).delete();
        // Then
        expect(status).toBe(404);
    });
});
