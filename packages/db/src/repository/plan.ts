import { and, asc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { plan, planChunkRef, planStep } from "../schema/plan";

// ── Plans ──────────────────────────────────────────────────────────

export interface CreatePlanParams {
    id: string;
    title: string;
    description?: string;
    status?: string;
    userId: string;
    codebaseId?: string;
}

export function createPlan(params: CreatePlanParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(plan).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getPlanById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.id, id)];
            if (userId) conditions.push(eq(plan.userId, userId));
            const [found] = await db
                .select()
                .from(plan)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listPlans(userId: string, codebaseId?: string, status?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.userId, userId)];
            if (codebaseId) conditions.push(eq(plan.codebaseId, codebaseId));
            if (status) conditions.push(eq(plan.status, status));

            const plans = await db
                .select({
                    id: plan.id,
                    title: plan.title,
                    description: plan.description,
                    status: plan.status,
                    userId: plan.userId,
                    codebaseId: plan.codebaseId,
                    createdAt: plan.createdAt,
                    updatedAt: plan.updatedAt,
                    totalSteps: sql<number>`(SELECT count(*) FROM plan_step ps WHERE ps.plan_id = ${plan.id})`.as("total_steps"),
                    doneCount: sql<number>`(SELECT count(*) FROM plan_step ps WHERE ps.plan_id = ${plan.id} AND ps.status = 'done')`.as("done_count")
                })
                .from(plan)
                .where(and(...conditions))
                .orderBy(asc(plan.createdAt));

            return plans.map(p => ({
                ...p,
                totalSteps: Number(p.totalSteps),
                doneCount: Number(p.doneCount)
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdatePlanParams {
    title?: string;
    description?: string | null;
    status?: string;
    codebaseId?: string | null;
}

export function updatePlan(id: string, userId: string, params: UpdatePlanParams) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.title !== undefined) setClause.title = params.title;
            if (params.description !== undefined) setClause.description = params.description;
            if (params.status !== undefined) setClause.status = params.status;
            if (params.codebaseId !== undefined) setClause.codebaseId = params.codebaseId;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(plan)
                    .where(and(eq(plan.id, id), eq(plan.userId, userId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(plan)
                .set(setClause)
                .where(and(eq(plan.id, id), eq(plan.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deletePlan(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(plan)
                .where(and(eq(plan.id, id), eq(plan.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

// ── Steps ──────────────────────────────────────────────────────────

export interface CreateStepParams {
    id: string;
    planId: string;
    description: string;
    status?: string;
    order: number;
    parentStepId?: string;
    note?: string;
    chunkId?: string;
    requirementId?: string;
}

export function getStepsForPlan(planId: string) {
    return Effect.tryPromise({
        try: async () => {
            return db
                .select()
                .from(planStep)
                .where(eq(planStep.planId, planId))
                .orderBy(asc(planStep.order));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createStep(params: CreateStepParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(planStep).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateStepParams {
    description?: string;
    status?: string;
    order?: number;
    parentStepId?: string | null;
    note?: string | null;
    chunkId?: string | null;
    requirementId?: string | null;
}

export function updateStep(stepId: string, planId: string, params: UpdateStepParams) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.description !== undefined) setClause.description = params.description;
            if (params.status !== undefined) setClause.status = params.status;
            if (params.order !== undefined) setClause.order = params.order;
            if (params.parentStepId !== undefined) setClause.parentStepId = params.parentStepId;
            if (params.note !== undefined) setClause.note = params.note;
            if (params.chunkId !== undefined) setClause.chunkId = params.chunkId;
            if (params.requirementId !== undefined) setClause.requirementId = params.requirementId;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(planStep)
                    .where(and(eq(planStep.id, stepId), eq(planStep.planId, planId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(planStep)
                .set(setClause)
                .where(and(eq(planStep.id, stepId), eq(planStep.planId, planId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteStep(stepId: string, planId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(planStep)
                .where(and(eq(planStep.id, stepId), eq(planStep.planId, planId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

// ── Chunk Refs ─────────────────────────────────────────────────────

export interface CreateChunkRefParams {
    id: string;
    planId: string;
    chunkId: string;
    relation?: string;
}

export function getChunkRefsForPlan(planId: string) {
    return Effect.tryPromise({
        try: async () => {
            return db
                .select()
                .from(planChunkRef)
                .where(eq(planChunkRef.planId, planId))
                .orderBy(asc(planChunkRef.createdAt));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function addPlanChunkRef(params: CreateChunkRefParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(planChunkRef).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function removePlanChunkRef(refId: string, planId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(planChunkRef)
                .where(and(eq(planChunkRef.id, refId), eq(planChunkRef.planId, planId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
