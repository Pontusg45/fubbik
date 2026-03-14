import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as fileRefService from "./service";

export const fileRefRoutes = new Elysia()
    .get("/chunks/:id/file-refs", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => fileRefService.getFileRefs(ctx.params.id, session.user.id)))
        )
    )
    .put(
        "/chunks/:id/file-refs",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => fileRefService.setFileRefs(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Array(
                t.Object({
                    path: t.String({ maxLength: 1000 }),
                    anchor: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                    relation: t.Union([
                        t.Literal("documents"),
                        t.Literal("configures"),
                        t.Literal("tests"),
                        t.Literal("implements")
                    ])
                }),
                { maxItems: 50 }
            )
        }
    )
    .get(
        "/file-refs/lookup",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => fileRefService.lookupByPath(ctx.query.path, session.user.id)))
            ),
        {
            query: t.Object({
                path: t.String()
            })
        }
    );
