import { desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db, dbEffect } from "../index";
import { contextSnapshot, type ContextSnapshot, type NewContextSnapshot } from "../schema/context-snapshot";

export function createSnapshot(input: NewContextSnapshot): Effect.Effect<ContextSnapshot, DatabaseError> {
    return dbEffect(async () => {
            const [row] = await db.insert(contextSnapshot).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        });
}

export function getSnapshotById(id: string): Effect.Effect<ContextSnapshot | null, DatabaseError> {
    return dbEffect(async () => {
            const [row] = await db.select().from(contextSnapshot).where(eq(contextSnapshot.id, id)).limit(1);
            return row ?? null;
        });
}

export function listSnapshots(userId: string): Effect.Effect<ContextSnapshot[], DatabaseError> {
    return dbEffect(async () =>
            db.select().from(contextSnapshot)
                .where(eq(contextSnapshot.userId, userId))
                .orderBy(desc(contextSnapshot.createdAt))
                .limit(50));
}

export function deleteSnapshot(id: string): Effect.Effect<void, DatabaseError> {
    return dbEffect(async () => { await db.delete(contextSnapshot).where(eq(contextSnapshot.id, id)); });
}
