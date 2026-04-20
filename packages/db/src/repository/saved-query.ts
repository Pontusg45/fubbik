import { and, desc, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { savedQuery } from "../schema/saved-query";

export function listSavedQueries(userId: string, codebaseId?: string) {
    return dbEffect(() => {
            const conditions = [eq(savedQuery.userId, userId)];
            if (codebaseId) conditions.push(eq(savedQuery.codebaseId, codebaseId));
            return db
                .select()
                .from(savedQuery)
                .where(and(...conditions))
                .orderBy(desc(savedQuery.createdAt));
        });
}

export function createSavedQuery(params: {
    id: string;
    name: string;
    query: unknown;
    userId: string;
    codebaseId?: string;
}) {
    return dbEffect(async () => {
            const [created] = await db.insert(savedQuery).values(params).returning();
            return created;
        });
}

export function deleteSavedQuery(id: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(savedQuery)
                .where(and(eq(savedQuery.id, id), eq(savedQuery.userId, userId)))
                .returning();
            return deleted ?? null;
        });
}
