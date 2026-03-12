import { and, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { tagType } from "../schema/tag";

export function createTagType(params: { id: string; name: string; color: string; userId: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(tagType).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagTypesForUser(userId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(tagType).where(eq(tagType.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateTagType(id: string, userId: string, data: { name?: string; color?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(tagType)
                .set(data)
                .where(and(eq(tagType.id, id), eq(tagType.userId, userId)))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteTagType(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(tagType)
                .where(and(eq(tagType.id, id), eq(tagType.userId, userId)))
                .returning();
            return deleted;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
