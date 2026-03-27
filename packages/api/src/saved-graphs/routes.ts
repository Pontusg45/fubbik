import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as savedGraphService from "./service";

export const savedGraphRoutes = new Elysia()
    .get("/saved-graphs", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    savedGraphService.listSavedGraphs(
                        session.user.id,
                        (ctx.query as { codebaseId?: string }).codebaseId
                    )
                )
            )
        )
    )
    .post(
        "/saved-graphs",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        savedGraphService.createSavedGraph(session.user.id, ctx.body)
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
                name: t.String({ maxLength: 200 }),
                description: t.Optional(t.String({ maxLength: 2000 })),
                chunkIds: t.Array(t.String()),
                positions: t.Record(
                    t.String(),
                    t.Object({ x: t.Number(), y: t.Number() })
                ),
                layoutAlgorithm: t.Optional(t.String()),
                codebaseId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .get("/saved-graphs/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    savedGraphService.getSavedGraphDetail(ctx.params.id, session.user.id)
                )
            )
        )
    )
    .patch(
        "/saved-graphs/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        savedGraphService.updateSavedGraph(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                chunkIds: t.Optional(t.Array(t.String())),
                positions: t.Optional(
                    t.Record(
                        t.String(),
                        t.Object({ x: t.Number(), y: t.Number() })
                    )
                ),
                layoutAlgorithm: t.Optional(t.String())
            })
        }
    )
    .delete("/saved-graphs/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    savedGraphService.deleteSavedGraph(ctx.params.id, session.user.id)
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
