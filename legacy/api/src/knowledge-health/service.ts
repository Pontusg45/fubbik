import { getFileRefsForHealth, getOrphanChunks, getStaleChunks, getStaleEmbeddings, getThinChunks } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getKnowledgeHealth(userId: string, spaceId?: string) {
    return Effect.all(
        {
            orphans: getOrphanChunks(userId, spaceId),
            stale: getStaleChunks(userId, spaceId),
            thin: getThinChunks(userId, spaceId),
            staleEmbeddings: getStaleEmbeddings(userId, spaceId),
            fileRefs: getFileRefsForHealth(userId, spaceId)
        },
        { concurrency: "unbounded" }
    );
}
