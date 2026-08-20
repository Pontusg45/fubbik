import { and, asc, eq, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkComment } from "../schema/comment";

/**
 * SECURITY: scoped through the commented chunk's owner. `chunk_comment` has
 * a `user_id` (the comment's author) but that is not the authority here —
 * a thread on your chunk may contain other people's comments, and the right
 * to read the thread comes from owning the chunk. Without this join,
 * GET /chunks/:id/comments returned any chunk's discussion to any caller.
 */
export function listComments(chunkId: string, userId: string) {
    return dbEffect(() =>
        db
            .select({
                id: chunkComment.id,
                chunkId: chunkComment.chunkId,
                userId: chunkComment.userId,
                content: chunkComment.content,
                createdAt: chunkComment.createdAt,
                updatedAt: chunkComment.updatedAt
            })
            .from(chunkComment)
            .innerJoin(chunk, eq(chunk.id, chunkComment.chunkId))
            .where(and(eq(chunkComment.chunkId, chunkId), eq(chunk.userId, userId)))
            .orderBy(asc(chunkComment.createdAt))
    );
}

export function createComment(params: { id: string; chunkId: string; userId: string; content: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(chunkComment).values(params).returning();
        return created!;
    });
}

export function updateComment(id: string, userId: string, content: string) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(chunkComment)
            .set({ content })
            .where(and(eq(chunkComment.id, id), eq(chunkComment.userId, userId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteComment(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(chunkComment)
            .where(and(eq(chunkComment.id, id), eq(chunkComment.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

export function getCommentCount(chunkId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ count: sql<number>`count(*)` })
            .from(chunkComment)
            .where(eq(chunkComment.chunkId, chunkId));
        return Number(result?.count ?? 0);
    });
}
