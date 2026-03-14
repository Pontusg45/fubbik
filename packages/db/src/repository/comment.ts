import { and, asc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkComment } from "../schema/comment";

export function listComments(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkComment)
                .where(eq(chunkComment.chunkId, chunkId))
                .orderBy(asc(chunkComment.createdAt)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function createComment(params: { id: string; chunkId: string; userId: string; content: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(chunkComment).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateComment(id: string, userId: string, content: string) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunkComment)
                .set({ content })
                .where(and(eq(chunkComment.id, id), eq(chunkComment.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteComment(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(chunkComment)
                .where(and(eq(chunkComment.id, id), eq(chunkComment.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCommentCount(chunkId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkComment)
                .where(eq(chunkComment.chunkId, chunkId));
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}
