import { and, eq, inArray, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { codebase, chunkCodebase } from "../schema/codebase";

export interface CreateCodebaseParams {
    id: string;
    name: string;
    remoteUrl?: string;
    localPaths?: string[];
    userId: string;
}

export function createCodebase(params: CreateCodebaseParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(codebase)
                .values({
                    id: params.id,
                    name: params.name,
                    remoteUrl: params.remoteUrl,
                    localPaths: params.localPaths ?? [],
                    userId: params.userId
                })
                .returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebaseById(codebaseId: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(codebase.id, codebaseId)];
            if (userId) conditions.push(eq(codebase.userId, userId));
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listCodebases(userId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(codebase).where(eq(codebase.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebaseByRemoteUrl(remoteUrl: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(eq(codebase.remoteUrl, remoteUrl), eq(codebase.userId, userId)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebaseByLocalPath(localPath: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(sql`${codebase.localPaths} @> ${JSON.stringify([localPath])}::jsonb`, eq(codebase.userId, userId)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateCodebaseParams {
    name?: string;
    remoteUrl?: string;
    localPaths?: string[];
}

export function updateCodebase(codebaseId: string, userId: string, params: UpdateCodebaseParams) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(codebase)
                .set({
                    ...(params.name !== undefined && { name: params.name }),
                    ...(params.remoteUrl !== undefined && { remoteUrl: params.remoteUrl }),
                    ...(params.localPaths !== undefined && { localPaths: params.localPaths })
                })
                .where(and(eq(codebase.id, codebaseId), eq(codebase.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteCodebase(codebaseId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(codebase)
                .where(and(eq(codebase.id, codebaseId), eq(codebase.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function countChunksInCodebase(codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkCodebase)
                .where(eq(chunkCodebase.codebaseId, codebaseId));
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function setChunkCodebases(chunkId: string, codebaseIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkCodebase).where(eq(chunkCodebase.chunkId, chunkId));
            if (codebaseIds.length === 0) return [];
            return db
                .insert(chunkCodebase)
                .values(codebaseIds.map(codebaseId => ({ chunkId, codebaseId })))
                .onConflictDoNothing()
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebasesForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: codebase.id,
                    name: codebase.name,
                    remoteUrl: codebase.remoteUrl,
                    localPaths: codebase.localPaths
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id))
                .where(eq(chunkCodebase.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebasesForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([]);
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    chunkId: chunkCodebase.chunkId,
                    codebaseId: codebase.id,
                    codebaseName: codebase.name
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id))
                .where(inArray(chunkCodebase.chunkId, chunkIds)),
        catch: cause => new DatabaseError({ cause })
    });
}
