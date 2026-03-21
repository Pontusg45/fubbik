import { getFileRefsForHealth, getOrphanChunks, getStaleChunks, getStaleEmbeddings, getThinChunks } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getKnowledgeHealth(userId: string, codebaseId?: string) {
    return Effect.all(
        {
            orphans: getOrphanChunks(userId, codebaseId),
            stale: getStaleChunks(userId, codebaseId),
            thin: getThinChunks(userId, codebaseId),
            staleEmbeddings: getStaleEmbeddings(userId, codebaseId),
            fileRefs: getFileRefsForHealth(userId, codebaseId)
        },
        { concurrency: "unbounded" }
    );
}
