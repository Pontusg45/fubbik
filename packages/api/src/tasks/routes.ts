import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as planService from "../plans/service";

export const taskQueueRoutes = new Elysia()
    // Create a task (creates a single-step plan)
    .post(
        "/tasks",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.createPlan(session.user.id, {
                            title: ctx.body.title,
                            description: ctx.body.description,
                            codebaseId: ctx.body.codebaseId,
                            steps: [{ description: ctx.body.title, order: 0 }]
                        })
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
                description: t.Optional(t.String()),
                priority: t.Optional(t.String()),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    // List open tasks (active single-step plans)
    .get("/tasks", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    planService.listPlans(session.user.id, undefined, "active")
                )
            )
        )
    )
    // Claim a task (mark step as in_progress)
    .post("/tasks/:id/claim", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    Effect.gen(function* () {
                        const detail = yield* planService.getPlanDetail(
                            ctx.params.id,
                            session.user.id
                        );
                        const firstStep = detail.steps[0];
                        if (!firstStep) return detail;
                        yield* planService.updateStep(
                            ctx.params.id,
                            firstStep.id,
                            session.user.id,
                            { status: "in_progress" }
                        );
                        return yield* planService.getPlanDetail(
                            ctx.params.id,
                            session.user.id
                        );
                    })
                )
            )
        )
    )
    // Complete a task
    .post(
        "/tasks/:id/complete",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        Effect.gen(function* () {
                            const detail = yield* planService.getPlanDetail(
                                ctx.params.id,
                                session.user.id
                            );
                            const firstStep = detail.steps[0];
                            if (firstStep) {
                                yield* planService.updateStep(
                                    ctx.params.id,
                                    firstStep.id,
                                    session.user.id,
                                    {
                                        status: "done",
                                        note: ctx.body?.note
                                    }
                                );
                            }
                            return yield* planService.updatePlan(
                                ctx.params.id,
                                session.user.id,
                                { status: "completed" }
                            );
                        })
                    )
                )
            ),
        {
            body: t.Object({
                note: t.Optional(t.String())
            })
        }
    );
