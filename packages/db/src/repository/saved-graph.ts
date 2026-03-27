import { and, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { savedGraph } from "../schema/saved-graph";

// ── Types ────────────────────────────────────────────────────────

export interface CreateSavedGraphParams {
    id: string;
    name: string;
    description?: string;
    chunkIds: string[];
    positions: Record<string, { x: number; y: number }>;
    layoutAlgorithm: string;
    userId: string;
    codebaseId?: string | null;
}

export interface UpdateSavedGraphParams {
    name?: string;
    description?: string | null;
    chunkIds?: string[];
    positions?: Record<string, { x: number; y: number }>;
    layoutAlgorithm?: string;
}

// ── CRUD ─────────────────────────────────────────────────────────

export function createSavedGraph(params: CreateSavedGraphParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(savedGraph).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getSavedGraphById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(savedGraph.id, id)];
            if (userId) conditions.push(eq(savedGraph.userId, userId));
            const [found] = await db
                .select()
                .from(savedGraph)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listSavedGraphs(userId: string, codebaseId?: string | null) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(savedGraph.userId, userId)];
            if (codebaseId) conditions.push(eq(savedGraph.codebaseId, codebaseId));
            return db
                .select()
                .from(savedGraph)
                .where(and(...conditions));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateSavedGraph(id: string, userId: string, params: UpdateSavedGraphParams) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.name !== undefined) setClause.name = params.name;
            if (params.description !== undefined) setClause.description = params.description;
            if (params.chunkIds !== undefined) setClause.chunkIds = params.chunkIds;
            if (params.positions !== undefined) setClause.positions = params.positions;
            if (params.layoutAlgorithm !== undefined) setClause.layoutAlgorithm = params.layoutAlgorithm;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(savedGraph)
                    .where(and(eq(savedGraph.id, id), eq(savedGraph.userId, userId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(savedGraph)
                .set(setClause)
                .where(and(eq(savedGraph.id, id), eq(savedGraph.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteSavedGraph(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(savedGraph)
                .where(and(eq(savedGraph.id, id), eq(savedGraph.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
