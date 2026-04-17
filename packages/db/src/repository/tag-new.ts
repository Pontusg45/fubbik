import { and, eq, inArray, isNull, ne, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { tag, chunkTag, tagType } from "../schema/tag";

export function createTag(params: { id: string; name: string; tagTypeId?: string; userId: string; origin?: string; reviewStatus?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(tag).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagsForUser(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: tag.id,
                    name: tag.name,
                    tagTypeId: tag.tagTypeId,
                    tagTypeName: tagType.name,
                    tagTypeColor: tagType.color,
                    tagTypeIcon: tagType.icon,
                    chunkCount: sql<number>`count(${chunkTag.chunkId})::int`.as("chunk_count")
                })
                .from(tag)
                .leftJoin(tagType, eq(tag.tagTypeId, tagType.id))
                .leftJoin(chunkTag, eq(chunkTag.tagId, tag.id))
                .where(eq(tag.userId, userId))
                .groupBy(tag.id, tagType.id),
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateTag(id: string, userId: string, data: { name?: string; tagTypeId?: string | null; origin?: string; reviewStatus?: string; reviewedBy?: string | null; reviewedAt?: Date | null }) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(tag)
                .set(data)
                .where(and(eq(tag.id, id), eq(tag.userId, userId)))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function tagNameConflict(id: string, userId: string, name: string) {
    return Effect.tryPromise({
        try: async () => {
            const [hit] = await db
                .select({ id: tag.id })
                .from(tag)
                .where(and(eq(tag.userId, userId), eq(tag.name, name), ne(tag.id, id)))
                .limit(1);
            return !!hit;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function mergeTags(sourceId: string, targetId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            return await db.transaction(async tx => {
                // Ensure both belong to the same user (authorisation)
                const rows = await tx
                    .select({ id: tag.id })
                    .from(tag)
                    .where(and(inArray(tag.id, [sourceId, targetId]), eq(tag.userId, userId)));
                if (rows.length !== 2) {
                    throw new Error("source or target tag not found for user");
                }

                // Re-point chunk_tag rows from source → target, skipping pairs
                // where the target link already exists.
                await tx.execute(sql`
                    INSERT INTO chunk_tag (chunk_id, tag_id)
                    SELECT chunk_id, ${targetId} FROM chunk_tag
                    WHERE tag_id = ${sourceId}
                    ON CONFLICT (chunk_id, tag_id) DO NOTHING
                `);
                await tx.delete(chunkTag).where(eq(chunkTag.tagId, sourceId));

                // Drop the source tag.
                await tx.delete(tag).where(and(eq(tag.id, sourceId), eq(tag.userId, userId)));

                // Return the new total chunk count on the target for UX.
                const [count] = await tx
                    .select({ count: sql<number>`count(*)::int` })
                    .from(chunkTag)
                    .where(eq(chunkTag.tagId, targetId));
                return { targetId, chunkCount: count?.count ?? 0 };
            });
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteTag(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(tag)
                .where(and(eq(tag.id, id), eq(tag.userId, userId)))
                .returning();
            return deleted;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function setChunkTags(chunkId: string, tagIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkTag).where(eq(chunkTag.chunkId, chunkId));
            if (tagIds.length === 0) return [];
            return db
                .insert(chunkTag)
                .values(tagIds.map(tagId => ({ chunkId, tagId })))
                .onConflictDoNothing()
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagsForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({ id: tag.id, name: tag.name, tagTypeId: tag.tagTypeId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(eq(chunkTag.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagsForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([]);
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    chunkId: chunkTag.chunkId,
                    tagId: tag.id,
                    tagName: tag.name,
                    tagTypeId: tag.tagTypeId
                })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(inArray(chunkTag.chunkId, chunkIds)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function findOrCreateTag(name: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const existing = await db
                .select()
                .from(tag)
                .where(and(eq(tag.name, name), eq(tag.userId, userId)))
                .limit(1);
            if (existing.length > 0) return existing[0]!;
            const id = crypto.randomUUID();
            const [created] = await db.insert(tag).values({ id, name, userId }).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface ListChunksByTagParams {
    userId: string;
    tagName: string;
    codebaseId?: string;
}

export function listChunksByTag(params: ListChunksByTagParams) {
    return Effect.tryPromise({
        try: async () => {
            // Find the tag by name for this user
            const [matchingTag] = await db
                .select({ id: tag.id })
                .from(tag)
                .where(and(eq(tag.name, params.tagName), eq(tag.userId, params.userId)));

            if (!matchingTag) return [];

            // Get chunk IDs that have this tag
            const taggedChunkIds = await db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .where(eq(chunkTag.tagId, matchingTag.id));

            if (taggedChunkIds.length === 0) return [];

            const ids = taggedChunkIds.map(r => r.chunkId);

            const conditions = [
                inArray(chunk.id, ids),
                eq(chunk.userId, params.userId),
                isNull(chunk.archivedAt)
            ];

            if (params.codebaseId) {
                const inCodebase = db
                    .select({ chunkId: chunkCodebase.chunkId })
                    .from(chunkCodebase)
                    .where(eq(chunkCodebase.codebaseId, params.codebaseId));
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(
                    sql`(${chunk.id} IN (${inCodebase}) OR ${chunk.id} NOT IN (${inAnyCodebase}))`
                );
            }

            return db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    content: chunk.content,
                    type: chunk.type,
                    rationale: chunk.rationale,
                    summary: chunk.summary
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(chunk.title);
        },
        catch: cause => new DatabaseError({ cause })
    });
}
