import { and, eq, ne, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";

export interface SimilarChunk {
    id: string;
    title: string;
    type: string;
    similarity: number;
}

export interface DuplicatePair {
    idA: string;
    idB: string;
    similarity: number;
}

export function findDuplicatePairs(params: {
    chunkIds: string[];
    threshold?: number;
    limit?: number;
}): Effect.Effect<DuplicatePair[], DatabaseError> {
    const { chunkIds, threshold = 0.85, limit = 5 } = params;

    if (chunkIds.length < 2) return Effect.succeed([]);

    return Effect.tryPromise({
        try: async () => {
            const idList = chunkIds.map(id => `'${id.replace(/'/g, "''")}'`).join(",");
            const rows = await db.execute(sql`
                SELECT a.id AS id_a, b.id AS id_b,
                       1 - (a.embedding <=> b.embedding) AS similarity
                FROM chunk a, chunk b
                WHERE a.id IN (${sql.raw(idList)})
                  AND b.id IN (${sql.raw(idList)})
                  AND a.id < b.id
                  AND a.embedding IS NOT NULL AND b.embedding IS NOT NULL
                  AND 1 - (a.embedding <=> b.embedding) > ${threshold}
                ORDER BY similarity DESC
                LIMIT ${limit}
            `);
            return (rows.rows as Array<{ id_a: string; id_b: string; similarity: string | number }>).map(r => ({
                idA: r.id_a,
                idB: r.id_b,
                similarity: Number(r.similarity),
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function findSimilarByEmbedding(params: {
    embedding: number[];
    userId: string;
    excludeId?: string;
    threshold?: number;
    limit?: number;
}): Effect.Effect<SimilarChunk[], DatabaseError> {
    const { embedding, userId, excludeId, threshold = 0.7, limit = 5 } = params;

    return Effect.tryPromise({
        try: async () => {
            const vectorStr = `[${embedding.join(",")}]`;
            const conditions = [
                eq(chunk.userId, userId),
                sql`${chunk.embedding} IS NOT NULL`,
            ];
            if (excludeId) conditions.push(ne(chunk.id, excludeId));

            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    similarity: sql<number>`1 - (${chunk.embedding} <=> ${vectorStr}::vector)`,
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(sql`${chunk.embedding} <=> ${vectorStr}::vector`)
                .limit(limit);

            return results.filter(r => r.similarity >= threshold);
        },
        catch: cause => new DatabaseError({ cause })
    });
}
