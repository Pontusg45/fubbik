import { Effect } from "effect";
import { Elysia, t } from "elysia";

import * as planRepo from "@fubbik/db/repository/plan";
import { requireSession } from "../require-session";
import { createActivity } from "../activity/service";
import * as planService from "./service";

const planBase = new Elysia({ prefix: "/plans" })
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
                    Effect.flatMap(session =>
                        Effect.gen(function* () {
                            const created = yield* planService.createPlan(session.user.id, ctx.body);
                            yield* createActivity({
                                userId: session.user.id,
                                entityType: "plan",
                                entityId: created.id,
                                entityTitle: created.title,
                                action: "created",
                                codebaseId: created.codebaseId ?? undefined,
                            });
                            return created;
                        }),
                    ),
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
                metadata: t.Optional(t.Record(t.String(), t.Unknown())),
            }),
        },
    )
    .patch(
        "/:id",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        Effect.gen(function* () {
                            const updated = yield* planService.updatePlan(ctx.params.id, ctx.body);
                            const action = ctx.body.status !== undefined ? "status_changed" : "updated";
                            yield* createActivity({
                                userId: session.user.id,
                                entityType: "plan",
                                entityId: updated.id,
                                entityTitle: updated.title,
                                action,
                                codebaseId: updated.codebaseId ?? undefined,
                            });
                            return updated;
                        }),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                title: t.Optional(t.String()),
                description: t.Optional(t.Union([t.String(), t.Null()])),
                status: t.Optional(t.String()),
                codebaseId: t.Optional(t.Union([t.String(), t.Null()])),
                metadata: t.Optional(t.Record(t.String(), t.Unknown())),
            }),
        },
    )
    .delete("/:id", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    Effect.gen(function* () {
                        const existing = yield* planService.getPlan(ctx.params.id);
                        yield* planService.deletePlan(ctx.params.id);
                        yield* createActivity({
                            userId: session.user.id,
                            entityType: "plan",
                            entityId: existing.id,
                            entityTitle: existing.title,
                            action: "deleted",
                            codebaseId: existing.codebaseId ?? undefined,
                        });
                    }),
                ),
            ),
        );
        return { ok: true };
    })
    // External links on the plan itself
    .get("/:id/links", async ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => planService.getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.listPlanLinks(ctx.params.id)),
            ),
        ),
    )
    .post(
        "/:id/links",
        async ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => planService.getPlan(ctx.params.id)),
                    Effect.flatMap(() =>
                        planRepo.addPlanLink({
                            planId: ctx.params.id,
                            system: ctx.body.system ?? "url",
                            url: ctx.body.url,
                            label: ctx.body.label ?? null,
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                url: t.String({ maxLength: 2000 }),
                system: t.Optional(t.String({ maxLength: 40 })),
                label: t.Optional(t.String({ maxLength: 200 })),
            }),
        },
    )
    .delete("/:id/links/:linkId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => planService.getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.removePlanLink(ctx.params.linkId)),
            ),
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
