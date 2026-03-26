import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as planService from "./service";

export const planRoutes = new Elysia()
    .get(
        "/plans",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.listPlans(session.user.id, ctx.query.codebaseId, ctx.query.status)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                status: t.Optional(t.String())
            })
        }
    )
    .post(
        "/plans",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.createPlan(session.user.id, ctx.body)
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                title: t.String({ maxLength: 200 }),
                description: t.Optional(t.String({ maxLength: 5000 })),
                status: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                template: t.Optional(t.String()),
                steps: t.Optional(
                    t.Array(
                        t.Object({
                            description: t.String({ maxLength: 2000 }),
                            order: t.Optional(t.Number()),
                            parentStepId: t.Optional(t.String()),
                            note: t.Optional(t.String({ maxLength: 2000 })),
                            chunkId: t.Optional(t.String()),
                            requirementId: t.Optional(t.String())
                        })
                    )
                )
            })
        }
    )
    .get("/plans/templates", () => ({ templates: planService.listPlanTemplates() }))
    .post(
        "/plans/generate-from-requirements",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.generatePlanFromRequirements(session.user.id, ctx.body)
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                title: t.String({ maxLength: 200 }),
                description: t.Optional(t.String({ maxLength: 5000 })),
                requirementIds: t.Array(t.String()),
                codebaseId: t.Optional(t.String()),
                template: t.Optional(t.Union([t.Literal("standard"), t.Literal("detailed")]))
            })
        }
    )
    .get("/plans/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    planService.getPlanDetail(ctx.params.id, session.user.id)
                )
            )
        )
    )
    .patch(
        "/plans/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.updatePlan(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        {
            body: t.Object({
                title: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 5000 }), t.Null()])),
                status: t.Optional(t.String()),
                codebaseId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .delete("/plans/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    planService.deletePlan(ctx.params.id, session.user.id)
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    // ── Steps ──────────────────────────────────────────────────────
    .post(
        "/plans/:id/steps",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.addStep(ctx.params.id, session.user.id, ctx.body)
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                description: t.String({ maxLength: 2000 }),
                order: t.Optional(t.Number()),
                parentStepId: t.Optional(t.String()),
                note: t.Optional(t.String({ maxLength: 2000 })),
                chunkId: t.Optional(t.String()),
                requirementId: t.Optional(t.String())
            })
        }
    )
    .patch(
        "/plans/:id/steps/:stepId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.updateStep(
                            ctx.params.id,
                            ctx.params.stepId,
                            session.user.id,
                            ctx.body
                        )
                    )
                )
            ),
        {
            body: t.Object({
                description: t.Optional(t.String({ maxLength: 2000 })),
                status: t.Optional(t.String()),
                order: t.Optional(t.Number()),
                parentStepId: t.Optional(t.Union([t.String(), t.Null()])),
                note: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                chunkId: t.Optional(t.Union([t.String(), t.Null()])),
                requirementId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .post(
        "/plans/:id/steps/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.reorderSteps(ctx.params.id, session.user.id, ctx.body.stepIds)
                    )
                )
            ),
        {
            body: t.Object({
                stepIds: t.Array(t.String())
            })
        }
    )
    .delete("/plans/:id/steps/:stepId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    planService.deleteStep(ctx.params.id, ctx.params.stepId, session.user.id)
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    // ── Chunk Refs ─────────────────────────────────────────────────
    .post(
        "/plans/:id/chunks",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.addPlanChunkRef(ctx.params.id, session.user.id, ctx.body)
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                chunkId: t.String(),
                relation: t.Optional(t.String())
            })
        }
    )
    .delete("/plans/:id/chunks/:refId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    planService.removePlanChunkRef(
                        ctx.params.id,
                        ctx.params.refId,
                        session.user.id
                    )
                ),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
