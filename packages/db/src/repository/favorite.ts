import { and, asc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { userFavorite } from "../schema/favorite";

export function listFavorites(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(userFavorite)
                .where(eq(userFavorite.userId, userId))
                .orderBy(asc(userFavorite.order)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function addFavorite(params: { id: string; userId: string; chunkId: string; order: number }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(userFavorite)
                .values(params)
                .onConflictDoNothing()
                .returning();
            return created ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function removeFavorite(userId: string, chunkId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(userFavorite)
                .where(and(eq(userFavorite.userId, userId), eq(userFavorite.chunkId, chunkId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function isFavorite(userId: string, chunkId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(userFavorite)
                .where(and(eq(userFavorite.userId, userId), eq(userFavorite.chunkId, chunkId)));
            return !!found;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function reorderFavorites(userId: string, favorites: { chunkId: string; order: number }[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (const fav of favorites) {
                    await tx
                        .update(userFavorite)
                        .set({ order: fav.order })
                        .where(and(eq(userFavorite.userId, userId), eq(userFavorite.chunkId, fav.chunkId)));
                }
            });
        },
        catch: cause => new DatabaseError({ cause })
    });
}
