import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as vocabularyService from "./service";

const chunkTypeBodySchema = t.Object({
    id: t.String({ maxLength: 41 }),
    label: t.String({ maxLength: 80 }),
    description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
    icon: t.Optional(t.Union([t.String({ maxLength: 40 }), t.Null()])),
    color: t.Optional(t.String({ maxLength: 9 })),
    examples: t.Optional(t.Array(t.String({ maxLength: 80 }), { maxItems: 20 })),
    displayOrder: t.Optional(t.Number())
});

const chunkTypePatchSchema = t.Object({
    label: t.Optional(t.String({ maxLength: 80 })),
    description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
    icon: t.Optional(t.Union([t.String({ maxLength: 40 }), t.Null()])),
    color: t.Optional(t.String({ maxLength: 9 })),
    examples: t.Optional(t.Array(t.String({ maxLength: 80 }), { maxItems: 20 })),
    displayOrder: t.Optional(t.Number())
});

const relationBodySchema = t.Object({
    id: t.String({ maxLength: 41 }),
    label: t.String({ maxLength: 80 }),
    description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
    arrowStyle: t.Optional(t.Union([t.Literal("solid"), t.Literal("dashed"), t.Literal("dotted")])),
    direction: t.Optional(t.Union([t.Literal("forward"), t.Literal("bidirectional")])),
    color: t.Optional(t.String({ maxLength: 9 })),
    inverseOfId: t.Optional(t.Union([t.String({ maxLength: 41 }), t.Null()])),
    displayOrder: t.Optional(t.Number())
});

const relationPatchSchema = t.Object({
    label: t.Optional(t.String({ maxLength: 80 })),
    description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
    arrowStyle: t.Optional(t.Union([t.Literal("solid"), t.Literal("dashed"), t.Literal("dotted")])),
    direction: t.Optional(t.Union([t.Literal("forward"), t.Literal("bidirectional")])),
    color: t.Optional(t.String({ maxLength: 9 })),
    inverseOfId: t.Optional(t.Union([t.String({ maxLength: 41 }), t.Null()])),
    displayOrder: t.Optional(t.Number())
});

export const vocabularyCatalogRoutes = new Elysia()
    .get(
        "/chunk-types",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        vocabularyService.getChunkTypes(session.user.id, ctx.query.codebaseId || undefined)
                    )
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()) }) }
    )
    .post(
        "/chunk-types",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => vocabularyService.createChunkType(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        { body: chunkTypeBodySchema }
    )
    .patch(
        "/chunk-types/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => vocabularyService.updateChunkType(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        { body: chunkTypePatchSchema }
    )
    .delete("/chunk-types/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => vocabularyService.deleteChunkType(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .get(
        "/connection-relations",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        vocabularyService.getConnectionRelations(session.user.id, ctx.query.codebaseId || undefined)
                    )
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()) }) }
    )
    .post(
        "/connection-relations",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => vocabularyService.createConnectionRelation(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        { body: relationBodySchema }
    )
    .patch(
        "/connection-relations/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        vocabularyService.updateConnectionRelation(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        { body: relationPatchSchema }
    )
    .delete("/connection-relations/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => vocabularyService.deleteConnectionRelation(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
