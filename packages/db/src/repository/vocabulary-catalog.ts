import { and, asc, eq, or } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkType } from "../schema/chunk-type";
import { connectionRelation } from "../schema/connection-relation";

export interface ListCatalogParams {
    userId?: string;
    codebaseId?: string;
}

/**
 * Returns all builtin chunk types plus any scoped to this user or codebase.
 * Ordered by displayOrder for stable UI rendering.
 */
export function listChunkTypes(params: ListCatalogParams = {}) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(chunkType.builtIn, true)];
            if (params.userId) conditions.push(eq(chunkType.userId, params.userId));
            if (params.codebaseId) conditions.push(eq(chunkType.codebaseId, params.codebaseId));
            return db
                .select()
                .from(chunkType)
                .where(or(...conditions))
                .orderBy(asc(chunkType.displayOrder), asc(chunkType.id));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listConnectionRelations(params: ListCatalogParams = {}) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(connectionRelation.builtIn, true)];
            if (params.userId) conditions.push(eq(connectionRelation.userId, params.userId));
            if (params.codebaseId) conditions.push(eq(connectionRelation.codebaseId, params.codebaseId));
            return db
                .select()
                .from(connectionRelation)
                .where(or(...conditions))
                .orderBy(asc(connectionRelation.displayOrder), asc(connectionRelation.id));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

// --- writes ------------------------------------------------------------

export interface ChunkTypeInsert {
    id: string;
    label: string;
    description?: string | null;
    icon?: string | null;
    color?: string;
    examples?: string[];
    displayOrder?: number;
    userId: string;
    codebaseId?: string | null;
}

export function createChunkType(row: ChunkTypeInsert) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(chunkType)
                .values({
                    id: row.id,
                    label: row.label,
                    description: row.description ?? null,
                    icon: row.icon ?? null,
                    color: row.color ?? "#8b5cf6",
                    examples: row.examples ?? [],
                    displayOrder: row.displayOrder ?? 500,
                    builtIn: false,
                    userId: row.userId,
                    codebaseId: row.codebaseId ?? null
                })
                .returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function findChunkTypeById(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(chunkType).where(eq(chunkType.id, id));
            return row ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateChunkTypeRow(id: string, userId: string, data: Partial<Omit<ChunkTypeInsert, "id" | "userId">>) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunkType)
                .set({
                    ...(data.label !== undefined && { label: data.label }),
                    ...(data.description !== undefined && { description: data.description }),
                    ...(data.icon !== undefined && { icon: data.icon }),
                    ...(data.color !== undefined && { color: data.color }),
                    ...(data.examples !== undefined && { examples: data.examples }),
                    ...(data.displayOrder !== undefined && { displayOrder: data.displayOrder })
                })
                .where(and(eq(chunkType.id, id), eq(chunkType.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteChunkTypeRow(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(chunkType)
                .where(and(eq(chunkType.id, id), eq(chunkType.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface ConnectionRelationInsert {
    id: string;
    label: string;
    description?: string | null;
    arrowStyle?: "solid" | "dashed" | "dotted";
    direction?: "forward" | "bidirectional";
    color?: string;
    inverseOfId?: string | null;
    displayOrder?: number;
    userId: string;
    codebaseId?: string | null;
}

export function createConnectionRelation(row: ConnectionRelationInsert) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(connectionRelation)
                .values({
                    id: row.id,
                    label: row.label,
                    description: row.description ?? null,
                    arrowStyle: row.arrowStyle ?? "solid",
                    direction: row.direction ?? "forward",
                    color: row.color ?? "#64748b",
                    inverseOfId: row.inverseOfId ?? null,
                    displayOrder: row.displayOrder ?? 500,
                    builtIn: false,
                    userId: row.userId,
                    codebaseId: row.codebaseId ?? null
                })
                .returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function findConnectionRelationById(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(connectionRelation).where(eq(connectionRelation.id, id));
            return row ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateConnectionRelationRow(
    id: string,
    userId: string,
    data: Partial<Omit<ConnectionRelationInsert, "id" | "userId">>
) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(connectionRelation)
                .set({
                    ...(data.label !== undefined && { label: data.label }),
                    ...(data.description !== undefined && { description: data.description }),
                    ...(data.arrowStyle !== undefined && { arrowStyle: data.arrowStyle }),
                    ...(data.direction !== undefined && { direction: data.direction }),
                    ...(data.color !== undefined && { color: data.color }),
                    ...(data.inverseOfId !== undefined && { inverseOfId: data.inverseOfId }),
                    ...(data.displayOrder !== undefined && { displayOrder: data.displayOrder })
                })
                .where(and(eq(connectionRelation.id, id), eq(connectionRelation.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteConnectionRelationRow(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(connectionRelation)
                .where(and(eq(connectionRelation.id, id), eq(connectionRelation.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

