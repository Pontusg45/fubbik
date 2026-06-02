import { and, eq, inArray, or, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { space, chunkSpace } from "../schema/space";
import { chunkTag, tag, tagType } from "../schema/tag";
import { workspaceSpace } from "../schema/workspace";

export function getAllChunksMeta(userId?: string, codebaseId?: string, workspaceId?: string) {
    return dbEffect(() => {
            const conditions = [];
            if (userId) conditions.push(eq(chunk.userId, userId));
            if (workspaceId) {
                const inWorkspace = db
                    .select({ spaceId: workspaceSpace.spaceId })
                    .from(workspaceSpace)
                    .where(eq(workspaceSpace.workspaceId, workspaceId));
                const inSpaces = db
                    .select({ chunkId: chunkSpace.chunkId })
                    .from(chunkSpace)
                    .where(inArray(chunkSpace.spaceId, inWorkspace));
                const inAnySpace = db.select({ chunkId: chunkSpace.chunkId }).from(chunkSpace);
                conditions.push(
                    or(sql`${chunk.id} IN (${inSpaces})`, sql`${chunk.id} NOT IN (${inAnySpace})`)!
                );
            } else if (codebaseId) {
                const inSpace = db
                    .select({ chunkId: chunkSpace.chunkId })
                    .from(chunkSpace)
                    .where(eq(chunkSpace.spaceId, codebaseId));
                const inAnySpace = db.select({ chunkId: chunkSpace.chunkId }).from(chunkSpace);
                conditions.push(
                    or(sql`${chunk.id} IN (${inSpace})`, sql`${chunk.id} NOT IN (${inAnySpace})`)!
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
        });
}

export function getAllTagsWithTypes(userId?: string) {
    return dbEffect(() => {
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
        });
}

export function getTagTypesForGraph(userId?: string) {
    return dbEffect(() => {
            const query = db.select().from(tagType);
            if (userId) return query.where(eq(tagType.userId, userId));
            return query;
        });
}

export function getChunkSpaceMappings(userId?: string) {
    return dbEffect(() => {
            const query = db
                .select({
                    chunkId: chunkSpace.chunkId,
                    spaceId: chunkSpace.spaceId,
                    spaceName: space.name
                })
                .from(chunkSpace)
                .innerJoin(space, eq(chunkSpace.spaceId, space.id));
            if (userId) return query.where(eq(space.userId, userId));
            return query;
        });
}

export function getAllConnectionsForUser(userId?: string) {
    return dbEffect(async () => {
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
        });
}
