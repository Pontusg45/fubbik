import {
    exportAllChunks as exportAllChunksRepo,
    findNeighborsByChunkId,
    getChunkById,
    getDistinctUpdateTags,
    getVersionsByTag,
    semanticSearch as semanticSearchRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";
import { generateQueryEmbedding } from "../ollama/client";
import { createChunk } from "./chunk-mutations";

interface NeighborItem {
    id: string;
    title: string;
    summary: string | null;
    type: string;
    distance: number;
}

export interface NeighborsResult {
    neighbors: NeighborItem[];
    note: string | null;
}

export function getChunkNeighbors(chunkId: string, userId: string, k: number) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(source => {
            if (!source.embedding) {
                return Effect.succeed<NeighborsResult>({
                    neighbors: [],
                    note: "Chunk has no embedding — run enrichment first."
                });
            }
            return findNeighborsByChunkId(chunkId, userId, k).pipe(
                Effect.map((neighbors): NeighborsResult => ({ neighbors, note: null }))
            );
        })
    );
}

export function exportChunks(userId?: string) {
    return exportAllChunksRepo(userId);
}

export function importChunks(userId: string, chunks: { title: string; content?: string; type?: string; tags?: string[] }[]) {
    return Effect.all(
        chunks.map(c => createChunk(userId, c)),
        { concurrency: 10 }
    );
}

export function semanticSearch(userId: string | undefined, query: { q: string; limit?: string; exclude?: string; scope?: string }) {
    const limit = Math.min(Number(query.limit ?? 5), 20);
    const exclude = query.exclude ? query.exclude.split(",").map(s => s.trim()) : undefined;
    const scope = query.scope
        ? Object.fromEntries(
              query.scope
                  .split(",")
                  .map(s => s.trim().split(":"))
                  .filter(p => p.length === 2) as [string, string][]
          )
        : undefined;

    return generateQueryEmbedding(query.q).pipe(
        Effect.flatMap(embedding => semanticSearchRepo({ embedding, userId, exclude, scope, limit }))
    );
}

export function listUpdatesByTag(userId: string, tag: string, codebaseId?: string) {
    return getVersionsByTag(tag, userId, codebaseId).pipe(
        Effect.map(versions => versions.map(v => ({
            versionId: v.versionId,
            chunkId: v.chunkId,
            chunkTitle: v.chunkTitle,
            updateTag: v.updateTag,
            version: v.version,
            createdAt: v.createdAt,
            before: v.version === 0
                ? { title: null, content: null, type: null, rationale: null, alternatives: null, consequences: null, scope: null }
                : { title: v.title, content: v.content, type: v.type, rationale: v.rationale, alternatives: v.alternatives, consequences: v.consequences, scope: v.scope },
            after: {
                title: v.chunkTitle,
                content: v.chunkContent,
                type: v.chunkType,
                rationale: v.chunkRationale,
                alternatives: v.chunkAlternatives,
                consequences: v.chunkConsequences,
                scope: v.chunkScope,
            }
        })))
    );
}

export function listUpdateTags(userId: string, codebaseId?: string) {
    return getDistinctUpdateTags(userId, codebaseId);
}
