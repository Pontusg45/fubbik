import { and, eq, inArray, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkFileRef } from "../schema/file-ref";
import { chunkSpace } from "../schema/space";

export function getFileRefsForChunk(chunkId: string) {
    return dbEffect(() =>
        db
            .select({
                id: chunkFileRef.id,
                path: chunkFileRef.path,
                anchor: chunkFileRef.anchor,
                relation: chunkFileRef.relation
            })
            .from(chunkFileRef)
            .where(eq(chunkFileRef.chunkId, chunkId))
    );
}

export function getFileRefsForChunks(chunkIds: string[]) {
    return dbEffect(() => db.select().from(chunkFileRef).where(inArray(chunkFileRef.chunkId, chunkIds)));
}

export function setFileRefsForChunk(chunkId: string, refs: { path: string; anchor?: string | null; relation: string }[]) {
    return dbEffect(async () => {
        await db.delete(chunkFileRef).where(eq(chunkFileRef.chunkId, chunkId));
        if (refs.length === 0) return [];
        return db
            .insert(chunkFileRef)
            .values(
                refs.map(r => ({
                    id: crypto.randomUUID(),
                    chunkId,
                    path: r.path,
                    anchor: r.anchor ?? null,
                    relation: r.relation
                }))
            )
            .returning();
    });
}

export function lookupChunksByFilePath(path: string, userId: string, spaceId?: string) {
    return dbEffect(() => {
        const conditions = [eq(chunkFileRef.path, path), eq(chunk.userId, userId)];

        if (spaceId) {
            const inSpace = db.select({ chunkId: chunkSpace.chunkId }).from(chunkSpace).where(eq(chunkSpace.spaceId, spaceId));
            const inAnySpace = db.select({ chunkId: chunkSpace.chunkId }).from(chunkSpace);
            conditions.push(sql`(${chunk.id} IN (${inSpace}) OR ${chunk.id} NOT IN (${inAnySpace}))`);
        }

        return db
            .select({
                chunkId: chunk.id,
                chunkTitle: chunk.title,
                chunkType: chunk.type,
                refId: chunkFileRef.id,
                path: chunkFileRef.path,
                anchor: chunkFileRef.anchor,
                relation: chunkFileRef.relation
            })
            .from(chunkFileRef)
            .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
            .where(and(...conditions));
    });
}

export function listAllFileRefs(userId: string) {
    return dbEffect(() =>
        db
            .select({
                chunkId: chunk.id,
                chunkTitle: chunk.title,
                path: chunkFileRef.path,
                anchor: chunkFileRef.anchor
            })
            .from(chunkFileRef)
            .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
            .where(eq(chunk.userId, userId))
    );
}
