import {
    addFavorite as addFavoriteRepo,
    getChunkById,
    isFavorite as isFavoriteRepo,
    listFavorites as listFavoritesRepo,
    removeFavorite as removeFavoriteRepo,
    reorderFavorites as reorderFavoritesRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function listFavorites(userId: string) {
    return listFavoritesRepo(userId);
}

export function addFavorite(userId: string, chunkId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(chunk =>
            chunk ? Effect.succeed(chunk) : Effect.fail(new NotFoundError({ resource: "Chunk" }))
        ),
        Effect.flatMap(() => listFavoritesRepo(userId)),
        Effect.flatMap(existing => {
            const nextOrder = existing.length > 0 ? Math.max(...existing.map(f => f.order)) + 1 : 0;
            return addFavoriteRepo({
                id: crypto.randomUUID(),
                userId,
                chunkId,
                order: nextOrder
            });
        })
    );
}

export function removeFavorite(userId: string, chunkId: string) {
    return removeFavoriteRepo(userId, chunkId);
}

export function isFavorite(userId: string, chunkId: string) {
    return isFavoriteRepo(userId, chunkId);
}

export function reorderFavorites(userId: string, favorites: { chunkId: string; order: number }[]) {
    return reorderFavoritesRepo(userId, favorites);
}
