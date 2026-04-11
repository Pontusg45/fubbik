import { and, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { learningPath } from "../schema/learning-path";

export function listLearningPaths(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(learningPath)
                .where(eq(learningPath.userId, userId))
                .orderBy(desc(learningPath.updatedAt)),
        catch: cause => new DatabaseError({ cause }),
    });
}

export function getLearningPath(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .select()
                .from(learningPath)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)));
            return row ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function createLearningPath(params: {
    id: string;
    title: string;
    description?: string;
    chunkIds: string[];
    userId: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(learningPath).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function updateLearningPath(
    id: string,
    userId: string,
    params: { title?: string; description?: string; chunkIds?: string[] }
) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(learningPath)
                .set(params)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function deleteLearningPath(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(learningPath)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
