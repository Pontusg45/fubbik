import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { optionalSession, requireSession } from "../require-session";
import * as chunkService from "./service";

export const chunkRoutes = new Elysia()
    .get(
        "/chunks",
        ctx => Effect.runPromise(optionalSession(ctx).pipe(Effect.flatMap(session => chunkService.listChunks(session?.user.id, ctx.query)))),
        {
            query: t.Object({
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
                exclude: t.Optional(t.String()),
                scope: t.Optional(t.String()),
                alias: t.Optional(t.String())
            })
        }
    )
    .get("/chunks/export", ctx =>
        Effect.runPromise(optionalSession(ctx).pipe(Effect.flatMap(session => chunkService.exportChunks(session?.user.id))))
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
    .get(
        "/chunks/search/semantic",
        ctx =>
            Effect.runPromise(
                optionalSession(ctx).pipe(Effect.flatMap(session => chunkService.semanticSearch(session?.user.id, ctx.query)))
            ),
        {
            query: t.Object({
                q: t.String(),
                limit: t.Optional(t.String()),
                exclude: t.Optional(t.String()),
                scope: t.Optional(t.String())
            })
        }
    )
    .get("/chunks/:id/history", ctx =>
        Effect.runPromise(optionalSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkHistory(ctx.params.id, session?.user.id))))
    )
    .get("/chunks/:id", ctx =>
        Effect.runPromise(optionalSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkDetail(ctx.params.id, session?.user.id))))
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
                tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 })),
                summary: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                aliases: t.Optional(t.Array(t.String({ maxLength: 100 }), { maxItems: 20 })),
                notAbout: t.Optional(t.Array(t.String({ maxLength: 100 }), { maxItems: 20 })),
                scope: t.Optional(t.Record(t.String(), t.String()))
            })
        }
    )
    .delete(
        "/chunks/bulk",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => chunkService.deleteMany(ctx.body.ids, session.user.id)),
                    Effect.map(deleted => ({ deleted: deleted.length }))
                )
            ),
        {
            body: t.Object({
                ids: t.Array(t.String(), { maxItems: 100 })
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
