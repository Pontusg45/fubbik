import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as connectionService from "./service";

export const connectionRoutes = new Elysia()
    .post(
        "/connections",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => connectionService.createConnection(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                sourceId: t.String({ maxLength: 100 }),
                targetId: t.String({ maxLength: 100 }),
                relation: t.String({ maxLength: 50 })
            })
        }
    )
    .delete("/connections/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => connectionService.deleteConnection(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
