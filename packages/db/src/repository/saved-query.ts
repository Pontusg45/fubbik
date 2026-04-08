import { and, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { savedQuery } from "../schema/saved-query";

export function listSavedQueries(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(savedQuery.userId, userId)];
            if (codebaseId) conditions.push(eq(savedQuery.codebaseId, codebaseId));
            return db
                .select()
                .from(savedQuery)
                .where(and(...conditions))
                .orderBy(desc(savedQuery.createdAt));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createSavedQuery(params: {
    id: string;
    name: string;
    query: unknown;
    userId: string;
    codebaseId?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(savedQuery).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteSavedQuery(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(savedQuery)
                .where(and(eq(savedQuery.id, id), eq(savedQuery.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
