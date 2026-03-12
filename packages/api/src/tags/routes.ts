import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as tagService from "./service-new";

export const tagRoutes = new Elysia()
    .get("/tags", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => tagService.getUserTags(session.user.id))))
    )
    .post(
        "/tags",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagService.createUserTag(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 50 }),
                tagTypeId: t.Optional(t.String())
            })
        }
    )
    .patch(
        "/tags/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagService.updateUserTag(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 50 })),
                tagTypeId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .delete("/tags/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => tagService.deleteUserTag(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
