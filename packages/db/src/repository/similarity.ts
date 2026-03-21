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
