import { describe, expect, it } from "vitest";
import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { api } from "./index";

// Wrap the api plugin in a standalone Elysia app for testing
const app = new Elysia().use(api);
const client = treaty(app);

describe("Health check", () => {
  it("GET /api/health returns status and db fields", async () => {
    // The health check hits the real DB. If no DB is available it returns 503.
    const res = await client.api.health.get();

    // Either 200 (DB up) or 503 (DB down) — both are valid in CI/local
    expect([200, 503]).toContain(res.status);

    // Eden puts body in `data` on 2xx, `error` on non-2xx
    const body = res.status === 200 ? res.data : res.error?.value;
    expect(body).toHaveProperty("status");
    expect(body).toHaveProperty("db");

    if (res.status === 200) {
      expect((body as any).status).toBe("ok");
      expect((body as any).db).toBe("connected");
    } else {
      expect((body as any).status).toBe("degraded");
      expect((body as any).db).toBe("disconnected");
    }
  });
});

describe("Auth guards — unauthenticated access", () => {
  it("GET /api/me returns 401", async () => {
    const { status } = await client.api.me.get();
    expect(status).toBe(401);
  });

  it("GET /api/chunks returns 401", async () => {
    const { status } = await client.api.chunks.get();
    expect(status).toBe(401);
  });

  it("GET /api/chunks/:id returns 401", async () => {
    const { status } = await client.api.chunks({ id: "nonexistent" }).get();
    expect(status).toBe(401);
  });

  it("POST /api/chunks returns 401", async () => {
    const { status } = await client.api.chunks.post({
      title: "Test chunk",
    });
    expect(status).toBe(401);
  });

  it("PATCH /api/chunks/:id returns 401", async () => {
    const { status } = await client.api.chunks({ id: "nonexistent" }).patch({
      title: "Updated",
    });
    expect(status).toBe(401);
  });

  it("DELETE /api/chunks/:id returns 401", async () => {
    const { status } = await client.api.chunks({ id: "nonexistent" }).delete();
    expect(status).toBe(401);
  });

  it("GET /api/stats returns 401", async () => {
    const { status } = await client.api.stats.get();
    expect(status).toBe(401);
  });
});
