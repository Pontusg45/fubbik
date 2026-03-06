import { checkDbConnectivity } from "@fubbik/db/repository";
import { Effect } from "effect";
import { Elysia } from "elysia";

export const healthRoutes = new Elysia().get("/health", ({ set }) =>
    Effect.runPromise(
        checkDbConnectivity().pipe(
            Effect.match({
                onSuccess: () => ({ status: "ok" as const, db: "connected" as const }),
                onFailure: () => {
                    set.status = 503;
                    return { status: "degraded" as const, db: "disconnected" as const };
                }
            })
        )
    )
);
