import { Elysia, t } from "elysia";
import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";
import type { PlanTaskChunkRelation, PlanTaskStatus } from "@fubbik/db/schema/plan";

import { requireSession } from "../require-session";
import { ValidationError } from "../errors";
import { createActivity } from "../activity/service";
import { VALID_TASK_RELATIONS, getPlan } from "./service";

const VALID_TASK_STATUSES: PlanTaskStatus[] = ["pending", "in_progress", "done", "skipped", "blocked"];

function validateStatus(status: string): Effect.Effect<PlanTaskStatus, ValidationError> {
    if (!VALID_TASK_STATUSES.includes(status as PlanTaskStatus)) {
        return Effect.fail(new ValidationError({ message: `Invalid task status: ${status}` }));
    }
    return Effect.succeed(status as PlanTaskStatus);
}

function validateRelation(rel: string): Effect.Effect<PlanTaskChunkRelation, ValidationError> {
    if (!VALID_TASK_RELATIONS.includes(rel as PlanTaskChunkRelation)) {
        return Effect.fail(new ValidationError({ message: `Invalid task chunk relation: ${rel}` }));
    }
    return Effect.succeed(rel as PlanTaskChunkRelation);
}

export const planTaskRoutes = new Elysia({ prefix: "/plans/:id/tasks" })
    .post(
        "/",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        Effect.gen(function* () {
                            const plan = yield* getPlan(ctx.params.id);
                            const task = yield* planRepo.createTask({
                                planId: ctx.params.id,
                                title: ctx.body.title,
                                description: ctx.body.description ?? null,
                                acceptanceCriteria: ctx.body.acceptanceCriteria ?? [],
                                status: "pending",
                                metadata: ctx.body.metadata ?? {},
                            });
                            if (ctx.body.chunks) {
                                for (const c of ctx.body.chunks) {
                                    const rel = yield* validateRelation(c.relation);
                                    yield* planRepo.addTaskChunk(task.id, c.chunkId, rel);
                                }
                            }
                            if (ctx.body.dependsOnTaskIds) {
                                for (const depId of ctx.body.dependsOnTaskIds) {
                                    yield* planRepo.addTaskDependency(task.id, depId);
                                }
                            }
                            yield* createActivity({
                                userId: session.user.id,
                                entityType: "plan_task",
                                entityId: task.id,
                                entityTitle: task.title,
                                action: "created",
                                codebaseId: plan.codebaseId ?? undefined,
                            });
                            return task;
                        }),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                title: t.String(),
                description: t.Optional(t.String()),
                acceptanceCriteria: t.Optional(t.Array(t.String())),
                chunks: t.Optional(t.Array(t.Object({ chunkId: t.String(), relation: t.String() }))),
                dependsOnTaskIds: t.Optional(t.Array(t.String())),
                metadata: t.Optional(t.Record(t.String(), t.Unknown())),
            }),
        },
    )
    .patch(
        "/:taskId",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        Effect.gen(function* () {
                            const plan = yield* getPlan(ctx.params.id);
                            const patch: Record<string, unknown> = {};
                            if (ctx.body.title !== undefined) patch.title = ctx.body.title;
                            if (ctx.body.description !== undefined) patch.description = ctx.body.description;
                            if (ctx.body.acceptanceCriteria !== undefined)
                                patch.acceptanceCriteria = ctx.body.acceptanceCriteria;
                            if (ctx.body.metadata !== undefined) patch.metadata = ctx.body.metadata;
                            let markedDone = false;
                            if (ctx.body.status !== undefined) {
                                const status = yield* validateStatus(ctx.body.status);
                                patch.status = status;
                                markedDone = status === "done";
                            }
                            const updated = yield* planRepo.updateTask(ctx.params.taskId, patch);
                            if (markedDone) {
                                yield* planRepo.unblockDependentsOf(ctx.params.taskId);
                            }
                            yield* createActivity({
                                userId: session.user.id,
                                entityType: "plan_task",
                                entityId: updated.id,
                                entityTitle: updated.title,
                                action: ctx.body.status !== undefined ? "status_changed" : "updated",
                                codebaseId: plan.codebaseId ?? undefined,
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
                acceptanceCriteria: t.Optional(t.Array(t.String())),
                status: t.Optional(t.String()),
                metadata: t.Optional(t.Record(t.String(), t.Unknown())),
            }),
        },
    )
    .delete("/:taskId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    Effect.gen(function* () {
                        const plan = yield* getPlan(ctx.params.id);
                        yield* planRepo.deleteTask(ctx.params.taskId);
                        yield* createActivity({
                            userId: session.user.id,
                            entityType: "plan_task",
                            entityId: ctx.params.taskId,
                            action: "deleted",
                            codebaseId: plan.codebaseId ?? undefined,
                        });
                    }),
                ),
            ),
        );
        return { ok: true };
    })
    .post(
        "/reorder",
        async ctx => {
            await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => planRepo.reorderTasks(ctx.params.id, ctx.body.taskIds)),
                ),
            );
            return { ok: true };
        },
        { body: t.Object({ taskIds: t.Array(t.String()) }) },
    )
    .post(
        "/:taskId/chunks",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => validateRelation(ctx.body.relation)),
                    Effect.flatMap(rel => planRepo.addTaskChunk(ctx.params.taskId, ctx.body.chunkId, rel)),
                ),
            );
        },
        { body: t.Object({ chunkId: t.String(), relation: t.String() }) },
    )
    .delete("/:taskId/chunks/:linkId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.removeTaskChunk(ctx.params.linkId)),
            ),
        );
        return { ok: true };
    })
    // Task external links — parallels plan /links
    .get("/:taskId/links", async ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.listTaskLinks(ctx.params.taskId)),
            ),
        ),
    )
    .post(
        "/:taskId/links",
        async ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() =>
                        planRepo.addTaskLink({
                            taskId: ctx.params.taskId,
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
    .delete("/:taskId/links/:linkId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.removeTaskLink(ctx.params.linkId)),
            ),
        );
        return { ok: true };
    });
