import { treaty } from "@elysiajs/eden";
import { Cause, Effect, Option } from "effect";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

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

describe("Effect error pipeline — FiberFailure extraction", () => {
    it("FiberFailure wraps tagged errors correctly", async () => {
        const FiberFailureCauseSymbol = Symbol.for("effect/Runtime/FiberFailure/Cause");
        try {
            await Effect.runPromise(Effect.fail({ _tag: "TestError", message: "test" }));
            expect.unreachable("Should have thrown");
        } catch (e: unknown) {
            const error = e as Record<symbol, unknown>;
            expect(error[FiberFailureCauseSymbol]).toBeDefined();
            const cause = error[FiberFailureCauseSymbol] as Cause.Cause<{ _tag: string }>;
            const option = Cause.failureOption(cause);
            expect(Option.isSome(option)).toBe(true);
            if (Option.isSome(option)) {
                expect(option.value._tag).toBe("TestError");
            }
        }
    });
});
