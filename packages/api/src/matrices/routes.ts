import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as matrixService from "./service";

export const matrixRoutes = new Elysia()
    .get(
        "/matrices",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        matrixService.listMatrices(session.user.id, {
                            spaceId: ctx.query.spaceId,
                            layer: ctx.query.layer
                        })
                    )
                )
            ),
        {
            query: t.Object({
                spaceId: t.Optional(t.String()),
                layer: t.Optional(t.String())
            })
        }
    )
    .post(
        "/matrices",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.createMatrix(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 200 }),
                layer: t.Union([t.Literal("invariant"), t.Literal("contract")]),
                description: t.Optional(t.String({ maxLength: 1000 })),
                spaceId: t.Optional(t.String())
            })
        }
    )
    .get("/matrices/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => matrixService.getMatrixDetail(ctx.params.id, session.user.id)))
        )
    )
    .get("/matrices/:id/view", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => matrixService.getMatrixViewService(ctx.params.id, session.user.id)))
        )
    )
    .patch(
        "/matrices/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => matrixService.updateMatrix(ctx.params.id, session.user.id, ctx.body)))
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()]))
            })
        }
    )
    .delete("/matrices/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.deleteMatrixService(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post(
        "/matrices/:id/dimensions",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.addDimension(ctx.params.id, session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 })
            })
        }
    )
    .patch(
        "/matrices/:id/dimensions/:dimId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.renameDimension(ctx.params.id, ctx.params.dimId, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 })
            })
        }
    )
    .delete("/matrices/:id/dimensions/:dimId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.removeDimension(ctx.params.id, ctx.params.dimId, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post(
        "/matrices/:id/dimensions/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.reorderDimensions(ctx.params.id, session.user.id, ctx.body.dimensionIds))
                )
            ),
        {
            body: t.Object({
                dimensionIds: t.Array(t.String())
            })
        }
    )
    .post(
        "/matrices/:id/rules",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.addRule(ctx.params.id, session.user.id, ctx.body)),
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
                description: t.Optional(t.String({ maxLength: 1000 })),
                category: t.Optional(t.String({ maxLength: 100 })),
                rationale: t.Optional(t.String({ maxLength: 2000 })),
                alternatives: t.Optional(t.String({ maxLength: 2000 })),
                consequences: t.Optional(t.String({ maxLength: 2000 })),
                counterexample: t.Optional(t.String({ maxLength: 2000 }))
            })
        }
    )
    .patch(
        "/matrices/:id/rules/:ruleId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.updateRule(ctx.params.id, ctx.params.ruleId, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                title: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
                category: t.Optional(t.Union([t.String({ maxLength: 100 }), t.Null()])),
                rationale: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                alternatives: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                consequences: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                counterexample: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()]))
            })
        }
    )
    .get("/matrices/:id/rules/:ruleId/history", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.getRuleHistory(ctx.params.id, ctx.params.ruleId, session.user.id))
            )
        )
    )
    .delete("/matrices/:id/rules/:ruleId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.removeRule(ctx.params.id, ctx.params.ruleId, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post(
        "/matrices/:id/rules/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.reorderRules(ctx.params.id, session.user.id, ctx.body.ruleIds))
                )
            ),
        {
            body: t.Object({
                ruleIds: t.Array(t.String())
            })
        }
    )
    .put(
        "/matrices/:id/cells",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(() => matrixService.toggleCell(ctx.body.ruleId, ctx.body.dimensionId)))
            ),
        {
            body: t.Object({
                ruleId: t.String(),
                dimensionId: t.String()
            })
        }
    )
    .post(
        "/matrices/:id/cells/:cellId/requirements",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => matrixService.linkRequirementToCell(ctx.params.cellId, ctx.body.requirementId)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                requirementId: t.String()
            })
        }
    )
    .delete("/matrices/:id/cells/:cellId/requirements/:reqId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => matrixService.unlinkRequirementFromCell(ctx.params.cellId, ctx.params.reqId)),
                Effect.map(() => ({ message: "Unlinked" }))
            )
        )
    )
    .get("/matrices/:id/cells/:cellId/requirements", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(() => matrixService.getRequirementsForCell(ctx.params.cellId))))
    )
    // --- Cell code links (behavior ↔ code) ---
    .post(
        "/matrices/:id/cells/:cellId/code",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => matrixService.linkCodeToCell(ctx.params.cellId, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                kind: t.Union([t.Literal("file"), t.Literal("symbol"), t.Literal("test")]),
                ref: t.String({ maxLength: 500 })
            })
        }
    )
    .delete("/matrices/:id/cells/:cellId/code/:codeId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => matrixService.unlinkCodeFromCell(ctx.params.cellId, ctx.params.codeId)),
                Effect.map(() => ({ message: "Unlinked" }))
            )
        )
    )
    .get("/matrices/:id/cells/:cellId/code", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(() => matrixService.getCodeForCell(ctx.params.cellId))))
    )
    // --- Cell test results (behavior verification) ---
    .post(
        "/matrices/:id/cells/:cellId/test-results",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => matrixService.recordTestResult(ctx.params.cellId, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                testRef: t.String({ maxLength: 500 }),
                status: t.Union([t.Literal("pass"), t.Literal("fail")]),
                detail: t.Optional(t.String({ maxLength: 2000 }))
            })
        }
    )
    .get("/matrices/:id/cells/:cellId/test-results", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(() => matrixService.getTestResultsForCell(ctx.params.cellId))))
    )
    // --- Reverse lookup: which behaviors govern a file path ---
    .get(
        "/matrices/behaviors-for-file",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => matrixService.getBehaviorsForCodePath(session.user.id, ctx.query.path)))
            ),
        {
            query: t.Object({
                path: t.String()
            })
        }
    );
