import { and, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";

export interface SemanticSearchParams {
    embedding: number[];
    userId?: string;
    exclude?: string[];
    scope?: Record<string, string>;
    limit: number;
}

export interface NeighborRow {
    id: string;
    title: string;
    summary: string | null;
    type: string;
    distance: number;
}

export function findNeighborsByChunkId(chunkId: string, userId: string, k: number) {
    return Effect.tryPromise({
        try: async (): Promise<NeighborRow[]> => {
            const result = await db.execute(sql`
                WITH source AS (
                    SELECT embedding
                    FROM chunk
                    WHERE id = ${chunkId}
                      AND user_id = ${userId}
                      AND embedding IS NOT NULL
                )
                SELECT
                    c.id AS id,
                    c.title AS title,
                    c.summary AS summary,
                    c.type AS type,
                    (c.embedding <=> (SELECT embedding FROM source))::float8 AS distance
                FROM chunk c, source
                WHERE c.id <> ${chunkId}
                  AND c.user_id = ${userId}
                  AND c.embedding IS NOT NULL
                  AND c.archived_at IS NULL
                ORDER BY c.embedding <=> (SELECT embedding FROM source)
                LIMIT ${k}
            `);
            const rows = result.rows as Array<{
                id: string;
                title: string;
                summary: string | null;
                type: string;
                distance: string | number;
            }>;
            return rows.map(r => ({
                id: r.id,
                title: r.title,
                summary: r.summary,
                type: r.type,
                distance: Number(r.distance)
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function semanticSearch(params: SemanticSearchParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [sql`${chunk.embedding} IS NOT NULL`];
            if (params.userId) conditions.push(eq(chunk.userId, params.userId));
            if (params.exclude?.length) {
                for (const term of params.exclude) {
                    conditions.push(sql`NOT (${chunk.notAbout} @> ${JSON.stringify([term])}::jsonb)`);
                }
            }
            if (params.scope && Object.keys(params.scope).length > 0) {
                conditions.push(sql`${chunk.scope} @> ${JSON.stringify(params.scope)}::jsonb`);
            }

            const embeddingStr = `[${params.embedding.join(",")}]`;
            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    summary: chunk.summary,
                    type: chunk.type,
                    aliases: chunk.aliases,
                    scope: chunk.scope,
                    similarity: sql<number>`1 - (${chunk.embedding} <=> ${embeddingStr}::vector)`
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(sql`${chunk.embedding} <=> ${embeddingStr}::vector`)
                .limit(params.limit);

            return results;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
