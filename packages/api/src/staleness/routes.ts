import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { detectAgeStaleChunks } from "./detect-age";
import { flagImpactRipple } from "./detect-impact";
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
                            spaceId: ctx.query.spaceId,
                            limit: ctx.query.limit
                        })
                    )
                )
            ),
        {
            query: t.Object({
                reason: t.Optional(t.String()),
                spaceId: t.Optional(t.String()),
                limit: t.Optional(t.Numeric())
            })
        }
    )
    .get(
        "/chunks/stale/count",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => stalenessService.getStaleCount(session.user.id, ctx.query.spaceId)))
            ),
        {
            query: t.Object({
                spaceId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/chunks/:id/dismiss-staleness",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => stalenessService.dismissStaleFlag(ctx.params.id, session.user.id)))
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
                requireSession(ctx).pipe(Effect.flatMap(session => stalenessService.suppressDuplicatePair(ctx.body.chunkIdA, ctx.body.chunkIdB, session.user.id)))
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
                        Effect.all([
                            detectAgeStaleChunks(session.user.id, ctx.body.spaceId, ctx.body.thresholdDays),
                            stalenessService.detectUncoveredChunks(session.user.id, ctx.body.spaceId)
                        ]).pipe(
                            Effect.map(([ageResult, uncoveredResult]) => ({
                                flagged: ageResult.flagged + uncoveredResult.flagged
                            }))
                        )
                    )
                )
            ),
        {
            body: t.Object({
                spaceId: t.Optional(t.String()),
                thresholdDays: t.Optional(t.Number())
            })
        }
    )
    .post(
        "/chunks/:id/scan-impact",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => flagImpactRipple(ctx.params.id, ctx.body.title ?? "Unknown", session.user.id))
                )
            ),
        {
            params: t.Object({ id: t.String() }),
            body: t.Object({ title: t.Optional(t.String()) })
        }
    );
