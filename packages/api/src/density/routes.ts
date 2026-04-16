import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as densityService from "./service";

export const densityRoutes = new Elysia().get(
    "/density",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    densityService.getDensity(session.user.id, ctx.query.codebaseId || undefined)
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String())
        })
    }
);
