import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { tagType } from "../schema/tag";

export function createTagType(params: { id: string; name: string; color: string; icon?: string | null; userId: string }) {
    return dbEffect(async () => {
            const [created] = await db.insert(tagType).values(params).returning();
            return created!;
        });
}

export function getTagTypesForUser(userId: string) {
    return dbEffect(() => db.select().from(tagType).where(eq(tagType.userId, userId)));
}

export function updateTagType(id: string, userId: string, data: { name?: string; color?: string; icon?: string | null }) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(tagType)
                .set(data)
                .where(and(eq(tagType.id, id), eq(tagType.userId, userId)))
                .returning();
            return updated;
        });
}

export function deleteTagType(id: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(tagType)
                .where(and(eq(tagType.id, id), eq(tagType.userId, userId)))
                .returning();
            return deleted;
        });
}
