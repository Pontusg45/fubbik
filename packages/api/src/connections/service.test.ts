import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

// Auth is globally bypassed for now — every request resolves to DEV_SESSION.
// Flip `.not.toBe(401)` back to `.toBe(401)` on write endpoints here when
// auth is re-enabled.
describe("Connection routes — auth currently bypassed", () => {
    it("POST /api/connections does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.connections.post({
            sourceId: "a",
            targetId: "b",
            relation: "related"
        });
        // Then
        expect(status).not.toBe(401);
    });

    it("DELETE /api/connections/:id does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.connections({ id: "test" }).delete();
        // Then
        expect(status).not.toBe(401);
    });
});

describe("AI routes — auth currently bypassed", () => {
    it("POST /api/ai/summarize does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.ai.summarize.post({ chunkId: "test" });
        // Then
        expect(status).not.toBe(401);
    });

    it("POST /api/ai/suggest-connections does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.ai["suggest-connections"].post({ chunkId: "test" });
        // Then
        expect(status).not.toBe(401);
    });

    it("POST /api/ai/generate does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.ai.generate.post({ prompt: "test" });
        // Then
        expect(status).not.toBe(401);
    });
});

describe("Graph & Tags routes — auth currently bypassed", () => {
    it("GET /api/graph does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.graph.get();
        // Then
        expect(status).not.toBe(401);
    });

    it("GET /api/tags does not 401 without auth", async () => {
        // Given the inline inputs and test fixtures.
        // When
        const { status } = await client.api.tags.get();
        // Then
        expect(status).not.toBe(401);
    });
});
