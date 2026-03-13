import { getAppliesToForChunk, getChunkById, setAppliesToForChunk } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function getAppliesTo(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => getAppliesToForChunk(chunkId))
    );
}

export function setAppliesTo(chunkId: string, userId: string, patterns: { pattern: string; note?: string | null }[]) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => setAppliesToForChunk(chunkId, patterns))
    );
}
