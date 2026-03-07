import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as chunkService from "./service";

export const chunkRoutes = new Elysia()
    .get(
        "/chunks",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.listChunks(session.user.id, ctx.query)))),
        {
            query: t.Object({
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String())
            })
        }
    )
    .get("/chunks/export", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.exportChunks(session.user.id))))
    )
    .post(
        "/chunks/import",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => chunkService.importChunks(session.user.id, ctx.body.chunks)),
                    Effect.map(created => ({ imported: created.length }))
                )
            ),
        {
            body: t.Object({
                chunks: t.Array(
                    t.Object({
                        title: t.String({ maxLength: 200 }),
                        content: t.Optional(t.String({ maxLength: 50000 })),
                        type: t.Optional(t.String({ maxLength: 20 })),
                        tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
                    }),
                    { maxItems: 500 }
                )
            })
        }
    )
    .get("/chunks/:id/history", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkHistory(ctx.params.id, session.user.id))))
    )
    .get("/chunks/:id", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkDetail(ctx.params.id, session.user.id))))
    )
    .post(
        "/chunks",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => chunkService.createChunk(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                title: t.String({ maxLength: 200 }),
                content: t.Optional(t.String({ maxLength: 50000 })),
                type: t.Optional(t.String({ maxLength: 20 })),
                tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
            })
        }
    )
    .patch(
        "/chunks/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => chunkService.updateChunk(ctx.params.id, session.user.id, ctx.body)))
            ),
        {
            body: t.Object({
                title: t.Optional(t.String({ maxLength: 200 })),
                content: t.Optional(t.String({ maxLength: 50000 })),
                type: t.Optional(t.String({ maxLength: 20 })),
                tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
            })
        }
    )
    .delete("/chunks/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => chunkService.deleteChunk(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
