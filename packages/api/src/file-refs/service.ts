import { getChunkById, getFileRefsForChunk, lookupChunksByFilePath, setFileRefsForChunk } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function getFileRefs(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => getFileRefsForChunk(chunkId))
    );
}

export function setFileRefs(
    chunkId: string,
    userId: string,
    refs: { path: string; anchor?: string | null; relation: string }[]
) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => setFileRefsForChunk(chunkId, refs))
    );
}

export function lookupByPath(path: string, userId: string) {
    return lookupChunksByFilePath(path, userId);
}
