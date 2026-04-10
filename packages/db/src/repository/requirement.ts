import { and, asc, eq, ilike, inArray, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { ensureVertex, deleteVertex, createEdge, deleteEdgesFrom } from "../age/sync";
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
    useCaseId?: string;
    userId: string;
    origin?: string;
    reviewStatus?: string;
}

export function createRequirement(params: CreateRequirementParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(requirement).values(params).returning();
            await Effect.runPromise(
                ensureVertex("requirement", created!.id).pipe(
                    Effect.catchAll(() => Effect.succeed(undefined))
                )
            );
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
    useCaseId?: string;
    status?: string;
    priority?: string;
    origin?: string;
    reviewStatus?: string;
    search?: string;
    limit: number;
    offset: number;
}

export function listRequirements(params: ListRequirementsParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(requirement.userId, params.userId)];
            if (params.codebaseId) conditions.push(eq(requirement.codebaseId, params.codebaseId));
            if (params.useCaseId) conditions.push(eq(requirement.useCaseId, params.useCaseId));
            if (params.status) conditions.push(eq(requirement.status, params.status));
            if (params.priority) conditions.push(eq(requirement.priority, params.priority));
            if (params.origin) conditions.push(eq(requirement.origin, params.origin));
            if (params.reviewStatus) conditions.push(eq(requirement.reviewStatus, params.reviewStatus));
            if (params.search) {
                const escaped = params.search.replace(/[%_\\]/g, c => `\\${c}`);
                const pattern = `%${escaped}%`;
                conditions.push(
                    or(
                        ilike(requirement.title, pattern),
                        ilike(requirement.description, pattern)
                    )!
                );
            }

            const requirements = await db
                .select()
                .from(requirement)
                .where(and(...conditions))
                .orderBy(asc(requirement.order), asc(requirement.createdAt))
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
    useCaseId?: string | null;
    origin?: string;
    reviewStatus?: string;
    reviewedBy?: string | null;
    reviewedAt?: Date | null;
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
            if (params.useCaseId !== undefined) setClause.useCaseId = params.useCaseId;
            if (params.origin !== undefined) setClause.origin = params.origin;
            if (params.reviewStatus !== undefined) setClause.reviewStatus = params.reviewStatus;
            if (params.reviewedBy !== undefined) setClause.reviewedBy = params.reviewedBy;
            if (params.reviewedAt !== undefined) setClause.reviewedAt = params.reviewedAt;

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
            await Effect.runPromise(
                deleteVertex("requirement", id).pipe(
                    Effect.catchAll(() => Effect.succeed(undefined))
                )
            );
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
            const result = await db
                .insert(requirementChunk)
                .values(chunkIds.map(chunkId => ({ requirementId, chunkId })))
                .onConflictDoNothing()
                .returning();
            await Effect.runPromise(
                deleteEdgesFrom("covers", "requirement", requirementId).pipe(
                    Effect.flatMap(() =>
                        Effect.all(
                            chunkIds.map(chunkId =>
                                createEdge("covers", "requirement", requirementId, "chunk", chunkId)
                            ),
                            { concurrency: 1 }
                        )
                    ),
                    Effect.catchAll(() => Effect.succeed(undefined))
                )
            );
            return result;
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

export function bulkUpdateRequirements(
    ids: string[],
    userId: string,
    params: { status?: string; useCaseId?: string | null }
) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.status !== undefined) setClause.status = params.status;
            if (params.useCaseId !== undefined) setClause.useCaseId = params.useCaseId;

            if (Object.keys(setClause).length === 0) return 0;

            const result = await db
                .update(requirement)
                .set(setClause)
                .where(and(inArray(requirement.id, ids), eq(requirement.userId, userId)));
            return result.rowCount ?? 0;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function bulkDeleteRequirements(ids: string[], userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await db
                .delete(requirement)
                .where(and(inArray(requirement.id, ids), eq(requirement.userId, userId)));
            return result.rowCount ?? 0;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getRequirementsByIds(ids: string[], userId: string) {
    return Effect.tryPromise({
        try: () =>
            db.select({ id: requirement.id, useCaseId: requirement.useCaseId, title: requirement.title, status: requirement.status })
                .from(requirement)
                .where(and(inArray(requirement.id, ids), eq(requirement.userId, userId))),
        catch: cause => new DatabaseError({ cause })
    });
}

export function setRequirementOrder(requirementIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            for (let i = 0; i < requirementIds.length; i++) {
                await db
                    .update(requirement)
                    .set({ order: i })
                    .where(eq(requirement.id, requirementIds[i]!));
            }
            return requirementIds.length;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface RequirementForChunk {
    chunkId: string;
    id: string;
    title: string;
    status: string;
    priority: string | null;
    steps: Array<{ keyword: string; text: string; params?: Record<string, string> }>;
}

export function getRequirementsForChunks(chunkIds: string[]) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    chunkId: requirementChunk.chunkId,
                    id: requirement.id,
                    title: requirement.title,
                    status: requirement.status,
                    priority: requirement.priority,
                    steps: requirement.steps
                })
                .from(requirementChunk)
                .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                .where(inArray(requirementChunk.chunkId, chunkIds)),
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

export function searchRequirementTitles(prefix: string, limit = 10) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({ id: requirement.id, title: requirement.title })
                .from(requirement)
                .where(ilike(requirement.title, `%${prefix}%`))
                .limit(limit),
        catch: cause => new DatabaseError({ cause })
    });
}
