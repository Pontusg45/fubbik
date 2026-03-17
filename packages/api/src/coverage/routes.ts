import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as coverageService from "./service";

export const coverageRoutes = new Elysia().get(
    "/requirements/coverage",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    ctx.query.detail === "true"
                        ? coverageService.getCoverageMatrix(session.user.id, ctx.query.codebaseId)
                        : coverageService.getCoverage(session.user.id, ctx.query.codebaseId)
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String()),
            detail: t.Optional(t.String())
        })
    }
);
