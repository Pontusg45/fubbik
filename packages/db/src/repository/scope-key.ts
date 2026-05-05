import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { scopeKey } from "../schema/scope-key";

export function listScopeKeys(userId: string) {
    return dbEffect(() =>
        db
            .select()
            .from(scopeKey)
            .where(eq(scopeKey.userId, userId))
            .orderBy(scopeKey.key)
    );
}

export function createScopeKey(params: {
    id: string;
    userId: string;
    key: string;
    description?: string;
    valueType?: string;
    allowedValues?: unknown;
}) {
    return dbEffect(async () => {
        const [created] = await db
            .insert(scopeKey)
            .values({
                id: params.id,
                userId: params.userId,
                key: params.key,
                description: params.description ?? null,
                valueType: params.valueType ?? "string",
                allowedValues: params.allowedValues ?? null,
            })
            .returning();
        return created!;
    });
}

export function deleteScopeKey(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(scopeKey)
            .where(and(eq(scopeKey.id, id), eq(scopeKey.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}
