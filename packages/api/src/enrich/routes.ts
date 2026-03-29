import { listChunks } from "@fubbik/db/repository";
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { checkRateLimit } from "../middleware/rate-limit";
import { requireSession } from "../require-session";
import { enrichChunk } from "./service";

export const enrichRoutes = new Elysia()
    .post(
        "/chunks/:id/enrich",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => {
                        const rl = checkRateLimit(`enrich:${session.user.id}`, 10, 60_000);
                        if (!rl.allowed) {
                            ctx.set.status = 429;
                            return Effect.succeed({
                                error: "Rate limit exceeded",
                                retryAfter: Math.ceil((rl.resetAt - Date.now()) / 1000)
                            } as any);
                        }
                        return enrichChunk(ctx.params.id);
                    })
                )
            ),
        {
            params: t.Object({ id: t.String() })
        }
    )
    .post("/chunks/enrich-all", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => listChunks({ userId: session.user.id, limit: 1000, offset: 0 })),
                Effect.flatMap(result =>
                    Effect.forEach(result.chunks, c => enrichChunk(c.id).pipe(Effect.catchAll(() => Effect.succeed(null))), {
                        concurrency: 3
                    })
                ),
                Effect.map(results => ({ enriched: results.filter(Boolean).length }))
            )
        )
    );
