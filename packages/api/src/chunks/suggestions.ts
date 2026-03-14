import {
    findChunksSharingTags,
    findChunksWithSimilarTitle,
    getChunkById,
    getChunkConnections,
    getTagsForChunk
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export interface Suggestion {
    id: string;
    title: string;
    type: string;
    reason: string;
}

export function getConnectionSuggestions(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" }))
        ),
        Effect.flatMap(targetChunk =>
            Effect.all({
                targetChunk: Effect.succeed(targetChunk),
                connections: getChunkConnections(chunkId),
                tags: getTagsForChunk(chunkId)
            })
        ),
        Effect.flatMap(({ targetChunk, connections, tags }) => {
            const connectedIds = new Set<string>();
            connectedIds.add(chunkId);
            for (const conn of connections) {
                connectedIds.add(conn.sourceId);
                connectedIds.add(conn.targetId);
            }

            return Effect.all({
                tagMatches: findChunksSharingTags(chunkId, userId, tags.map(t => t.id), connectedIds),
                titleMatches: findChunksWithSimilarTitle(targetChunk.title, userId, connectedIds)
            }).pipe(
                Effect.map(({ tagMatches, titleMatches }) => {
                    const seen = new Set<string>();
                    const suggestions: Suggestion[] = [];

                    for (const match of tagMatches) {
                        if (seen.has(match.id)) continue;
                        seen.add(match.id);
                        suggestions.push(match);
                    }

                    for (const match of titleMatches) {
                        if (seen.has(match.id)) continue;
                        seen.add(match.id);
                        suggestions.push(match);
                    }

                    return suggestions.slice(0, 5);
                })
            );
        })
    );
}
