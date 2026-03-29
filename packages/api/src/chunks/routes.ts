import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as bulkService from "./bulk-service";
import { federatedSearch } from "./federated-search";
import * as chunkService from "./service";
import { checkSimilar } from "./similarity";
import { getConnectionSuggestions } from "./suggestions";

export const chunkRoutes = new Elysia()
    .get(
        "/chunks",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.listChunks(session.user.id, ctx.query)))),
        {
            query: t.Object({
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
                exclude: t.Optional(t.String()),
                scope: t.Optional(t.String()),
                alias: t.Optional(t.String()),
                sort: t.Optional(t.Union([t.Literal("newest"), t.Literal("oldest"), t.Literal("alpha"), t.Literal("updated")])),
                tags: t.Optional(t.String()),
                after: t.Optional(t.String()),
                enrichment: t.Optional(t.Union([t.Literal("missing"), t.Literal("complete")])),
                minConnections: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                workspaceId: t.Optional(t.String()),
                global: t.Optional(t.String()),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
                reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")])),
                allCodebases: t.Optional(t.String())
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
    .post(
        "/chunks/import-docs",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        chunkService.importDocs(session.user.id, ctx.body.files, ctx.body.codebaseId)
                    )
                )
            ),
        {
            body: t.Object({
                files: t.Array(
                    t.Object({
                        path: t.String({ maxLength: 500 }),
                        content: t.String({ maxLength: 100000 })
                    }),
                    { maxItems: 500 }
                ),
                codebaseId: t.String()
            })
        }
    )
    .post(
        "/chunks/bulk-update",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => bulkService.bulkUpdate(session.user.id, ctx.body) as Effect.Effect<{ updated: number }, any>)
                )
            ),
        {
            body: t.Object({
                ids: t.Array(t.String(), { maxItems: 100 }),
                action: t.Union([
                    t.Literal("add_tags"),
                    t.Literal("remove_tags"),
                    t.Literal("set_type"),
                    t.Literal("set_codebase"),
                    t.Literal("set_review_status"),
                    t.Literal("archive"),
                    t.Literal("delete")
                ]),
                value: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .get(
        "/chunks/archived",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => chunkService.listArchivedChunks(session.user.id, ctx.query.codebaseId))
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/chunks/search/semantic",
        ctx =>
            Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.semanticSearch(session.user.id, ctx.query)))),
        {
            query: t.Object({
                q: t.String(),
                limit: t.Optional(t.String()),
                exclude: t.Optional(t.String()),
                scope: t.Optional(t.String())
            })
        }
    )
    .post(
        "/chunks/check-similar",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        checkSimilar({
                            title: ctx.body.title,
                            content: ctx.body.content,
                            userId: session.user.id,
                            excludeId: ctx.body.excludeId,
                        })
                    )
                )
            ),
        {
            body: t.Object({
                title: t.String(),
                content: t.String(),
                excludeId: t.Optional(t.String()),
            }),
        }
    )
    .get(
        "/chunks/search/federated",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => federatedSearch(session.user.id, ctx.query)))
            ),
        {
            query: t.Object({
                search: t.Optional(t.String()),
                type: t.Optional(t.String()),
                tags: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
                sort: t.Optional(t.Union([t.Literal("newest"), t.Literal("oldest"), t.Literal("alpha"), t.Literal("updated")]))
            })
        }
    )
    .get("/chunks/:id/suggestions", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => getConnectionSuggestions(ctx.params.id, session.user.id))))
    )
    .get("/chunks/:id/history", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkHistory(ctx.params.id, session.user.id))))
    )
    .get("/chunks/:id", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkDetail(ctx.params.id, session.user.id))))
    )
    .post("/chunks/:id/archive", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => chunkService.archiveChunk(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Archived" }))
            )
        )
    )
    .post("/chunks/:id/restore", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => chunkService.restoreChunk(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Restored" }))
            )
        )
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
                tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 })),
                codebaseIds: t.Optional(t.Array(t.String(), { maxItems: 20 })),
                rationale: t.Optional(t.String({ maxLength: 5000 })),
                alternatives: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 })),
                consequences: t.Optional(t.String({ maxLength: 5000 })),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")]))
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
                codebaseIds: t.Optional(t.Array(t.String(), { maxItems: 20 })),
                summary: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                aliases: t.Optional(t.Array(t.String({ maxLength: 100 }), { maxItems: 20 })),
                notAbout: t.Optional(t.Array(t.String({ maxLength: 100 }), { maxItems: 20 })),
                scope: t.Optional(t.Record(t.String(), t.String())),
                rationale: t.Optional(t.String({ maxLength: 5000 })),
                alternatives: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 })),
                consequences: t.Optional(t.String({ maxLength: 5000 })),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
                reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")]))
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
