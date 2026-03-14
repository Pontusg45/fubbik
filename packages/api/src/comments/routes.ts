import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as commentService from "./service";

export const commentRoutes = new Elysia()
    .get("/chunks/:id/comments", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => commentService.listComments(ctx.params.id))
            )
        )
    )
    .post(
        "/chunks/:id/comments",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => commentService.createComment(ctx.params.id, session.user.id, ctx.body.content)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                content: t.String({ maxLength: 5000 })
            })
        }
    )
    .patch(
        "/comments/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => commentService.updateComment(ctx.params.id, session.user.id, ctx.body.content))
                )
            ),
        {
            body: t.Object({
                content: t.String({ maxLength: 5000 })
            })
        }
    )
    .delete("/comments/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => commentService.deleteComment(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
