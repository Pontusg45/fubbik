import { eq, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { chunkTag, tag, tagType } from "../schema/tag";

export function getAllChunksMeta(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    summary: chunk.summary,
                    createdAt: chunk.createdAt
                })
                .from(chunk);
            return userId ? query.where(eq(chunk.userId, userId)) : query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getAllTagsWithTypes(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db
                .select({
                    chunkId: chunkTag.chunkId,
                    tagId: tag.id,
                    tagName: tag.name,
                    tagTypeId: tag.tagTypeId,
                    tagTypeName: tagType.name,
                    tagTypeColor: tagType.color
                })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .leftJoin(tagType, eq(tag.tagTypeId, tagType.id));

            if (userId) {
                return query.where(eq(tag.userId, userId));
            }
            return query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagTypesForGraph(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db.select().from(tagType);
            if (userId) return query.where(eq(tagType.userId, userId));
            return query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getAllConnectionsForUser(userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            if (!userId) {
                return db
                    .select({
                        id: chunkConnection.id,
                        sourceId: chunkConnection.sourceId,
                        targetId: chunkConnection.targetId,
                        relation: chunkConnection.relation
                    })
                    .from(chunkConnection);
            }
            const userChunkIds = db.select({ id: chunk.id }).from(chunk).where(eq(chunk.userId, userId));
            return db
                .select({
                    id: chunkConnection.id,
                    sourceId: chunkConnection.sourceId,
                    targetId: chunkConnection.targetId,
                    relation: chunkConnection.relation
                })
                .from(chunkConnection)
                .where(or(sql`${chunkConnection.sourceId} IN (${userChunkIds})`, sql`${chunkConnection.targetId} IN (${userChunkIds})`));
        },
        catch: cause => new DatabaseError({ cause })
    });
}
