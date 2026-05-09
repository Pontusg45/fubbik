import {
    batchFetchDeltas,
    getAppliesToForChunk,
    getChunkById,
    getChunkConnections,
    getCodebasesForChunk,
    getCodebasesForChunks,
    getDeltasForChunk as getDeltasForChunkRepo,
    getFileRefsForChunk,
    getRequirementsForChunks,
    getTagsForChunk,
    getVersionsByChunkId,
    listChunks as listChunksRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { resolveChunks } from "../features/resolve";
import { NotFoundError } from "../errors";
import { computeHealthScore } from "./health-score";

export * from "./chunk-mutations";
export * from "./chunk-import";
export * from "./chunk-search";

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
        tagMode?: "any" | "all";
        after?: string;
        enrichment?: "missing" | "complete";
        minConnections?: string;
        codebaseId?: string;
        workspaceId?: string;
        global?: string;
        origin?: string;
        reviewStatus?: string;
        allCodebases?: string;
    },
    activeFeatureIds: string[] = []
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
        tagMode: query.tagMode,
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
    ).pipe(
        Effect.flatMap(result => {
            if (activeFeatureIds.length === 0 || result.chunks.length === 0) {
                return Effect.succeed(result);
            }
            const chunkIds = result.chunks.map((c: { id: string }) => c.id);
            return batchFetchDeltas(chunkIds, activeFeatureIds).pipe(
                Effect.map(deltas => ({
                    ...result,
                    chunks: resolveChunks(result.chunks, activeFeatureIds, deltas)
                }))
            );
        })
    );
}

export function getChunkDetail(chunkId: string, userId?: string, activeFeatureIds: string[] = []) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(found =>
            Effect.all({
                chunk: Effect.succeed(found),
                connections: getChunkConnections(chunkId),
                codebases: getCodebasesForChunk(chunkId),
                appliesTo: getAppliesToForChunk(chunkId),
                fileReferences: getFileRefsForChunk(chunkId),
                tags: getTagsForChunk(chunkId),
                requirements: getRequirementsForChunks([chunkId]),
                allDeltas: getDeltasForChunkRepo(chunkId)
            })
        ),
        Effect.map(result => {
            const chunkRequirements = result.requirements.filter(r => r.chunkId === chunkId);
            const requirementCount = chunkRequirements.length;
            const allRequirementsPassing = requirementCount > 0 && chunkRequirements.every(r => r.status === "passing");
            const healthScore = computeHealthScore({
                content: result.chunk.content,
                updatedAt: result.chunk.updatedAt,
                summary: result.chunk.summary,
                rationale: result.chunk.rationale,
                alternatives: result.chunk.alternatives,
                consequences: result.chunk.consequences,
                connectionCount: result.connections.length,
                centralityDegree: 0,
                hasEmbedding: result.chunk.embedding != null,
                requirementCount,
                allRequirementsPassing,
                referencedInSession: false
            });

            // Resolve chunk through active features
            let resolvedChunk = result.chunk as Record<string, unknown>;
            const _appliedFeatures: string[] = [];
            const _hasDeltas = result.allDeltas.length > 0;
            if (activeFeatureIds.length > 0 && result.allDeltas.length > 0) {
                const activeDeltas = result.allDeltas
                    .filter(d => activeFeatureIds.includes(d.featureId))
                    .sort((a, b) => a.featurePriority - b.featurePriority);
                for (const d of activeDeltas) {
                    resolvedChunk = { ...resolvedChunk, ...(d.delta as Record<string, unknown>) };
                    _appliedFeatures.push(d.featureId);
                }
            }

            return {
                ...result,
                chunk: resolvedChunk,
                healthScore,
                _appliedFeatures,
                _hasDeltas,
                deltas: result.allDeltas
            };
        })
    );
}

export function getChunkHistory(chunkId: string, userId?: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => getVersionsByChunkId(chunkId))
    );
}
