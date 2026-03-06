import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

describe("Chunk routes — auth required", () => {
    it("GET /api/chunks/export returns 401 without auth", async () => {
        const { status } = await client.api.chunks.export.get();
        expect(status).toBe(401);
    });

    it("POST /api/chunks/import returns 401 without auth", async () => {
        const { status } = await client.api.chunks.import.post({
            chunks: [{ title: "test" }]
        });
        expect(status).toBe(401);
    });

    it("GET /api/chunks/:id/history returns 401 without auth", async () => {
        const { status } = await client.api.chunks({ id: "test" }).history.get();
        expect(status).toBe(401);
    });
});
