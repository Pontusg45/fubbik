// Legacy alias for the VS Code extension. Forwards /api/codebases to the same
// handlers that back /api/spaces. Remove once the VS Code extension upgrades.
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as spaceService from "../spaces/service";

export const codebaseRoutes = new Elysia()
    .get(
        "/codebases/detect",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.detectSpace(session.user.id, ctx.query)))),
        { query: t.Object({ remoteUrl: t.Optional(t.String()), localPath: t.Optional(t.String()) }) }
    )
    .get("/codebases", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.listSpaces(session.user.id))))
    )
    .post(
        "/codebases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => spaceService.createSpace(session.user.id, { ...ctx.body, kind: "code" })),
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
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.getSpace(ctx.params.id, session.user.id))))
    )
    .patch(
        "/codebases/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => spaceService.updateSpace(ctx.params.id, session.user.id, ctx.body)))
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                remoteUrl: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .post("/codebases/:id/reset", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.resetSpace(ctx.params.id, session.user.id))))
    )
    .delete("/codebases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => spaceService.deleteSpace(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
