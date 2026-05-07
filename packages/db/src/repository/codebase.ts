import { and, eq, inArray, sql } from "drizzle-orm";
import { Effect } from "effect";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { codebase, chunkCodebase } from "../schema/codebase";
import { document } from "../schema/document";
import { plan } from "../schema/plan";
import { requirement } from "../schema/requirement";

export interface CreateCodebaseParams {
    id: string;
    name: string;
    remoteUrl?: string;
    localPaths?: string[];
    userId: string;
}

export function createCodebase(params: CreateCodebaseParams) {
    return dbEffect(async () => {
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
        });
}

export function getCodebaseById(codebaseId: string, userId?: string) {
    return dbEffect(async () => {
            const conditions = [eq(codebase.id, codebaseId)];
            if (userId) conditions.push(eq(codebase.userId, userId));
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(...conditions));
            return found ?? null;
        });
}

export function listCodebases(userId: string) {
    return dbEffect(() => db.select().from(codebase).where(eq(codebase.userId, userId)));
}

export function getCodebaseByRemoteUrl(remoteUrl: string, userId: string) {
    return dbEffect(async () => {
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(eq(codebase.remoteUrl, remoteUrl), eq(codebase.userId, userId)));
            return found ?? null;
        });
}

export function getCodebaseByLocalPath(localPath: string, userId: string) {
    return dbEffect(async () => {
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(sql`${codebase.localPaths} @> ${JSON.stringify([localPath])}::jsonb`, eq(codebase.userId, userId)));
            return found ?? null;
        });
}

export interface UpdateCodebaseParams {
    name?: string;
    remoteUrl?: string | null;
    localPaths?: string[];
}

export function updateCodebase(codebaseId: string, userId: string, params: UpdateCodebaseParams) {
    return dbEffect(async () => {
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
        });
}

export function resetCodebaseData(codebaseId: string, userId: string) {
    return dbEffect(async () => {
        // Find chunks exclusively in this codebase (not shared with others)
        const exclusiveChunkIds = await db
            .select({ id: chunkCodebase.chunkId })
            .from(chunkCodebase)
            .where(eq(chunkCodebase.codebaseId, codebaseId))
            .then(rows => rows.map(r => r.id));

        let chunksDeleted = 0;
        if (exclusiveChunkIds.length > 0) {
            const sharedChunkIds = await db
                .select({ id: chunkCodebase.chunkId })
                .from(chunkCodebase)
                .where(and(
                    inArray(chunkCodebase.chunkId, exclusiveChunkIds),
                    sql`${chunkCodebase.codebaseId} != ${codebaseId}`
                ))
                .then(rows => new Set(rows.map(r => r.id)));

            const toDelete = exclusiveChunkIds.filter(id => !sharedChunkIds.has(id));

            if (toDelete.length > 0) {
                await db.delete(chunk).where(and(
                    inArray(chunk.id, toDelete),
                    eq(chunk.userId, userId)
                ));
                chunksDeleted = toDelete.length;
            }
        }

        // Unlink remaining shared chunks from this codebase
        await db.delete(chunkCodebase).where(eq(chunkCodebase.codebaseId, codebaseId));

        // Delete documents scoped to this codebase
        const docsDeleted = await db
            .delete(document)
            .where(and(eq(document.codebaseId, codebaseId), eq(document.userId, userId)))
            .returning()
            .then(rows => rows.length);

        // Delete plans scoped to this codebase
        const plansDeleted = await db
            .delete(plan)
            .where(and(eq(plan.codebaseId, codebaseId), eq(plan.userId, userId)))
            .returning()
            .then(rows => rows.length);

        // Delete requirements scoped to this codebase
        const requirementsDeleted = await db
            .delete(requirement)
            .where(and(eq(requirement.codebaseId, codebaseId), eq(requirement.userId, userId)))
            .returning()
            .then(rows => rows.length);

        return { chunksDeleted, docsDeleted, plansDeleted, requirementsDeleted };
    });
}

export function deleteCodebase(codebaseId: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(codebase)
                .where(and(eq(codebase.id, codebaseId), eq(codebase.userId, userId)))
                .returning();
            return deleted ?? null;
        });
}

export function countChunksInCodebase(codebaseId: string) {
    return dbEffect(async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkCodebase)
                .where(eq(chunkCodebase.codebaseId, codebaseId));
            return Number(result?.count ?? 0);
        });
}

export function setChunkCodebases(chunkId: string, codebaseIds: string[]) {
    return dbEffect(async () => {
            await db.delete(chunkCodebase).where(eq(chunkCodebase.chunkId, chunkId));
            if (codebaseIds.length === 0) return [];
            return db
                .insert(chunkCodebase)
                .values(codebaseIds.map(codebaseId => ({ chunkId, codebaseId })))
                .onConflictDoNothing()
                .returning();
        });
}

export function getCodebasesForChunk(chunkId: string) {
    return dbEffect(() =>
            db
                .select({
                    id: codebase.id,
                    name: codebase.name,
                    remoteUrl: codebase.remoteUrl,
                    localPaths: codebase.localPaths
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id))
                .where(eq(chunkCodebase.chunkId, chunkId)));
}

export function getCodebasesForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([]);
    return dbEffect(() =>
            db
                .select({
                    chunkId: chunkCodebase.chunkId,
                    codebaseId: codebase.id,
                    codebaseName: codebase.name
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id))
                .where(inArray(chunkCodebase.chunkId, chunkIds)));
}
