import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as collectionService from "./service";

const CollectionFilterSchema = t.Object({
    type: t.Optional(t.String()),
    tags: t.Optional(t.String()),
    search: t.Optional(t.String()),
    sort: t.Optional(t.String()),
    after: t.Optional(t.String()),
    enrichment: t.Optional(t.String()),
    minConnections: t.Optional(t.String()),
    origin: t.Optional(t.String()),
    reviewStatus: t.Optional(t.String())
});

export const collectionRoutes = new Elysia()
    .get("/collections", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => collectionService.listCollections(session.user.id))
            )
        )
    )
    .post(
        "/collections",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        collectionService.createCollection(session.user.id, ctx.body)
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 }),
                description: t.Optional(t.String({ maxLength: 500 })),
                filter: CollectionFilterSchema,
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .patch(
        "/collections/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        collectionService.updateCollection(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.String({ maxLength: 500 })),
                filter: t.Optional(CollectionFilterSchema)
            })
        }
    )
    .delete("/collections/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    collectionService.deleteCollection(ctx.params.id, session.user.id)
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .get("/collections/:id/chunks", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    collectionService.getCollectionChunks(ctx.params.id, session.user.id)
                )
            )
        )
    );
