import { and, eq, inArray, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { codebase, chunkCodebase } from "../schema/codebase";
import { chunkTag, tag, tagType } from "../schema/tag";
import { workspaceCodebase } from "../schema/workspace";

export function getAllChunksMeta(userId?: string, codebaseId?: string, workspaceId?: string) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [];
            if (userId) conditions.push(eq(chunk.userId, userId));
            if (workspaceId) {
                const inWorkspace = db
                    .select({ codebaseId: workspaceCodebase.codebaseId })
                    .from(workspaceCodebase)
                    .where(eq(workspaceCodebase.workspaceId, workspaceId));
                const inCodebases = db
                    .select({ chunkId: chunkCodebase.chunkId })
                    .from(chunkCodebase)
                    .where(inArray(chunkCodebase.codebaseId, inWorkspace));
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(
                    or(sql`${chunk.id} IN (${inCodebases})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!
                );
            } else if (codebaseId) {
                const inCodebase = db
                    .select({ chunkId: chunkCodebase.chunkId })
                    .from(chunkCodebase)
                    .where(eq(chunkCodebase.codebaseId, codebaseId));
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(
                    or(sql`${chunk.id} IN (${inCodebase})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!
                );
            }
            const query = db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    summary: chunk.summary,
                    createdAt: chunk.createdAt
                })
                .from(chunk);
            return conditions.length > 0 ? query.where(and(...conditions)) : query;
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

export function getChunkCodebaseMappings(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db
                .select({
                    chunkId: chunkCodebase.chunkId,
                    codebaseId: chunkCodebase.codebaseId,
                    codebaseName: codebase.name
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id));
            if (userId) return query.where(eq(codebase.userId, userId));
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
