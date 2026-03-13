import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as codebaseService from "./service";

export const codebaseRoutes = new Elysia()
    .get(
        "/codebases/detect",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => codebaseService.detectCodebase(session.user.id, ctx.query)))
            ),
        {
            query: t.Object({
                remoteUrl: t.Optional(t.String()),
                localPath: t.Optional(t.String())
            })
        }
    )
    .get("/codebases", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => codebaseService.listCodebases(session.user.id))))
    )
    .post(
        "/codebases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => codebaseService.createCodebase(session.user.id, ctx.body)),
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
                remoteUrl: t.Optional(t.String({ maxLength: 500 })),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .get("/codebases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => codebaseService.getCodebase(ctx.params.id, session.user.id)))
        )
    )
    .patch(
        "/codebases/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => codebaseService.updateCodebase(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                remoteUrl: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .delete("/codebases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => codebaseService.deleteCodebase(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
