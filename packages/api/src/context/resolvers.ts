import {
    getChunkById,
    getChunkConnections,
    getChunksForRequirement,
    getStaleFlagsForChunk,
    getTagsForChunk,
    listAnalyzeItems,
    listChunks,
    listPlanRequirements,
    listProposalsForChunk,
    listTaskChunks,
    listTasks,
    semanticSearch as semanticSearchRepo,
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { computeHealthScore } from "../chunks/health-score";
import { getContextForFile } from "../context-for-file/service";
import { generateQueryEmbedding } from "../ollama/client";
import type { ChunkWithMetadata } from "./formatter";
import { scoreChunk } from "./utils";

// ---------------------------------------------------------------------------
// enrichChunks — fetch full chunk rows + stale/proposal metadata
// ---------------------------------------------------------------------------

export function enrichChunks(
    chunkIds: string[],
    userId?: string,
): Effect.Effect<ChunkWithMetadata[], never> {
    if (chunkIds.length === 0) return Effect.succeed([]);

    const unique = [...new Set(chunkIds)];

    return Effect.all(
        unique.map(id =>
            Effect.gen(function* () {
                const row = yield* getChunkById(id, userId).pipe(
                    Effect.catchAll(() => Effect.succeed(null)),
                );
                if (!row) return null;

                const connections = yield* getChunkConnections(id).pipe(
                    Effect.catchAll(() => Effect.succeed([])),
                );
                const tags = yield* getTagsForChunk(id).pipe(
                    Effect.catchAll(() => Effect.succeed([])),
                );
                const staleFlags = yield* getStaleFlagsForChunk(id).pipe(
                    Effect.catchAll(() => Effect.succeed([])),
                );
                const proposals = yield* listProposalsForChunk(id, "pending").pipe(
                    Effect.catchAll(() => Effect.succeed([])),
                );

                const connectionCount = connections.length;
                const score = scoreChunk(row, connectionCount);

                // Health score is embedded inside scoreChunk but we need raw total
                const health = computeHealthScore({
                    content: row.content,
                    updatedAt: row.updatedAt,
                    summary: row.summary,
                    rationale: row.rationale,
                    alternatives: row.alternatives,
                    consequences: row.consequences,
                    connectionCount,
                    hasEmbedding: row.embedding != null,
                    requirementCount: 0,
                    allRequirementsPassing: false,
                    referencedInSession: false,
                });

                const isStale = staleFlags.length > 0;
                const hasPendingProposal = proposals.length > 0;

                return {
                    id: row.id,
                    title: row.title,
                    content: row.content,
                    type: row.type,
                    rationale: row.rationale,
                    tags: tags.map((t: { tagName: string }) => t.tagName),
                    score,
                    healthScore: health.total,
                    isStale,
                    hasPendingProposal,
                } satisfies ChunkWithMetadata;
            }),
        ),
        { concurrency: 8 },
    ).pipe(
        Effect.map(results => results.filter((r): r is ChunkWithMetadata => r !== null)),
    );
}

// ---------------------------------------------------------------------------
// resolveForPlan — collect chunk IDs from analyze items, requirements, tasks
// ---------------------------------------------------------------------------

export function resolveForPlan(planId: string): Effect.Effect<string[], never> {
    return Effect.gen(function* () {
        const ids = new Set<string>();

        // 1. plan_analyze_item where kind=chunk
        const analyzeItems = yield* listAnalyzeItems(planId).pipe(
            Effect.catchAll(() => Effect.succeed([])),
        );
        for (const item of analyzeItems) {
            if (item.kind === "chunk" && item.chunkId) {
                ids.add(item.chunkId);
            }
        }

        // 2. plan_requirement → requirement_chunk
        const planReqs = yield* listPlanRequirements(planId).pipe(
            Effect.catchAll(() => Effect.succeed([])),
        );
        const reqChunkResults = yield* Effect.all(
            planReqs.map(pr =>
                getChunksForRequirement(pr.requirementId).pipe(
                    Effect.catchAll(() => Effect.succeed([])),
                ),
            ),
            { concurrency: 5 },
        );
        for (const chunks of reqChunkResults) {
            for (const c of chunks) {
                ids.add(c.id);
            }
        }

        // 3. plan_task → plan_task_chunk
        const tasks = yield* listTasks(planId).pipe(
            Effect.catchAll(() => Effect.succeed([])),
        );
        const taskChunkResults = yield* Effect.all(
            tasks.map(t =>
                listTaskChunks(t.id).pipe(
                    Effect.catchAll(() => Effect.succeed([])),
                ),
            ),
            { concurrency: 5 },
        );
        for (const taskChunks of taskChunkResults) {
            for (const tc of taskChunks) {
                ids.add(tc.chunkId);
            }
        }

        return [...ids];
    });
}

// ---------------------------------------------------------------------------
// resolveForConcept — semantic + text search
// ---------------------------------------------------------------------------

export function resolveForConcept(
    query: string,
    userId?: string,
    codebaseId?: string,
): Effect.Effect<string[], never> {
    return Effect.gen(function* () {
        const ids = new Set<string>();

        // Semantic search (requires Ollama; fall back silently if unavailable)
        const semanticIds = yield* generateQueryEmbedding(query).pipe(
            Effect.flatMap(embedding =>
                semanticSearchRepo({ embedding, userId, limit: 20 }),
            ),
            Effect.map(results => results.map((r: { id: string }) => r.id)),
            Effect.catchAll(() => Effect.succeed([] as string[])),
        );
        for (const id of semanticIds) ids.add(id);

        // Text search
        const textResults = yield* listChunks({
            userId,
            codebaseId,
            search: query,
            limit: 20,
            offset: 0,
        }).pipe(
            Effect.map(r => r.chunks.map((c: { id: string }) => c.id)),
            Effect.catchAll(() => Effect.succeed([] as string[])),
        );
        for (const id of textResults) ids.add(id);

        return [...ids];
    });
}

// ---------------------------------------------------------------------------
// resolveForFiles — calls getContextForFile for each path, deduplicates
// ---------------------------------------------------------------------------

export function resolveForFiles(
    paths: string[],
    userId: string,
    codebaseId?: string,
): Effect.Effect<string[], never> {
    return Effect.gen(function* () {
        const ids = new Set<string>();

        const results = yield* Effect.all(
            paths.map(path =>
                getContextForFile(userId, path, codebaseId).pipe(
                    Effect.catchAll(() => Effect.succeed({ chunks: [], requirements: [] })),
                ),
            ),
            { concurrency: 5 },
        );

        for (const result of results) {
            for (const c of result.chunks) {
                ids.add(c.id);
            }
        }

        return [...ids];
    });
}
