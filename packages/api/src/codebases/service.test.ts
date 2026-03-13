import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

describe("Codebase routes", () => {
    it("GET /api/codebases returns 200", async () => {
        const { status } = await client.api.codebases.get();
        expect(status).toBe(200);
    });

    it("GET /api/codebases/detect returns 200", async () => {
        const { status } = await client.api.codebases.detect.get({
            query: { remoteUrl: "https://github.com/test/repo" }
        });
        expect(status).toBe(200);
    });
});
