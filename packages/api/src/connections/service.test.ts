import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

describe("Connection routes — auth required", () => {
    it("POST /api/connections returns 401 without auth", async () => {
        const { status } = await client.api.connections.post({
            sourceId: "a",
            targetId: "b",
            relation: "related"
        });
        expect(status).toBe(401);
    });

    it("DELETE /api/connections/:id returns 401 without auth", async () => {
        const { status } = await client.api.connections({ id: "test" }).delete();
        expect(status).toBe(401);
    });
});

describe("AI routes — auth required", () => {
    it("POST /api/ai/summarize returns 401 without auth", async () => {
        const { status } = await client.api.ai.summarize.post({ chunkId: "test" });
        expect(status).toBe(401);
    });

    it("POST /api/ai/suggest-connections returns 401 without auth", async () => {
        const { status } = await client.api.ai["suggest-connections"].post({ chunkId: "test" });
        expect(status).toBe(401);
    });

    it("POST /api/ai/generate returns 401 without auth", async () => {
        const { status } = await client.api.ai.generate.post({ prompt: "test" });
        expect(status).toBe(401);
    });
});

describe("Graph & Tags routes — auth required", () => {
    it("GET /api/graph returns 401 without auth", async () => {
        const { status } = await client.api.graph.get();
        expect(status).toBe(401);
    });

    it("GET /api/tags returns 401 without auth", async () => {
        const { status } = await client.api.tags.get();
        expect(status).toBe(401);
    });
});
