import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as proposalService from "./service";

export const proposalRoutes = new Elysia()
    // Create proposal for a chunk
    .post(
        "/chunks/:id/proposals",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.createProposal(ctx.params.id, session.user.id, {
                            changes: ctx.body.changes,
                            reason: ctx.body.reason,
                        })
                    )
                )
            );
        },
        {
            body: t.Object({
                changes: t.Object(
                    {
                        title: t.Optional(t.String()),
                        content: t.Optional(t.String()),
                        type: t.Optional(t.String()),
                        tags: t.Optional(t.Array(t.String())),
                        rationale: t.Optional(t.String()),
                        alternatives: t.Optional(t.Array(t.String())),
                        consequences: t.Optional(t.String()),
                        scope: t.Optional(t.Record(t.String(), t.String())),
                    },
                    { additionalProperties: false }
                ),
                reason: t.Optional(t.String()),
            }),
        }
    )
    // List proposals for a chunk
    .get(
        "/chunks/:id/proposals",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        proposalService.listProposalsForChunk(ctx.params.id, ctx.query.status)
                    )
                )
            );
        },
        {
            query: t.Object({
                status: t.Optional(t.String()),
            }),
        }
    )
    // Global proposal count — MUST be before /:proposalId
    .get("/proposals/count", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => proposalService.getPendingCount()),
                Effect.map(pending => ({ pending }))
            )
        );
    })
    // Bulk approve/reject — MUST be before /:proposalId
    .post(
        "/proposals/bulk",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.bulkAction(ctx.body.actions, session.user.id)
                    )
                )
            );
        },
        {
            body: t.Object({
                actions: t.Array(
                    t.Object({
                        proposalId: t.String(),
                        action: t.Union([t.Literal("approve"), t.Literal("reject")]),
                        note: t.Optional(t.String()),
                    })
                ),
            }),
        }
    )
    // Global queue
    .get(
        "/proposals",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        proposalService.listProposals({
                            chunkId: ctx.query.chunkId,
                            status: ctx.query.status,
                            limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
                            offset: ctx.query.offset ? Number(ctx.query.offset) : undefined,
                        })
                    )
                )
            );
        },
        {
            query: t.Object({
                status: t.Optional(t.String()),
                chunkId: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
            }),
        }
    )
    // Single proposal detail
    .get("/proposals/:proposalId", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => proposalService.getProposal(ctx.params.proposalId))
            )
        );
    })
    // Approve proposal
    .post(
        "/proposals/:proposalId/approve",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.approveProposal(
                            ctx.params.proposalId,
                            session.user.id,
                            ctx.body.note
                        )
                    )
                )
            );
        },
        {
            body: t.Object({
                note: t.Optional(t.String()),
            }),
        }
    )
    // Reject proposal
    .post(
        "/proposals/:proposalId/reject",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.rejectProposal(
                            ctx.params.proposalId,
                            session.user.id,
                            ctx.body.note
                        )
                    )
                )
            );
        },
        {
            body: t.Object({
                note: t.Optional(t.String()),
            }),
        }
    );
