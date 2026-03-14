import { and, eq, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
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
                    tagTypeColor: tagType.color
                })
                .from(tag)
                .leftJoin(tagType, eq(tag.tagTypeId, tagType.id))
                .where(eq(tag.userId, userId)),
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
