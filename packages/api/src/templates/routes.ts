import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as templateService from "./service";

export const templateRoutes = new Elysia()
    .get("/templates", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => templateService.listTemplates(session.user.id))))
    )
    .post(
        "/templates",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => templateService.createTemplate(session.user.id, ctx.body)),
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
                description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                type: t.String({ maxLength: 20 }),
                content: t.String({ maxLength: 50000 })
            })
        }
    )
    .patch(
        "/templates/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => templateService.updateTemplate(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                type: t.Optional(t.String({ maxLength: 20 })),
                content: t.Optional(t.String({ maxLength: 50000 }))
            })
        }
    )
    .delete("/templates/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => templateService.deleteTemplate(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
