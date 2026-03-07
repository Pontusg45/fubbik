import { listChunks } from "@fubbik/db/repository";
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { enrichChunk } from "./service";

export const enrichRoutes = new Elysia()
    .post("/chunks/:id/enrich", ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(() => enrichChunk(ctx.params.id)))), {
        params: t.Object({ id: t.String() })
    })
    .post("/chunks/enrich-all", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => listChunks({ userId: session.user.id, limit: 1000, offset: 0 })),
                Effect.flatMap(result =>
                    Effect.forEach(result.chunks, c => enrichChunk(c.id).pipe(Effect.catchAll(() => Effect.succeed(null))), {
                        concurrency: 1
                    })
                ),
                Effect.map(results => ({ enriched: results.filter(Boolean).length }))
            )
        )
    );
