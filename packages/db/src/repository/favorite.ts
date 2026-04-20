import { and, asc, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { userFavorite } from "../schema/favorite";

export function listFavorites(userId: string) {
    return dbEffect(() =>
            db
                .select()
                .from(userFavorite)
                .where(eq(userFavorite.userId, userId))
                .orderBy(asc(userFavorite.order)));
}

export function addFavorite(params: { id: string; userId: string; chunkId: string; order: number }) {
    return dbEffect(async () => {
            const [created] = await db
                .insert(userFavorite)
                .values(params)
                .onConflictDoNothing()
                .returning();
            return created ?? null;
        });
}

export function removeFavorite(userId: string, chunkId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(userFavorite)
                .where(and(eq(userFavorite.userId, userId), eq(userFavorite.chunkId, chunkId)))
                .returning();
            return deleted ?? null;
        });
}

export function isFavorite(userId: string, chunkId: string) {
    return dbEffect(async () => {
            const [found] = await db
                .select()
                .from(userFavorite)
                .where(and(eq(userFavorite.userId, userId), eq(userFavorite.chunkId, chunkId)));
            return !!found;
        });
}

export function reorderFavorites(userId: string, favorites: { chunkId: string; order: number }[]) {
    return dbEffect(async () => {
            await db.transaction(async tx => {
                for (const fav of favorites) {
                    await tx
                        .update(userFavorite)
                        .set({ order: fav.order })
                        .where(and(eq(userFavorite.userId, userId), eq(userFavorite.chunkId, fav.chunkId)));
                }
            });
        });
}
