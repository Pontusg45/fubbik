import {
    createChunk as createChunkRepo,
    createVersion,
    deleteChunk as deleteChunkRepo,
    exportAllChunks as exportAllChunksRepo,
    getChunkById,
    getChunkConnections,
    getNextVersionNumber,
    getVersionsByChunkId,
    listChunks as listChunksRepo,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function listChunks(userId: string, query: { type?: string; search?: string; limit?: string; offset?: string }) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);
    return listChunksRepo({ userId, type: query.type, search: query.search, limit, offset }).pipe(
        Effect.map(result => ({ ...result, limit, offset }))
    );
}

export function getChunkDetail(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(found => getChunkConnections(chunkId).pipe(Effect.map(connections => ({ chunk: found, connections }))))
    );
}

export function createChunk(userId: string, body: { title: string; content?: string; type?: string; tags?: string[] }) {
    return createChunkRepo({
        id: crypto.randomUUID(),
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        tags: body.tags ?? [],
        userId
    });
}

export function updateChunk(chunkId: string, userId: string, body: { title?: string; content?: string; type?: string; tags?: string[] }) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(existing =>
            Effect.all({ existing: Effect.succeed(existing), version: getNextVersionNumber(chunkId) })
        ),
        Effect.flatMap(({ existing, version }) =>
            createVersion({
                id: crypto.randomUUID(),
                chunkId,
                version,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: existing.tags as string[]
            })
        ),
        Effect.flatMap(() => updateChunkRepo(chunkId, body))
    );
}

export function getChunkHistory(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => getVersionsByChunkId(chunkId))
    );
}

export function exportChunks(userId: string) {
    return exportAllChunksRepo(userId);
}

export function importChunks(userId: string, chunks: { title: string; content?: string; type?: string; tags?: string[] }[]) {
    return Effect.all(
        chunks.map(c => createChunk(userId, c)),
        { concurrency: 10 }
    );
}

export function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}
