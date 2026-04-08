import { and, eq, isNotNull, sql } from "drizzle-orm";
import { Effect } from "effect";

import { getOrphanChunkIds } from "../age/query";
import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { chunkFileRef } from "../schema/file-ref";

function codebaseConditions(codebaseId?: string) {
    if (!codebaseId) return [];
    const inCodebase = db
        .select({ chunkId: chunkCodebase.chunkId })
        .from(chunkCodebase)
        .where(eq(chunkCodebase.codebaseId, codebaseId));
    const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
    return [sql`(${chunk.id} IN (${inCodebase}) OR ${chunk.id} NOT IN (${inAnyCodebase}))`];
}

export function getOrphanChunks(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                sql`${chunk.id} NOT IN (SELECT ${chunkConnection.sourceId} FROM ${chunkConnection})`,
                sql`${chunk.id} NOT IN (SELECT ${chunkConnection.targetId} FROM ${chunkConnection})`,
                ...codebaseConditions(codebaseId)
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    createdAt: chunk.createdAt
                })
                .from(chunk)
                .where(and(...conditions))
                .limit(50);

            const countResult = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, count: Number(countResult[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getStaleChunks(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const thirtyDaysAgo = sql`NOW() - INTERVAL '30 days'`;
            const sevenDaysAgo = sql`NOW() - INTERVAL '7 days'`;

            // NOTE: Must use "chunk"."id" (fully qualified) to avoid ambiguity
            // with the "neighbor" alias which is also the chunk table.
            const neighborExists = sql`EXISTS (
                SELECT 1 FROM "chunk_connection" cc
                INNER JOIN "chunk" neighbor
                    ON neighbor.id = CASE
                        WHEN cc.source_id = "chunk"."id" THEN cc.target_id
                        WHEN cc.target_id = "chunk"."id" THEN cc.source_id
                    END
                WHERE (cc.source_id = "chunk"."id" OR cc.target_id = "chunk"."id")
                    AND neighbor.updated_at > ${sevenDaysAgo}
            )`;

            const newestNeighborUpdate = sql<Date>`(
                SELECT MAX(neighbor.updated_at) FROM "chunk_connection" cc
                INNER JOIN "chunk" neighbor
                    ON neighbor.id = CASE
                        WHEN cc.source_id = "chunk"."id" THEN cc.target_id
                        WHEN cc.target_id = "chunk"."id" THEN cc.source_id
                    END
                WHERE (cc.source_id = "chunk"."id" OR cc.target_id = "chunk"."id")
            )`;

            const conditions = [
                eq(chunk.userId, userId),
                sql`${chunk.updatedAt} < ${thirtyDaysAgo}`,
                neighborExists,
                ...codebaseConditions(codebaseId)
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    updatedAt: chunk.updatedAt,
                    newestNeighborUpdate
                })
                .from(chunk)
                .where(and(...conditions))
                .limit(50);

            const countResult = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, count: Number(countResult[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getThinChunks(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                sql`LENGTH(${chunk.content}) < 100`,
                ...codebaseConditions(codebaseId)
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    contentLength: sql<number>`LENGTH(${chunk.content})`
                })
                .from(chunk)
                .where(and(...conditions))
                .limit(50);

            const countResult = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, count: Number(countResult[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getStaleEmbeddings(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                isNotNull(chunk.embedding),
                sql`${chunk.updatedAt} > ${chunk.embeddingUpdatedAt}`,
                ...codebaseConditions(codebaseId)
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    updatedAt: chunk.updatedAt,
                    embeddingUpdatedAt: chunk.embeddingUpdatedAt
                })
                .from(chunk)
                .where(and(...conditions))
                .limit(50);

            const countResult = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, count: Number(countResult[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getOrphanChunkIdsViaAge() {
    return getOrphanChunkIds();
}

export function getFileRefsForHealth(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.userId, userId), ...codebaseConditions(codebaseId)];

            const refs = await db
                .select({
                    refId: chunkFileRef.id,
                    chunkId: chunkFileRef.chunkId,
                    chunkTitle: chunk.title,
                    chunkType: chunk.type,
                    path: chunkFileRef.path,
                    relation: chunkFileRef.relation
                })
                .from(chunkFileRef)
                .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
                .where(and(...conditions))
                .limit(100);

            const countResult = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkFileRef)
                .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
                .where(and(...conditions));

            return { refs, count: Number(countResult[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}
