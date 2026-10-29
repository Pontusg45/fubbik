import { getChunkCount, getConnectionCount, getTagCount } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserStats(userId?: string) {
    return Effect.all(
        {
            chunks: getChunkCount(userId),
            connections: getConnectionCount(userId),
            tags: getTagCount(userId)
        },
        { concurrency: "unbounded" }
    );
}
