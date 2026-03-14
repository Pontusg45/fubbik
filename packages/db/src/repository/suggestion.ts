import { and, eq, ne, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkTag } from "../schema/tag";

interface SuggestionRow {
    id: string;
    title: string;
    type: string;
    reason: string;
}

export function findChunksSharingTags(
    chunkId: string,
    userId: string,
    tagIds: string[],
    excludeIds: Set<string>
): Effect.Effect<SuggestionRow[], DatabaseError> {
    if (tagIds.length === 0) return Effect.succeed([]);

    return Effect.tryPromise({
        try: async () => {
            const excludeArray = [...excludeIds];
            const tagPlaceholders = tagIds.map(id => sql`${id}`);
            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    sharedCount: sql<number>`count(*)`.as("shared_count")
                })
                .from(chunkTag)
                .innerJoin(chunk, eq(chunkTag.chunkId, chunk.id))
                .where(
                    and(
                        sql`${chunkTag.tagId} IN (${sql.join(tagPlaceholders, sql`, `)})`,
                        ne(chunkTag.chunkId, chunkId),
                        eq(chunk.userId, userId),
                        ...(excludeArray.length > 0
                            ? [sql`${chunk.id} NOT IN (${sql.join(excludeArray.map(id => sql`${id}`), sql`, `)})`]
                            : [])
                    )
                )
                .groupBy(chunk.id, chunk.title, chunk.type)
                .orderBy(sql`count(*) DESC`)
                .limit(5);

            return results.map(r => ({
                id: r.id,
                title: r.title,
                type: r.type,
                reason: `shares ${r.sharedCount} tag${Number(r.sharedCount) > 1 ? "s" : ""}`
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function findChunksWithSimilarTitle(
    title: string,
    userId: string,
    excludeIds: Set<string>
): Effect.Effect<SuggestionRow[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const excludeArray = [...excludeIds];
            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    sim: sql<number>`similarity(${chunk.title}, ${title})`
                })
                .from(chunk)
                .where(
                    and(
                        eq(chunk.userId, userId),
                        sql`similarity(${chunk.title}, ${title}) > 0.15`,
                        ...(excludeArray.length > 0
                            ? [sql`${chunk.id} NOT IN (${sql.join(excludeArray.map(id => sql`${id}`), sql`, `)})`]
                            : [])
                    )
                )
                .orderBy(sql`similarity(${chunk.title}, ${title}) DESC`)
                .limit(5);

            return results.map(r => ({
                id: r.id,
                title: r.title,
                type: r.type,
                reason: "similar title"
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}
