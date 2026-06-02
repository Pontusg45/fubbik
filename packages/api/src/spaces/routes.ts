import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as spaceService from "./service";

export const spaceRoutes = new Elysia()
    .get(
        "/spaces/detect",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.detectSpace(session.user.id, ctx.query)))),
        {
            query: t.Object({
                remoteUrl: t.Optional(t.String()),
                localPath: t.Optional(t.String())
            })
        }
    )
    .get("/spaces", ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.listSpaces(session.user.id)))))
    .post(
        "/spaces",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => spaceService.createSpace(session.user.id, ctx.body)),
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
                kind: t.Optional(t.String({ maxLength: 50 })),
                description: t.Optional(t.String({ maxLength: 1000 })),
                remoteUrl: t.Optional(t.String({ maxLength: 500 })),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .get("/spaces/:id", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.getSpace(ctx.params.id, session.user.id))))
    )
    .patch(
        "/spaces/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => spaceService.updateSpace(ctx.params.id, session.user.id, ctx.body)))
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
                remoteUrl: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .post("/spaces/:id/reset", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.resetSpace(ctx.params.id, session.user.id))))
    )
    .delete("/spaces/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => spaceService.deleteSpace(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
