import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as sessionService from "./service";

const StatusSchema = t.Union([
    t.Literal("passing"),
    t.Literal("failing"),
    t.Literal("untested")
]);

export const sessionRoutes = new Elysia()
    // Create session
    .post(
        "/sessions",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    const result = yield* sessionService.createSession(session.user.id, ctx.body);
                    ctx.set.status = 201;
                    return result;
                })
            ),
        { body: t.Object({ title: t.String(), codebaseId: t.Optional(t.String()) }) }
    )
    // List sessions
    .get(
        "/sessions",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* sessionService.listSessions(session.user.id, {
                        ...ctx.query,
                        limit: ctx.query.limit !== undefined ? Number(ctx.query.limit) : undefined,
                        offset: ctx.query.offset !== undefined ? Number(ctx.query.offset) : undefined
                    });
                })
            ),
        {
            query: t.Object({
                status: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String())
            })
        }
    )
    // Integration: get sessions by requirement (must come before /sessions/:id)
    .get(
        "/sessions/by-requirement/:requirementId",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    yield* requireSession(ctx);
                    return yield* sessionService.getSessionsForRequirement(ctx.params.requirementId);
                })
            )
    )
    // Integration: knowledge gaps (must come before /sessions/:id)
    .get(
        "/sessions/knowledge-gaps",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* sessionService.getKnowledgeGaps(session.user.id);
                })
            )
    )
    // Get session detail
    .get(
        "/sessions/:id",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* sessionService.getSession(ctx.params.id, session.user.id);
                })
            )
    )
    // Complete session
    .patch(
        "/sessions/:id/complete",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* sessionService.completeSession(ctx.params.id, session.user.id, ctx.body.prUrl);
                })
            ),
        { body: t.Object({ prUrl: t.Optional(t.String()) }) }
    )
    // Add chunk ref
    .post(
        "/sessions/:id/chunk-refs",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* sessionService.addChunkRef(ctx.params.id, session.user.id, ctx.body.chunkId, ctx.body.reason);
                    ctx.set.status = 201;
                    return { message: "Chunk reference added" };
                })
            ),
        { body: t.Object({ chunkId: t.String(), reason: t.String() }) }
    )
    // Add assumption
    .post(
        "/sessions/:id/assumptions",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* sessionService.addAssumption(ctx.params.id, session.user.id, ctx.body.description);
                    ctx.set.status = 201;
                    return { message: "Assumption recorded" };
                })
            ),
        { body: t.Object({ description: t.String() }) }
    )
    // Resolve assumption
    .patch(
        "/sessions/:id/assumptions/:assumptionId",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* sessionService.resolveAssumption(ctx.params.id, ctx.params.assumptionId, session.user.id, ctx.body);
                    return { message: "Assumption resolved" };
                })
            ),
        { body: t.Object({ resolved: t.Boolean(), resolution: t.Optional(t.String()) }) }
    )
    // Add requirement ref
    .post(
        "/sessions/:id/requirement-refs",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* sessionService.addRequirementRef(
                        ctx.params.id,
                        session.user.id,
                        ctx.body.requirementId,
                        ctx.body.stepsAddressed
                    );
                    ctx.set.status = 201;
                    return { message: "Requirement reference added" };
                })
            ),
        { body: t.Object({ requirementId: t.String(), stepsAddressed: t.Optional(t.Array(t.Number())) }) }
    )
    // Review session
    .patch(
        "/sessions/:id/review",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* sessionService.reviewSession(ctx.params.id, session.user.id, ctx.body.requirementStatuses);
                })
            ),
        {
            body: t.Object({
                requirementStatuses: t.Optional(
                    t.Array(
                        t.Object({
                            requirementId: t.String(),
                            status: StatusSchema
                        })
                    )
                )
            })
        }
    );
