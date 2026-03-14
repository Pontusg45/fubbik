import { and, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import type { RequirementStep } from "../schema/requirement";
import { requirement, requirementChunk } from "../schema/requirement";

export interface CreateRequirementParams {
    id: string;
    title: string;
    description?: string;
    steps: RequirementStep[];
    status?: string;
    priority?: string;
    codebaseId?: string;
    userId: string;
}

export function createRequirement(params: CreateRequirementParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(requirement).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getRequirementById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(requirement.id, id)];
            if (userId) conditions.push(eq(requirement.userId, userId));
            const [found] = await db
                .select()
                .from(requirement)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface ListRequirementsParams {
    userId: string;
    codebaseId?: string;
    status?: string;
    priority?: string;
    limit: number;
    offset: number;
}

export function listRequirements(params: ListRequirementsParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(requirement.userId, params.userId)];
            if (params.codebaseId) conditions.push(eq(requirement.codebaseId, params.codebaseId));
            if (params.status) conditions.push(eq(requirement.status, params.status));
            if (params.priority) conditions.push(eq(requirement.priority, params.priority));

            const requirements = await db
                .select()
                .from(requirement)
                .where(and(...conditions))
                .limit(params.limit)
                .offset(params.offset);

            const total = await db
                .select({ count: sql<number>`count(*)` })
                .from(requirement)
                .where(and(...conditions));

            return { requirements, total: Number(total[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateRequirementParams {
    title?: string;
    description?: string | null;
    steps?: RequirementStep[];
    status?: string;
    priority?: string | null;
    codebaseId?: string | null;
}

export function updateRequirement(id: string, userId: string, params: UpdateRequirementParams) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.title !== undefined) setClause.title = params.title;
            if (params.description !== undefined) setClause.description = params.description;
            if (params.steps !== undefined) setClause.steps = params.steps;
            if (params.status !== undefined) setClause.status = params.status;
            if (params.priority !== undefined) setClause.priority = params.priority;
            if (params.codebaseId !== undefined) setClause.codebaseId = params.codebaseId;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(requirement)
                    .where(and(eq(requirement.id, id), eq(requirement.userId, userId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(requirement)
                .set(setClause)
                .where(and(eq(requirement.id, id), eq(requirement.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteRequirement(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(requirement)
                .where(and(eq(requirement.id, id), eq(requirement.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateRequirementStatus(id: string, userId: string, status: string) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(requirement)
                .set({ status })
                .where(and(eq(requirement.id, id), eq(requirement.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function setRequirementChunks(requirementId: string, chunkIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(requirementChunk).where(eq(requirementChunk.requirementId, requirementId));
            if (chunkIds.length === 0) return [];
            return db
                .insert(requirementChunk)
                .values(chunkIds.map(chunkId => ({ requirementId, chunkId })))
                .onConflictDoNothing()
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunksForRequirement(requirementId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    content: chunk.content,
                    type: chunk.type
                })
                .from(requirementChunk)
                .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                .where(eq(requirementChunk.requirementId, requirementId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getRequirementStats(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(requirement.userId, userId)];
            if (codebaseId) conditions.push(eq(requirement.codebaseId, codebaseId));

            const results = await db
                .select({
                    status: requirement.status,
                    count: sql<number>`count(*)`
                })
                .from(requirement)
                .where(and(...conditions))
                .groupBy(requirement.status);

            const stats = { total: 0, passing: 0, failing: 0, untested: 0 };
            for (const row of results) {
                const count = Number(row.count);
                stats.total += count;
                if (row.status === "passing") stats.passing = count;
                else if (row.status === "failing") stats.failing = count;
                else if (row.status === "untested") stats.untested = count;
            }
            return stats;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
