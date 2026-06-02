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
                        spaceId: ctx.query.spaceId || undefined,
                        entityType: ctx.query.entityType || undefined,
                        limit: ctx.query.limit,
                        offset: ctx.query.offset
                    })
                )
            )
        ),
    {
        query: t.Object({
            spaceId: t.Optional(t.String()),
            entityType: t.Optional(t.String()),
            limit: t.Optional(t.Numeric()),
            offset: t.Optional(t.Numeric())
        })
    }
);
