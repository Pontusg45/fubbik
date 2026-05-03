import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as featureService from "./service";

export const featureRoutes = new Elysia()
    .get(
        "/features",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        featureService.listFeatures(session.user.id, {
                            codebaseId: ctx.query.codebaseId,
                            status: ctx.query.status,
                            search: ctx.query.search
                        })
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                status: t.Optional(t.String()),
                search: t.Optional(t.String())
            })
        }
    )
    .post(
        "/features",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.createFeature(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 }),
                description: t.Optional(t.String({ maxLength: 1000 })),
                priority: t.Optional(t.Number()),
                color: t.Optional(t.String({ maxLength: 7 })),
                codebaseIds: t.Optional(t.Array(t.String()))
            })
        }
    )
    .get("/features/active", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.getActiveFeatures(session.user.id))
            )
        )
    )
    .put(
        "/features/active",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.setActiveFeatures(session.user.id, ctx.body.featureIds)),
                    Effect.map(() => ({ message: "Active features updated" }))
                )
            ),
        {
            body: t.Object({
                featureIds: t.Array(t.String())
            })
        }
    )
    .get("/features/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.getFeatureDetail(ctx.params.id, session.user.id))
            )
        )
    )
    .patch(
        "/features/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.updateFeature(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
                priority: t.Optional(t.Number()),
                status: t.Optional(t.Union([t.Literal("active"), t.Literal("inactive"), t.Literal("archived")])),
                color: t.Optional(t.Union([t.String({ maxLength: 7 }), t.Null()])),
                codebaseIds: t.Optional(t.Array(t.String()))
            })
        }
    )
    .delete("/features/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.deleteFeatureService(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post("/features/:id/merge", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.mergeFeature(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Feature merged" }))
            )
        )
    )
    .post(
        "/features/:id/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.reorderFeature(ctx.params.id, session.user.id, ctx.body.priority))
                )
            ),
        {
            body: t.Object({
                priority: t.Number()
            })
        }
    )
    .get("/chunks/:id/deltas", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => featureService.getDeltasForChunk(ctx.params.id))
            )
        )
    )
    .put(
        "/chunks/:id/deltas/:featureId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        featureService.upsertDelta(ctx.params.id, ctx.params.featureId, session.user.id, ctx.body.delta)
                    )
                )
            ),
        {
            body: t.Object({
                delta: t.Record(t.String(), t.Unknown())
            })
        }
    )
    .delete("/chunks/:id/deltas/:featureId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    featureService.deleteDelta(ctx.params.id, ctx.params.featureId, session.user.id)
                ),
                Effect.map(() => ({ message: "Delta deleted" }))
            )
        )
    )
    .get("/features/:id/deltas", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => {
                    return featureService.getFeatureDetail(ctx.params.id, session.user.id).pipe(
                        Effect.map(detail => detail.deltas)
                    );
                })
            )
        )
    );
