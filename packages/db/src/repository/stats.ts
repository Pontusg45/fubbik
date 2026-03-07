import { eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export function getChunkCount(userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const query = userId
                ? db
                      .select({ count: sql<number>`count(*)` })
                      .from(chunk)
                      .where(eq(chunk.userId, userId))
                : db.select({ count: sql<number>`count(*)` }).from(chunk);
            const [result] = await query;
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getConnectionCount(userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const query = userId
                ? db
                      .select({ count: sql<number>`count(*)` })
                      .from(chunkConnection)
                      .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
                      .where(eq(chunk.userId, userId))
                : db.select({ count: sql<number>`count(*)` }).from(chunkConnection);
            const [result] = await query;
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagCount(userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const query = userId
                ? sql`(select jsonb_array_elements_text(${chunk.tags}) as tag from ${chunk} where ${chunk.userId} = ${userId}) t`
                : sql`(select jsonb_array_elements_text(${chunk.tags}) as tag from ${chunk}) t`;
            const [result] = await db.select({ count: sql<number>`count(distinct tag)` }).from(query);
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}
