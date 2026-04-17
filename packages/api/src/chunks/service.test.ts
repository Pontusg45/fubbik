import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

// Auth is globally bypassed for now — every request resolves to DEV_SESSION.
// When auth is re-enabled, flip `.not.toBe(401)` back to `.toBe(401)` on write
// endpoints that were previously gated.
describe("Chunk routes — auth currently bypassed", () => {
    it("POST /api/chunks/import does not 401 without auth", async () => {
        const { status } = await client.api.chunks.import.post({
            chunks: [{ title: "test" }]
        });
        expect(status).not.toBe(401);
    });
});
