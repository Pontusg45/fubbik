import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as favoriteService from "./service";

export const favoriteRoutes = new Elysia()
    .get("/favorites", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => favoriteService.listFavorites(session.user.id))
            )
        )
    )
    .post(
        "/favorites",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => favoriteService.addFavorite(session.user.id, ctx.body.chunkId)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                chunkId: t.String({ maxLength: 100 })
            })
        }
    )
    .delete("/favorites/:chunkId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => favoriteService.removeFavorite(session.user.id, ctx.params.chunkId)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .put(
        "/favorites/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => favoriteService.reorderFavorites(session.user.id, ctx.body)),
                    Effect.map(() => ({ message: "Reordered" }))
                )
            ),
        {
            body: t.Array(
                t.Object({
                    chunkId: t.String({ maxLength: 100 }),
                    order: t.Number()
                })
            )
        }
    );
