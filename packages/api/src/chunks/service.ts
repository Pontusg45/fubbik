import {
    addPlanChunkRef,
    archiveChunk as archiveChunkRepo,
    createChunk as createChunkRepo,
    createVersion,
    deleteChunk as deleteChunkRepo,
    deleteMany as deleteManyRepo,
    exportAllChunks as exportAllChunksRepo,
    findOrCreateTag,
    getActiveSessionsWithPlan,
    getAppliesToForChunk,
    getChunkById,
    getChunkConnections,
    getCodebasesForChunk,
    getCodebasesForChunks,
    getFileRefsForChunk,
    getNextVersionNumber,
    getTagsForChunk,
    getVersionsByChunkId,
    listArchivedChunks as listArchivedChunksRepo,
    listChunks as listChunksRepo,
    restoreChunk as restoreChunkRepo,
    semanticSearch as semanticSearchRepo,
    setChunkCodebases,
    setChunkTags,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { enrichChunk } from "../enrich/service";
import { parseDocFile } from "./parse-docs";
import { NotFoundError } from "../errors";
import { events, EVENTS } from "../events/bus";
import { generateQueryEmbedding } from "../ollama/client";
import { computeHealthScore } from "./health-score";

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
        workspaceId?: string;
        global?: string;
        origin?: string;
        reviewStatus?: string;
        allCodebases?: string;
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
    const searchAllCodebases = query.allCodebases === "true";
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
        codebaseId: searchAllCodebases ? undefined : query.codebaseId,
        workspaceId: searchAllCodebases ? undefined : query.workspaceId,
        globalOnly: searchAllCodebases ? false : globalOnly,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        limit,
        offset
    }).pipe(
        Effect.flatMap(result => {
            if (!searchAllCodebases || result.chunks.length === 0) {
                return Effect.succeed({ ...result, limit, offset });
            }
            return getCodebasesForChunks(result.chunks.map(c => c.id)).pipe(
                Effect.map(codebaseMap => {
                    const lookup = new Map<string, string[]>();
                    for (const entry of codebaseMap) {
                        const existing = lookup.get(entry.chunkId) ?? [];
                        existing.push(entry.codebaseName);
                        lookup.set(entry.chunkId, existing);
                    }
                    const chunks = result.chunks.map(c => ({
                        ...c,
                        codebaseNames: lookup.get(c.id) ?? []
                    }));
                    return { ...result, chunks, limit, offset };
                })
            );
        })
    );
}

export function getChunkDetail(chunkId: string, userId?: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(found =>
            Effect.all({
                chunk: Effect.succeed(found),
                connections: getChunkConnections(chunkId),
                codebases: getCodebasesForChunk(chunkId),
                appliesTo: getAppliesToForChunk(chunkId),
                fileReferences: getFileRefsForChunk(chunkId),
                tags: getTagsForChunk(chunkId)
            })
        ),
        Effect.map(result => {
            const healthScore = computeHealthScore({
                content: result.chunk.content,
                updatedAt: result.chunk.updatedAt,
                summary: result.chunk.summary,
                rationale: result.chunk.rationale,
                alternatives: result.chunk.alternatives,
                consequences: result.chunk.consequences,
                connectionCount: result.connections.length,
                hasEmbedding: result.chunk.embedding != null
            });
            return { ...result, healthScore };
        })
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
        origin?: string;
        documentId?: string;
        documentOrder?: number;
    }
) {
    const id = crypto.randomUUID();
    const origin = body.origin ?? "human";
    return createChunkRepo({
        id,
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        userId,
        rationale: body.rationale,
        alternatives: body.alternatives,
        consequences: body.consequences,
        origin,
        reviewStatus: origin === "ai" ? "draft" : "approved",
        documentId: body.documentId,
        documentOrder: body.documentOrder
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
            events.emit(EVENTS.CHUNK_CREATED, { chunkId: id, userId });
            return Effect.void;
        }),
        // Auto-link chunk to active plan session
        Effect.tap(() =>
            getActiveSessionsWithPlan(userId).pipe(
                Effect.flatMap(sessions => {
                    const session = sessions[0];
                    if (!session?.planId) return Effect.void;
                    return addPlanChunkRef({
                        id: crypto.randomUUID(),
                        planId: session.planId,
                        chunkId: id,
                        relation: "created"
                    }).pipe(Effect.asVoid);
                }),
                Effect.catchAll(() => Effect.void)
            )
        )
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
        origin?: string;
        reviewStatus?: string;
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
            const updateData: Record<string, unknown> = { ...repoBody };
            if (body.reviewStatus !== undefined) {
                updateData.reviewedBy = userId;
                updateData.reviewedAt = new Date();
            }
            if (Object.keys(updateData).length === 0) return Effect.void;
            return updateChunkRepo(chunkId, updateData as Parameters<typeof updateChunkRepo>[1]).pipe(Effect.asVoid);
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
                Effect.runPromise(enrichChunk(chunkId)).catch(err => {
                    console.error(`[enrich] Failed to re-enrich chunk ${chunkId}:`, err);
                });
            }
            events.emit(EVENTS.CHUNK_UPDATED, { chunkId, userId });
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

export function importDocs(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string
) {
    const results: { created: number; skipped: number; errors: { path: string; error: string }[] } = {
        created: 0,
        skipped: 0,
        errors: []
    };

    return Effect.forEach(
        files,
        file =>
            Effect.try(() => parseDocFile(file.path, file.content)).pipe(
                Effect.flatMap(parsed => {
                    if (!parsed.content && !parsed.title) {
                        results.skipped++;
                        return Effect.void;
                    }
                    return createChunk(userId, {
                        title: parsed.title,
                        content: parsed.content,
                        type: parsed.type,
                        tags: parsed.tags,
                        codebaseIds: [codebaseId]
                    }).pipe(
                        Effect.map(() => {
                            results.created++;
                        })
                    );
                }),
                Effect.catchAll(err => {
                    results.errors.push({ path: file.path, error: String(err) });
                    return Effect.void;
                })
            ),
        { concurrency: 10 }
    ).pipe(Effect.map(() => results));
}

export function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function deleteMany(ids: string[], userId: string) {
    return deleteManyRepo(ids, userId);
}

export function archiveChunk(chunkId: string, userId: string) {
    return archiveChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(archived => (archived ? Effect.succeed(archived) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function restoreChunk(chunkId: string, userId: string) {
    return restoreChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(restored => (restored ? Effect.succeed(restored) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function listArchivedChunks(userId: string, codebaseId?: string) {
    return listArchivedChunksRepo(userId, codebaseId);
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
