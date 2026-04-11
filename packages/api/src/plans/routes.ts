import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as planService from "./service";

const planBase = new Elysia({ prefix: "/api/plans" })
    .get(
        "/",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.listPlans({
                            userId: session.user.id,
                            codebaseId: ctx.query.codebaseId,
                            status: ctx.query.status,
                            requirementId: ctx.query.requirementId,
                            includeArchived: ctx.query.includeArchived === "true",
                        }),
                    ),
                ),
            );
        },
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                status: t.Optional(t.String()),
                requirementId: t.Optional(t.String()),
                includeArchived: t.Optional(t.String()),
            }),
        },
    )
    .get("/:id", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(() => planService.getPlanDetail(ctx.params.id))),
        );
    })
    .post(
        "/",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => planService.createPlan(session.user.id, ctx.body)),
                ),
            );
        },
        {
            body: t.Object({
                title: t.String(),
                description: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                requirementIds: t.Optional(t.Array(t.String())),
                tasks: t.Optional(
                    t.Array(
                        t.Object({
                            title: t.String(),
                            description: t.Optional(t.String()),
                            acceptanceCriteria: t.Optional(t.Array(t.String())),
                        }),
                    ),
                ),
            }),
        },
    )
    .patch(
        "/:id",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(() => planService.updatePlan(ctx.params.id, ctx.body))),
            );
        },
        {
            body: t.Object({
                title: t.Optional(t.String()),
                description: t.Optional(t.Union([t.String(), t.Null()])),
                status: t.Optional(t.String()),
                codebaseId: t.Optional(t.Union([t.String(), t.Null()])),
            }),
        },
    )
    .delete("/:id", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(() => planService.deletePlan(ctx.params.id))),
        );
        return { ok: true };
    });

import { planRequirementRoutes } from "./requirements";
import { planAnalyzeRoutes } from "./analyze";
import { planTaskRoutes } from "./tasks";

export const planRoutes = new Elysia()
    .use(planBase)
    .use(planRequirementRoutes)
    .use(planAnalyzeRoutes)
    .use(planTaskRoutes);
