import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as timelineService from "./service";

export const timelineRoutes = new Elysia().get(
    "/timeline",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    timelineService.getTimeline(session.user.id, {
                        range: ctx.query.range,
                        codebaseId: ctx.query.codebaseId || undefined,
                        tag: ctx.query.tag || undefined
                    })
                )
            )
        ),
    {
        query: t.Object({
            range: t.Optional(t.String()),
            codebaseId: t.Optional(t.String()),
            tag: t.Optional(t.String())
        })
    }
);
