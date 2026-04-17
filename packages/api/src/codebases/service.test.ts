import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

// Reads bypass auth by policy — only writes (POST/PATCH/DELETE) require a
// session. There are no codebase write-path tests here because those would hit
// a real database; exercise them in a higher-level integration suite instead.
describe("Codebase routes", () => {
    it("GET /api/codebases does not 401 without auth (reads bypass)", async () => {
        const { status } = await client.api.codebases.get();
        expect(status).not.toBe(401);
    });
});
