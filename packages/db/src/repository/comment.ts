import { and, asc, eq, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunkComment } from "../schema/comment";

export function listComments(chunkId: string) {
    return dbEffect(() =>
            db
                .select()
                .from(chunkComment)
                .where(eq(chunkComment.chunkId, chunkId))
                .orderBy(asc(chunkComment.createdAt)));
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
