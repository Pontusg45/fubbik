import { and, desc, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { learningPath } from "../schema/learning-path";

export function listLearningPaths(userId: string) {
    return dbEffect(() =>
            db
                .select()
                .from(learningPath)
                .where(eq(learningPath.userId, userId))
                .orderBy(desc(learningPath.updatedAt)));
}

export function getLearningPath(id: string, userId: string) {
    return dbEffect(async () => {
            const [row] = await db
                .select()
                .from(learningPath)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)));
            return row ?? null;
        });
}

export function createLearningPath(params: {
    id: string;
    title: string;
    description?: string;
    chunkIds: string[];
    userId: string;
}) {
    return dbEffect(async () => {
            const [created] = await db.insert(learningPath).values(params).returning();
            return created;
        });
}

export function updateLearningPath(
    id: string,
    userId: string,
    params: { title?: string; description?: string; chunkIds?: string[] }
) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(learningPath)
                .set(params)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)))
                .returning();
            return updated ?? null;
        });
}

export function deleteLearningPath(id: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(learningPath)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)))
                .returning();
            return deleted ?? null;
        });
}
