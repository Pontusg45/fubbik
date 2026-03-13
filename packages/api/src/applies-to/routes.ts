import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as appliesToService from "./service";

export const appliesToRoutes = new Elysia()
    .get("/chunks/:id/applies-to", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => appliesToService.getAppliesTo(ctx.params.id, session.user.id)))
        )
    )
    .put(
        "/chunks/:id/applies-to",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => appliesToService.setAppliesTo(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Array(
                t.Object({
                    pattern: t.String({ maxLength: 500 }),
                    note: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()]))
                }),
                { maxItems: 50 }
            )
        }
    );
