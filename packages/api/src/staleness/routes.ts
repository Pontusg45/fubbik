import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { detectAgeStaleChunks } from "./detect-age";
import * as stalenessService from "./service";

export const stalenessRoutes = new Elysia()
    .get(
        "/chunks/stale",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        stalenessService.getStaleFlags(session.user.id, {
                            reason: ctx.query.reason,
                            codebaseId: ctx.query.codebaseId,
                            limit: ctx.query.limit ? Number(ctx.query.limit) : undefined
                        })
                    )
                )
            ),
        {
            query: t.Object({
                reason: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                limit: t.Optional(t.String())
            })
        }
    )
    .get(
        "/chunks/stale/count",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        stalenessService.getStaleCount(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/chunks/:id/dismiss-staleness",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        stalenessService.dismissStaleFlag(ctx.params.id, session.user.id)
                    )
                )
            ),
        {
            params: t.Object({
                id: t.String()
            })
        }
    )
    .post(
        "/chunks/suppress-duplicate",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        stalenessService.suppressDuplicatePair(ctx.body.chunkIdA, ctx.body.chunkIdB)
                    )
                )
            ),
        {
            body: t.Object({
                chunkIdA: t.String(),
                chunkIdB: t.String()
            })
        }
    )
    .post(
        "/chunks/stale/scan-age",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        detectAgeStaleChunks(
                            session.user.id,
                            ctx.body.codebaseId,
                            ctx.body.thresholdDays
                        )
                    )
                )
            ),
        {
            body: t.Object({
                codebaseId: t.Optional(t.String()),
                thresholdDays: t.Optional(t.Number())
            })
        }
    );
