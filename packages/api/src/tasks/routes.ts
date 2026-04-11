import { Effect } from "effect";
import { Elysia, t } from "elysia";

import * as planRepo from "@fubbik/db/repository/plan";

import { requireSession } from "../require-session";
import * as planService from "../plans/service";

export const taskQueueRoutes = new Elysia()
    // Create a task (creates a single-task plan)
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
                            tasks: [{ title: ctx.body.title }],
                        }),
                    ),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                title: t.String({ maxLength: 200 }),
                description: t.Optional(t.String()),
                priority: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
            }),
        },
    )
    // List open tasks (in_progress plans)
    .get("/tasks", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    planService.listPlans({ userId: session.user.id, status: "in_progress" }),
                ),
            ),
        ),
    )
    // Claim a task (mark first plan_task as in_progress)
    .post("/tasks/:id/claim", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() =>
                    Effect.gen(function* () {
                        const detail = yield* planService.getPlanDetail(ctx.params.id);
                        const firstTask = detail.tasks[0];
                        if (firstTask) {
                            yield* planRepo.updateTask(firstTask.id, { status: "in_progress" });
                        }
                        return yield* planService.getPlanDetail(ctx.params.id);
                    }),
                ),
            ),
        ),
    )
    // Complete a task
    .post(
        "/tasks/:id/complete",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        Effect.gen(function* () {
                            const detail = yield* planService.getPlanDetail(ctx.params.id);
                            const firstTask = detail.tasks[0];
                            if (firstTask) {
                                yield* planRepo.updateTask(firstTask.id, { status: "done" });
                            }
                            return yield* planService.updatePlan(ctx.params.id, { status: "completed" });
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                note: t.Optional(t.String()),
            }),
        },
    );
