import { and, asc, eq, inArray, ne, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { codebase } from "../schema/codebase";
import {
    plan,
    planAnalyzeItem,
    planRequirement,
    planTask,
    planTaskChunk,
    planTaskDependency,
    type NewPlan,
    type NewPlanAnalyzeItem,
    type NewPlanTask,
    type Plan,
    type PlanAnalyzeItem,
    type PlanAnalyzeKind,
    type PlanRequirement,
    type PlanStatus,
    type PlanTask,
    type PlanTaskChunk,
    type PlanTaskChunkRelation,
    type PlanTaskDependency,
    type PlanTaskStatus,
} from "../schema/plan";

// --- Plan CRUD ---

export interface ListPlansFilter {
    userId: string;
    codebaseId?: string;
    status?: PlanStatus;
    requirementId?: string;
    includeArchived?: boolean;
}

export function listPlans(filter: ListPlansFilter): Effect.Effect<Plan[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.userId, filter.userId)];
            if (filter.codebaseId) conditions.push(eq(plan.codebaseId, filter.codebaseId));
            if (filter.status) conditions.push(eq(plan.status, filter.status));
            if (!filter.includeArchived && !filter.status) {
                conditions.push(ne(plan.status, "archived"));
            }
            if (filter.requirementId) {
                const reqPlanIds = await db
                    .select({ planId: planRequirement.planId })
                    .from(planRequirement)
                    .where(eq(planRequirement.requirementId, filter.requirementId));
                const ids = reqPlanIds.map(r => r.planId);
                if (ids.length === 0) return [];
                conditions.push(inArray(plan.id, ids));
            }
            return db.select().from(plan).where(and(...conditions)).orderBy(asc(plan.createdAt));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export interface PlanListRow extends Plan {
    codebaseName: string | null;
    taskTotal: number;
    taskDone: number;
    nextAction: string | null;
    lastActivityAt: Date;
}

/**
 * listPlans + per-row rollups used by `/plans` (list page):
 *   - codebase name (for the codebase chip)
 *   - total/done task counts (for progress bar)
 *   - title of the first non-`done` task (for the "Next:" line)
 *   - last-activity timestamp (max of plan.updated_at and task.updated_at)
 * Keeping this as a separate function so existing callers of `listPlans` keep
 * their lean response shape.
 */
export function listPlansWithRollups(filter: ListPlansFilter): Effect.Effect<PlanListRow[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.userId, filter.userId)];
            if (filter.codebaseId) conditions.push(eq(plan.codebaseId, filter.codebaseId));
            if (filter.status) conditions.push(eq(plan.status, filter.status));
            if (!filter.includeArchived && !filter.status) {
                conditions.push(ne(plan.status, "archived"));
            }
            if (filter.requirementId) {
                const reqPlanIds = await db
                    .select({ planId: planRequirement.planId })
                    .from(planRequirement)
                    .where(eq(planRequirement.requirementId, filter.requirementId));
                const ids = reqPlanIds.map(r => r.planId);
                if (ids.length === 0) return [];
                conditions.push(inArray(plan.id, ids));
            }

            const rows = await db
                .select({
                    plan,
                    codebaseName: codebase.name,
                    taskTotal: sql<number>`count(${planTask.id})::int`.as("task_total"),
                    taskDone: sql<number>`count(*) filter (where ${planTask.status} = 'done')::int`.as("task_done"),
                    lastTaskUpdate: sql<Date | null>`max(${planTask.updatedAt})`.as("last_task_update"),
                })
                .from(plan)
                .leftJoin(codebase, eq(codebase.id, plan.codebaseId))
                .leftJoin(planTask, eq(planTask.planId, plan.id))
                .where(and(...conditions))
                .groupBy(plan.id, codebase.name)
                .orderBy(asc(plan.createdAt));

            if (rows.length === 0) return [];

            // Second pass: first non-done task per plan for `nextAction`.
            const planIds = rows.map(r => r.plan.id);
            const nextActionRows = await db.execute(sql`
                SELECT DISTINCT ON (plan_id) plan_id, title
                FROM plan_task
                WHERE plan_id IN (${sql.join(planIds.map(id => sql`${id}`), sql`, `)})
                  AND status != 'done'
                ORDER BY plan_id, "order"
            `);
            const nextActionMap = new Map<string, string>();
            for (const row of nextActionRows.rows as Array<{ plan_id: string; title: string }>) {
                nextActionMap.set(row.plan_id, row.title);
            }

            return rows.map(r => ({
                ...r.plan,
                codebaseName: r.codebaseName ?? null,
                taskTotal: Number(r.taskTotal) || 0,
                taskDone: Number(r.taskDone) || 0,
                nextAction: nextActionMap.get(r.plan.id) ?? null,
                lastActivityAt:
                    r.lastTaskUpdate && r.lastTaskUpdate > r.plan.updatedAt
                        ? r.lastTaskUpdate
                        : r.plan.updatedAt,
            }));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function getPlan(id: string): Effect.Effect<Plan | null, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(plan).where(eq(plan.id, id)).limit(1);
            return row ?? null;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function createPlan(input: NewPlan): Effect.Effect<Plan, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(plan).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function updatePlan(id: string, patch: Partial<NewPlan>): Effect.Effect<Plan, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(plan)
                .set({ ...patch, updatedAt: new Date() })
                .where(eq(plan.id, id))
                .returning();
            if (!row) throw new Error("Plan not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

/**
 * Deep-copy a plan and everything underneath it: requirements, analyze items,
 * tasks (reset to `pending`), task-chunk links, and task dependencies. The new
 * plan is created as a `draft` with " (copy)" appended to the title.
 */
export function duplicatePlan(sourceId: string, userId: string): Effect.Effect<Plan, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            return await db.transaction(async tx => {
                const [source] = await tx
                    .select()
                    .from(plan)
                    .where(and(eq(plan.id, sourceId), eq(plan.userId, userId)));
                if (!source) throw new Error("Source plan not found");

                const newPlanId = crypto.randomUUID();
                const [newPlan] = await tx
                    .insert(plan)
                    .values({
                        id: newPlanId,
                        title: `${source.title} (copy)`,
                        description: source.description,
                        status: "draft",
                        userId: source.userId,
                        codebaseId: source.codebaseId,
                        metadata: source.metadata,
                    })
                    .returning();
                if (!newPlan) throw new Error("Insert returned no row");

                // Requirements (preserve order).
                const reqs = await tx
                    .select()
                    .from(planRequirement)
                    .where(eq(planRequirement.planId, sourceId));
                if (reqs.length > 0) {
                    await tx.insert(planRequirement).values(
                        reqs.map(r => ({ planId: newPlanId, requirementId: r.requirementId, order: r.order }))
                    );
                }

                // Analyze items.
                const items = await tx
                    .select()
                    .from(planAnalyzeItem)
                    .where(eq(planAnalyzeItem.planId, sourceId));
                if (items.length > 0) {
                    await tx.insert(planAnalyzeItem).values(
                        items.map(i => ({
                            id: crypto.randomUUID(),
                            planId: newPlanId,
                            kind: i.kind,
                            text: i.text,
                            chunkId: i.chunkId,
                            filePath: i.filePath,
                            metadata: i.metadata as Record<string, unknown>,
                            order: i.order,
                        }))
                    );
                }

                // Tasks — remap ids so we can rewrite chunk + dependency refs.
                const sourceTasks = await tx
                    .select()
                    .from(planTask)
                    .where(eq(planTask.planId, sourceId));
                const taskIdMap = new Map<string, string>();
                for (const t of sourceTasks) taskIdMap.set(t.id, crypto.randomUUID());
                if (sourceTasks.length > 0) {
                    await tx.insert(planTask).values(
                        sourceTasks.map(t => ({
                            id: taskIdMap.get(t.id)!,
                            planId: newPlanId,
                            title: t.title,
                            description: t.description,
                            acceptanceCriteria: t.acceptanceCriteria,
                            status: "pending" as PlanTaskStatus,
                            order: t.order,
                            metadata: t.metadata,
                        }))
                    );
                }

                // Task → chunk links.
                if (sourceTasks.length > 0) {
                    const sourceTaskIds = sourceTasks.map(t => t.id);
                    const links = await tx
                        .select()
                        .from(planTaskChunk)
                        .where(inArray(planTaskChunk.taskId, sourceTaskIds));
                    if (links.length > 0) {
                        await tx.insert(planTaskChunk).values(
                            links
                                .filter(l => taskIdMap.has(l.taskId))
                                .map(l => ({
                                    id: crypto.randomUUID(),
                                    taskId: taskIdMap.get(l.taskId)!,
                                    chunkId: l.chunkId,
                                    relation: l.relation,
                                }))
                        );
                    }

                    // Task dependencies.
                    const deps = await tx
                        .select()
                        .from(planTaskDependency)
                        .where(inArray(planTaskDependency.taskId, sourceTaskIds));
                    if (deps.length > 0) {
                        await tx.insert(planTaskDependency).values(
                            deps
                                .filter(d => taskIdMap.has(d.taskId) && taskIdMap.has(d.dependsOnTaskId))
                                .map(d => ({
                                    id: crypto.randomUUID(),
                                    taskId: taskIdMap.get(d.taskId)!,
                                    dependsOnTaskId: taskIdMap.get(d.dependsOnTaskId)!,
                                }))
                        );
                    }
                }

                return newPlan;
            });
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function deletePlan(id: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(plan).where(eq(plan.id, id));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

// --- Requirements links ---

export function listPlanRequirements(planId: string): Effect.Effect<PlanRequirement[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planRequirement)
                .where(eq(planRequirement.planId, planId))
                .orderBy(asc(planRequirement.order)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function addPlanRequirement(planId: string, requirementId: string): Effect.Effect<PlanRequirement, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const existing = await db
                .select({ order: planRequirement.order })
                .from(planRequirement)
                .where(eq(planRequirement.planId, planId));
            const maxOrder = existing.reduce((m, r) => Math.max(m, r.order), -1);
            const [row] = await db
                .insert(planRequirement)
                .values({ planId, requirementId, order: maxOrder + 1 })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function removePlanRequirement(planId: string, requirementId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db
                .delete(planRequirement)
                .where(and(eq(planRequirement.planId, planId), eq(planRequirement.requirementId, requirementId)));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function reorderPlanRequirements(planId: string, requirementIds: string[]): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (let i = 0; i < requirementIds.length; i++) {
                    const rid = requirementIds[i];
                    if (!rid) continue;
                    await tx
                        .update(planRequirement)
                        .set({ order: i })
                        .where(and(eq(planRequirement.planId, planId), eq(planRequirement.requirementId, rid)));
                }
            });
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

// --- Analyze items ---

export function listAnalyzeItems(planId: string): Effect.Effect<PlanAnalyzeItem[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planAnalyzeItem)
                .where(eq(planAnalyzeItem.planId, planId))
                .orderBy(asc(planAnalyzeItem.kind), asc(planAnalyzeItem.order)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function createAnalyzeItem(input: NewPlanAnalyzeItem): Effect.Effect<PlanAnalyzeItem, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const existing = await db
                .select({ order: planAnalyzeItem.order })
                .from(planAnalyzeItem)
                .where(and(eq(planAnalyzeItem.planId, input.planId), eq(planAnalyzeItem.kind, input.kind)));
            const maxOrder = existing.reduce((m, r) => Math.max(m, r.order), -1);
            const [row] = await db
                .insert(planAnalyzeItem)
                .values({ ...input, order: maxOrder + 1 })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function updateAnalyzeItem(
    itemId: string,
    patch: Partial<NewPlanAnalyzeItem>,
): Effect.Effect<PlanAnalyzeItem, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(planAnalyzeItem)
                .set({ ...patch, updatedAt: new Date() })
                .where(eq(planAnalyzeItem.id, itemId))
                .returning();
            if (!row) throw new Error("Analyze item not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function deleteAnalyzeItem(itemId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planAnalyzeItem).where(eq(planAnalyzeItem.id, itemId));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function reorderAnalyzeItems(
    planId: string,
    kind: PlanAnalyzeKind,
    itemIds: string[],
): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (let i = 0; i < itemIds.length; i++) {
                    const id = itemIds[i];
                    if (!id) continue;
                    await tx
                        .update(planAnalyzeItem)
                        .set({ order: i })
                        .where(
                            and(
                                eq(planAnalyzeItem.id, id),
                                eq(planAnalyzeItem.planId, planId),
                                eq(planAnalyzeItem.kind, kind),
                            ),
                        );
                }
            });
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

// --- Tasks ---

export function listTasks(planId: string): Effect.Effect<PlanTask[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planTask)
                .where(eq(planTask.planId, planId))
                .orderBy(asc(planTask.order)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function createTask(input: NewPlanTask): Effect.Effect<PlanTask, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const existing = await db
                .select({ order: planTask.order })
                .from(planTask)
                .where(eq(planTask.planId, input.planId));
            const maxOrder = existing.reduce((m, r) => Math.max(m, r.order), -1);
            const [row] = await db
                .insert(planTask)
                .values({ ...input, order: maxOrder + 1 })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function updateTask(taskId: string, patch: Partial<NewPlanTask>): Effect.Effect<PlanTask, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(planTask)
                .set({ ...patch, updatedAt: new Date() })
                .where(eq(planTask.id, taskId))
                .returning();
            if (!row) throw new Error("Task not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function deleteTask(taskId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTask).where(eq(planTask.id, taskId));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function reorderTasks(planId: string, taskIds: string[]): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (let i = 0; i < taskIds.length; i++) {
                    const id = taskIds[i];
                    if (!id) continue;
                    await tx
                        .update(planTask)
                        .set({ order: i })
                        .where(and(eq(planTask.id, id), eq(planTask.planId, planId)));
                }
            });
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

// --- Task chunk links ---

export function listTaskChunks(taskId: string): Effect.Effect<PlanTaskChunk[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => db.select().from(planTaskChunk).where(eq(planTaskChunk.taskId, taskId)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export interface PlanTaskChunkWithTitle extends PlanTaskChunk {
    chunkTitle: string | null;
    chunkType: string | null;
}

/**
 * Same as listTaskChunks but joins chunk to bring back the title + type so the
 * detail page can render a usable Link instead of just the hash fragment.
 */
export function listTaskChunksWithTitles(taskId: string): Effect.Effect<PlanTaskChunkWithTitle[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select({
                    id: planTaskChunk.id,
                    taskId: planTaskChunk.taskId,
                    chunkId: planTaskChunk.chunkId,
                    relation: planTaskChunk.relation,
                    createdAt: planTaskChunk.createdAt,
                    chunkTitle: chunk.title,
                    chunkType: chunk.type,
                })
                .from(planTaskChunk)
                .leftJoin(chunk, eq(chunk.id, planTaskChunk.chunkId))
                .where(eq(planTaskChunk.taskId, taskId)) as unknown as Promise<PlanTaskChunkWithTitle[]>,
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function addTaskChunk(
    taskId: string,
    chunkId: string,
    relation: PlanTaskChunkRelation,
): Effect.Effect<PlanTaskChunk, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(planTaskChunk).values({ taskId, chunkId, relation }).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function removeTaskChunk(linkId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTaskChunk).where(eq(planTaskChunk.id, linkId));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

// --- Task dependencies ---

export function listTaskDependencies(planId: string): Effect.Effect<PlanTaskDependency[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select({
                    id: planTaskDependency.id,
                    taskId: planTaskDependency.taskId,
                    dependsOnTaskId: planTaskDependency.dependsOnTaskId,
                    createdAt: planTaskDependency.createdAt,
                })
                .from(planTaskDependency)
                .innerJoin(planTask, eq(planTask.id, planTaskDependency.taskId))
                .where(eq(planTask.planId, planId)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function addTaskDependency(
    taskId: string,
    dependsOnTaskId: string,
): Effect.Effect<PlanTaskDependency, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .insert(planTaskDependency)
                .values({ taskId, dependsOnTaskId })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function removeTaskDependency(depId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTaskDependency).where(eq(planTaskDependency.id, depId));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

/**
 * When a task marks done, any tasks that depend on it and are currently
 * `blocked` should flip to `pending`. Returns the IDs of unblocked tasks.
 */
export function unblockDependentsOf(taskId: string): Effect.Effect<string[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const deps = await db
                .select({ dependent: planTaskDependency.taskId })
                .from(planTaskDependency)
                .where(eq(planTaskDependency.dependsOnTaskId, taskId));
            const dependentIds = deps.map(d => d.dependent);
            if (dependentIds.length === 0) return [];
            const result = await db
                .update(planTask)
                .set({ status: "pending", updatedAt: new Date() })
                .where(and(inArray(planTask.id, dependentIds), eq(planTask.status, "blocked")))
                .returning({ id: planTask.id });
            return result.map(r => r.id);
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export type PlanTaskStatusType = PlanTaskStatus;

// --- External links ---

import { planExternalLink, planTaskExternalLink } from "../schema/plan";
import type {
    NewPlanExternalLink,
    NewPlanTaskExternalLink,
    PlanExternalLink,
    PlanTaskExternalLink,
} from "../schema/plan";

export function listPlanLinks(planId: string): Effect.Effect<PlanExternalLink[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planExternalLink)
                .where(eq(planExternalLink.planId, planId))
                .orderBy(asc(planExternalLink.order), asc(planExternalLink.createdAt)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function addPlanLink(input: NewPlanExternalLink): Effect.Effect<PlanExternalLink, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(planExternalLink).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function removePlanLink(linkId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planExternalLink).where(eq(planExternalLink.id, linkId));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function listTaskLinks(taskId: string): Effect.Effect<PlanTaskExternalLink[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planTaskExternalLink)
                .where(eq(planTaskExternalLink.taskId, taskId))
                .orderBy(asc(planTaskExternalLink.order), asc(planTaskExternalLink.createdAt)),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function addTaskLink(input: NewPlanTaskExternalLink): Effect.Effect<PlanTaskExternalLink, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(planTaskExternalLink).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function removeTaskLink(linkId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTaskExternalLink).where(eq(planTaskExternalLink.id, linkId));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}
