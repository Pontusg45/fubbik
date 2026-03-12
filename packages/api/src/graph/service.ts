import { getAllChunksMeta, getAllConnectionsForUser, getAllTagsWithTypes, getTagTypesForGraph } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserGraph(userId?: string) {
    return Effect.all(
        {
            chunks: getAllChunksMeta(userId),
            connections: getAllConnectionsForUser(userId),
            chunkTags: getAllTagsWithTypes(userId),
            tagTypes: getTagTypesForGraph(userId)
        },
        { concurrency: "unbounded" }
    );
}
