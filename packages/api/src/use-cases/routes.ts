import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as useCaseService from "./service";

export const useCaseRoutes = new Elysia()
    .get(
        "/use-cases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        useCaseService.listUseCases(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/use-cases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        useCaseService.createUseCase(session.user.id, ctx.body)
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
                codebaseId: t.Optional(t.String()),
                parentId: t.Optional(t.String())
            })
        }
    )
    .get("/use-cases/:id/requirements", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    useCaseService.getUseCaseRequirements(ctx.params.id, session.user.id)
                )
            )
        )
    )
    .patch(
        "/use-cases/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        useCaseService.updateUseCase(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                order: t.Optional(t.Number()),
                parentId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .delete("/use-cases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    useCaseService.deleteUseCase(ctx.params.id, session.user.id)
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
