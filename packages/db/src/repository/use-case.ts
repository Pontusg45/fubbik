import { and, asc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { requirement } from "../schema/requirement";
import { useCase } from "../schema/use-case";

export interface CreateUseCaseParams {
    id: string;
    name: string;
    description?: string;
    codebaseId?: string;
    userId: string;
    order?: number;
    parentId?: string;
}

export function createUseCase(params: CreateUseCaseParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(useCase).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getUseCaseByName(userId: string, name: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(useCase)
                .where(and(eq(useCase.userId, userId), eq(useCase.name, name)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getUseCaseById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(useCase.id, id)];
            if (userId) conditions.push(eq(useCase.userId, userId));
            const [found] = await db
                .select()
                .from(useCase)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listUseCases(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(useCase.userId, userId)];
            if (codebaseId) conditions.push(eq(useCase.codebaseId, codebaseId));

            const useCases = await db
                .select({
                    id: useCase.id,
                    name: useCase.name,
                    description: useCase.description,
                    codebaseId: useCase.codebaseId,
                    userId: useCase.userId,
                    order: useCase.order,
                    parentId: useCase.parentId,
                    createdAt: useCase.createdAt,
                    updatedAt: useCase.updatedAt,
                    childCount: sql<number>`(SELECT count(*) FROM use_case uc2 WHERE uc2.parent_id = ${useCase.id})`.as("child_count")
                })
                .from(useCase)
                .where(and(...conditions))
                .orderBy(asc(useCase.order), asc(useCase.name));

            // Get requirement counts per use case
            const counts = await db
                .select({
                    useCaseId: requirement.useCaseId,
                    count: sql<number>`count(*)`
                })
                .from(requirement)
                .where(
                    and(
                        eq(requirement.userId, userId),
                        sql`${requirement.useCaseId} IS NOT NULL`
                    )
                )
                .groupBy(requirement.useCaseId);

            const countMap = new Map(counts.map(c => [c.useCaseId, Number(c.count)]));

            return useCases.map(uc => ({
                ...uc,
                childCount: Number(uc.childCount),
                requirementCount: countMap.get(uc.id) ?? 0
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateUseCaseParams {
    name?: string;
    description?: string | null;
    order?: number;
    parentId?: string | null;
}

export function updateUseCase(id: string, userId: string, params: UpdateUseCaseParams) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.name !== undefined) setClause.name = params.name;
            if (params.description !== undefined) setClause.description = params.description;
            if (params.order !== undefined) setClause.order = params.order;
            if (params.parentId !== undefined) setClause.parentId = params.parentId;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(useCase)
                    .where(and(eq(useCase.id, id), eq(useCase.userId, userId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(useCase)
                .set(setClause)
                .where(and(eq(useCase.id, id), eq(useCase.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteUseCase(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            // Unlink requirements first (set useCaseId to null)
            await db
                .update(requirement)
                .set({ useCaseId: null })
                .where(eq(requirement.useCaseId, id));

            const [deleted] = await db
                .delete(useCase)
                .where(and(eq(useCase.id, id), eq(useCase.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listRequirementsByUseCase(useCaseId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            return db
                .select()
                .from(requirement)
                .where(
                    and(
                        eq(requirement.useCaseId, useCaseId),
                        eq(requirement.userId, userId)
                    )
                );
        },
        catch: cause => new DatabaseError({ cause })
    });
}
