import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as activityService from "./service";

export const activityRoutes = new Elysia().get(
    "/activity",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    activityService.listActivity(session.user.id, {
                        codebaseId: ctx.query.codebaseId || undefined,
                        entityType: ctx.query.entityType || undefined,
                        limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
                        offset: ctx.query.offset ? Number(ctx.query.offset) : undefined
                    })
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String()),
            entityType: t.Optional(t.String()),
            limit: t.Optional(t.String()),
            offset: t.Optional(t.String())
        })
    }
);
