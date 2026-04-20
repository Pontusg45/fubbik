import { countDistinct, eq, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { tag } from "../schema/tag";

export function getChunkCount(userId?: string) {
    return dbEffect(async () => {
            const query = userId
                ? db
                      .select({ count: sql<number>`count(*)` })
                      .from(chunk)
                      .where(eq(chunk.userId, userId))
                : db.select({ count: sql<number>`count(*)` }).from(chunk);
            const [result] = await query;
            return Number(result?.count ?? 0);
        });
}

export function getConnectionCount(userId?: string) {
    return dbEffect(async () => {
            const query = userId
                ? db
                      .select({ count: sql<number>`count(*)` })
                      .from(chunkConnection)
                      .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
                      .where(eq(chunk.userId, userId))
                : db.select({ count: sql<number>`count(*)` }).from(chunkConnection);
            const [result] = await query;
            return Number(result?.count ?? 0);
        });
}

export function getTagCount(userId?: string) {
    return dbEffect(async () => {
            const query = db.select({ count: countDistinct(tag.id) }).from(tag);
            if (userId) query.where(eq(tag.userId, userId));
            const [result] = await query;
            return Number(result?.count ?? 0);
        });
}
