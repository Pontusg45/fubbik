import { and, eq, inArray } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkFileRef } from "../schema/file-ref";

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
                .where(eq(chunkFileRef.chunkId, chunkId)));
}

export function getFileRefsForChunks(chunkIds: string[]) {
    return dbEffect(() =>
            db
                .select()
                .from(chunkFileRef)
                .where(inArray(chunkFileRef.chunkId, chunkIds)));
}

export function setFileRefsForChunk(
    chunkId: string,
    refs: { path: string; anchor?: string | null; relation: string }[]
) {
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

export function lookupChunksByFilePath(path: string, userId: string) {
    return dbEffect(() =>
            db
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
                .where(and(eq(chunkFileRef.path, path), eq(chunk.userId, userId))));
}
