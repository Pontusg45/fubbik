import { getAllChunksMeta, getAllConnectionsForUser, getAllTagsWithTypes, getChunkCodebaseMappings, getTagTypesForGraph } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserGraph(userId?: string, codebaseId?: string, workspaceId?: string) {
    return Effect.all(
        {
            chunks: getAllChunksMeta(userId, codebaseId, workspaceId),
            connections: getAllConnectionsForUser(userId),
            chunkTags: getAllTagsWithTypes(userId),
            tagTypes: getTagTypesForGraph(userId),
            chunkCodebases: workspaceId ? getChunkCodebaseMappings(userId) : Effect.succeed([] as { chunkId: string; codebaseId: string; codebaseName: string }[])
        },
        { concurrency: "unbounded" }
    );
}
