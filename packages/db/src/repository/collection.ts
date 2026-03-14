import { and, asc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { collection, type CollectionFilter } from "../schema/collection";

export type { CollectionFilter } from "../schema/collection";

export function listCollections(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(collection)
                .where(eq(collection.userId, userId))
                .orderBy(asc(collection.name)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCollectionById(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(collection)
                .where(and(eq(collection.id, id), eq(collection.userId, userId)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createCollection(params: {
    id: string;
    name: string;
    description?: string;
    filter: CollectionFilter;
    userId: string;
    codebaseId?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(collection)
                .values(params)
                .returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateCollection(
    id: string,
    userId: string,
    params: {
        name?: string;
        description?: string;
        filter?: CollectionFilter;
    }
) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(collection)
                .set(params)
                .where(and(eq(collection.id, id), eq(collection.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteCollection(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(collection)
                .where(and(eq(collection.id, id), eq(collection.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
