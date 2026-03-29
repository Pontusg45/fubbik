import { and, eq, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkFileRef } from "../schema/file-ref";

export function getFileRefsForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkFileRef.id,
                    path: chunkFileRef.path,
                    anchor: chunkFileRef.anchor,
                    relation: chunkFileRef.relation
                })
                .from(chunkFileRef)
                .where(eq(chunkFileRef.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getFileRefsForChunks(chunkIds: string[]) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkFileRef)
                .where(inArray(chunkFileRef.chunkId, chunkIds)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function setFileRefsForChunk(
    chunkId: string,
    refs: { path: string; anchor?: string | null; relation: string }[]
) {
    return Effect.tryPromise({
        try: async () => {
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
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function lookupChunksByFilePath(path: string, userId: string) {
    return Effect.tryPromise({
        try: () =>
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
                .where(and(eq(chunkFileRef.path, path), eq(chunk.userId, userId))),
        catch: cause => new DatabaseError({ cause })
    });
}
