import {
    createChunk as createChunkRepo,
    createVersion,
    deleteChunk as deleteChunkRepo,
    deleteMany as deleteManyRepo,
    exportAllChunks as exportAllChunksRepo,
    findOrCreateTag,
    getChunkById,
    getChunkConnections,
    getCodebasesForChunk,
    getNextVersionNumber,
    getVersionsByChunkId,
    listChunks as listChunksRepo,
    semanticSearch as semanticSearchRepo,
    setChunkCodebases,
    setChunkTags,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { enrichChunk, enrichChunkIfEmpty } from "../enrich/service";
import { NotFoundError } from "../errors";
import { generateQueryEmbedding } from "../ollama/client";

export function listChunks(
    userId: string | undefined,
    query: {
        type?: string;
        search?: string;
        limit?: string;
        offset?: string;
        exclude?: string;
        scope?: string;
        alias?: string;
        sort?: "newest" | "oldest" | "alpha" | "updated";
        tags?: string;
        after?: string;
        enrichment?: "missing" | "complete";
        minConnections?: string;
        codebaseId?: string;
        global?: string;
    }
) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);
    const exclude = query.exclude ? query.exclude.split(",").map(s => s.trim()) : undefined;
    const scope = query.scope
        ? Object.fromEntries(
              query.scope
                  .split(",")
                  .map(s => s.trim().split(":"))
                  .filter(p => p.length === 2) as [string, string][]
          )
        : undefined;
    const parsedTags = query.tags
        ?.split(",")
        .map(s => s.trim())
        .filter(Boolean);
    const tags = parsedTags?.length ? parsedTags : undefined;
    const after = query.after ? new Date(Date.now() - Number(query.after) * 86400000) : undefined;
    const minConnections = query.minConnections ? Number(query.minConnections) : undefined;
    const globalOnly = query.global === "true";
    return listChunksRepo({
        userId,
        type: query.type,
        search: query.search,
        exclude,
        scope,
        alias: query.alias,
        sort: query.sort,
        tags,
        after,
        enrichment: query.enrichment,
        minConnections,
        codebaseId: query.codebaseId,
        globalOnly,
        limit,
        offset
    }).pipe(Effect.map(result => ({ ...result, limit, offset })));
}

export function getChunkDetail(chunkId: string, userId?: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(found =>
            Effect.all({
                chunk: Effect.succeed(found),
                connections: getChunkConnections(chunkId),
                codebases: getCodebasesForChunk(chunkId)
            })
        )
    );
}

export function createChunk(
    userId: string,
    body: {
        title: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
    }
) {
    const id = crypto.randomUUID();
    return createChunkRepo({
        id,
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        userId,
        rationale: body.rationale,
        alternatives: body.alternatives,
        consequences: body.consequences
    }).pipe(
        Effect.tap(() => {
            if (body.tags && body.tags.length > 0) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(id, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.codebaseIds && body.codebaseIds.length > 0) {
                return setChunkCodebases(id, body.codebaseIds);
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            Effect.runPromise(enrichChunkIfEmpty(id)).catch(() => {});
            return Effect.void;
        })
    );
}

export function updateChunk(
    chunkId: string,
    userId: string,
    body: {
        title?: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        summary?: string | null;
        aliases?: string[];
        notAbout?: string[];
        scope?: Record<string, string>;
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
    }
) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(existing => Effect.all({ existing: Effect.succeed(existing), version: getNextVersionNumber(chunkId) })),
        Effect.flatMap(({ existing, version }) =>
            createVersion({
                id: crypto.randomUUID(),
                chunkId,
                version,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: []
            })
        ),
        Effect.flatMap(() => {
            const { tags: _tags, codebaseIds: _codebaseIds, ...repoBody } = body;
            return updateChunkRepo(chunkId, repoBody);
        }),
        Effect.tap(() => {
            if (body.tags) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(chunkId, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.codebaseIds) {
                return setChunkCodebases(chunkId, body.codebaseIds);
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.title !== undefined || body.content !== undefined) {
                Effect.runPromise(enrichChunk(chunkId)).catch(() => {});
            }
            return Effect.void;
        })
    );
}

export function getChunkHistory(chunkId: string, userId?: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => getVersionsByChunkId(chunkId))
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

export function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function deleteMany(ids: string[], userId: string) {
    return deleteManyRepo(ids, userId);
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
