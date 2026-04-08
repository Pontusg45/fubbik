import {
    listChunks,
    getTagsForChunks,
    getChunkConnections,
    getTagsForUser,
    findShortestPath,
    getNeighborhood,
    getChunksAffectedByRequirement
} from "@fubbik/db/repository";
import { db } from "@fubbik/db";
import { chunk } from "@fubbik/db/schema/chunk";
import { requirement } from "@fubbik/db/schema/requirement";
import { Effect } from "effect";
import { ilike } from "drizzle-orm";

import type { QueryClause, SearchQuery, SearchResult, SearchResultChunk, GraphContext } from "./types";

const GRAPH_FIELDS = new Set(["near", "path", "affected-by"]);

function isGraphClause(clause: QueryClause): boolean {
    return GRAPH_FIELDS.has(clause.field);
}

function mapSortParam(sort?: SearchQuery["sort"]): "newest" | "oldest" | "alpha" | "updated" | undefined {
    if (!sort || sort === "relevance") return undefined;
    if (sort === "newest" || sort === "oldest" || sort === "updated") return sort;
    return undefined;
}

function buildListChunksParams(
    userId: string | undefined,
    clauses: QueryClause[],
    query: SearchQuery
): Parameters<typeof listChunks>[0] {
    const params: Parameters<typeof listChunks>[0] = {
        userId,
        limit: query.limit ?? 50,
        offset: query.offset ?? 0,
        codebaseId: query.codebaseId,
        sort: mapSortParam(query.sort)
    };

    for (const clause of clauses) {
        switch (clause.field) {
            case "type":
                params.type = clause.value;
                break;
            case "tag":
                params.tags = clause.value.split(",").map(t => t.trim()).filter(Boolean);
                break;
            case "text":
                params.search = clause.value;
                break;
            case "connections":
                params.minConnections = Number(clause.value);
                break;
            case "updated":
                params.after = new Date(Date.now() - Number(clause.value) * 86400000);
                break;
            case "origin":
                params.origin = clause.value;
                break;
            case "review":
                params.reviewStatus = clause.value;
                break;
        }
    }

    return params;
}

export function executeSearch(userId: string | undefined, searchQuery: SearchQuery): Effect.Effect<SearchResult, never> {
    const graphClauses = searchQuery.clauses.filter(isGraphClause);
    const standardClauses = searchQuery.clauses.filter(c => !isGraphClause(c));

    return Effect.gen(function* () {
        let graphIds: string[] | undefined;
        let graphMeta: SearchResult["graphMeta"] | undefined;

        // Execute graph clauses
        for (const clause of graphClauses) {
            if (clause.field === "near") {
                const hops = clause.params?.hops ? Number(clause.params.hops) : 1;
                const ids = yield* getNeighborhood(clause.value, hops).pipe(
                    Effect.orElse(() => Effect.succeed([] as string[]))
                );
                graphIds = graphIds ? graphIds.filter(id => ids.includes(id)) : ids;
                graphMeta = { type: "neighborhood", referenceChunk: clause.value };
            } else if (clause.field === "path") {
                const [chunkA, chunkB] = clause.value.split(",").map(s => s.trim());
                if (chunkA && chunkB) {
                    const path = yield* findShortestPath(chunkA, chunkB).pipe(
                        Effect.orElse(() => Effect.succeed(null as string[] | null))
                    );
                    const ids = path ?? [];
                    graphIds = graphIds ? graphIds.filter(id => ids.includes(id)) : ids;
                    graphMeta = { type: "path", pathChunks: ids };
                }
            } else if (clause.field === "affected-by") {
                const hops = clause.params?.hops ? Number(clause.params.hops) : 2;
                const ids = yield* getChunksAffectedByRequirement(clause.value, hops).pipe(
                    Effect.orElse(() => Effect.succeed([] as string[]))
                );
                graphIds = graphIds ? graphIds.filter(id => ids.includes(id)) : ids;
                graphMeta = { type: "requirement-reach" };
            }
        }

        // If graph clauses produced no IDs, return empty early
        if (graphIds !== undefined && graphIds.length === 0) {
            return { chunks: [], total: 0, graphMeta };
        }

        // Execute standard query
        const params = buildListChunksParams(userId, standardClauses, searchQuery);
        const result = yield* listChunks(params).pipe(
            Effect.orElse(() => Effect.succeed({ chunks: [] as any[], total: 0 }))
        );

        let filteredChunks = result.chunks;

        // Intersect with graph IDs
        if (graphIds !== undefined) {
            const idSet = new Set(graphIds);
            filteredChunks = filteredChunks.filter(c => idSet.has(c.id));

            // If no standard clauses were specified, fetch the graph-ID chunks directly
            if (standardClauses.length === 0) {
                const fetchedIds = new Set(filteredChunks.map(c => c.id));
                const missingIds = graphIds.filter(id => !fetchedIds.has(id));
                if (missingIds.length > 0) {
                    const extraResult = yield* listChunks({
                        limit: missingIds.length,
                        offset: 0
                    }).pipe(Effect.orElse(() => Effect.succeed({ chunks: [] as any[], total: 0 })));
                    const extraFiltered = extraResult.chunks.filter(c => missingIds.includes(c.id));
                    filteredChunks = [...filteredChunks, ...extraFiltered];
                }
            }
        }

        if (filteredChunks.length === 0) {
            return { chunks: [], total: 0, graphMeta };
        }

        // Enrich with tags
        const chunkIds = filteredChunks.map(c => c.id);
        const tagRows = yield* getTagsForChunks(chunkIds).pipe(
            Effect.orElse(() => Effect.succeed([] as any[]))
        );

        const tagsByChunk = new Map<string, string[]>();
        for (const row of tagRows) {
            const existing = tagsByChunk.get(row.chunkId) ?? [];
            existing.push(row.tagName);
            tagsByChunk.set(row.chunkId, existing);
        }

        // Enrich with connection counts (concurrently)
        const connectionCounts = new Map<string, number>();
        yield* Effect.forEach(
            chunkIds,
            chunkId =>
                getChunkConnections(chunkId).pipe(
                    Effect.map(conns => { connectionCounts.set(chunkId, conns.length); }),
                    Effect.orElse(() => Effect.succeed(void 0))
                ),
            { concurrency: 5 }
        );

        // Build graph context map
        const graphContextMap = new Map<string, GraphContext>();
        if (graphIds !== undefined && graphMeta) {
            if (graphMeta.type === "path" && graphMeta.pathChunks) {
                graphMeta.pathChunks.forEach((id, idx) => {
                    graphContextMap.set(id, { pathPosition: idx });
                });
            } else if (graphMeta.type === "neighborhood") {
                for (const id of graphIds) {
                    graphContextMap.set(id, { hopDistance: 1 });
                }
            } else if (graphMeta.type === "requirement-reach") {
                for (const id of graphIds) {
                    graphContextMap.set(id, {});
                }
            }
        }

        const chunks: SearchResultChunk[] = filteredChunks.map(c => ({
            id: c.id,
            title: c.title,
            type: c.type,
            summary: c.summary ?? null,
            tags: tagsByChunk.get(c.id) ?? [],
            connectionCount: connectionCounts.get(c.id) ?? 0,
            updatedAt: c.updatedAt,
            graphContext: graphContextMap.get(c.id)
        }));

        const total = graphIds !== undefined ? chunks.length : result.total;

        return { chunks, total, graphMeta } satisfies SearchResult;
    }).pipe(
        Effect.orElse(() => Effect.succeed({ chunks: [], total: 0 } as SearchResult))
    );
}

export function autocomplete(
    userId: string | undefined,
    field: string,
    prefix: string
): Effect.Effect<string[], never> {
    return Effect.gen(function* () {
        if (field === "tag") {
            if (!userId) return [];
            const allTags = yield* getTagsForUser(userId).pipe(
                Effect.orElse(() => Effect.succeed([] as any[]))
            );
            const lower = prefix.toLowerCase();
            return allTags
                .filter(t => t.name.toLowerCase().startsWith(lower))
                .slice(0, 10)
                .map(t => t.name);
        }

        if (field === "chunk") {
            const rows = yield* Effect.tryPromise({
                try: () =>
                    db
                        .select({ title: chunk.title })
                        .from(chunk)
                        .where(ilike(chunk.title, `%${prefix}%`))
                        .limit(10),
                catch: () => [] as { title: string }[]
            }).pipe(Effect.orElse(() => Effect.succeed([] as { title: string }[])));
            return rows.map(r => r.title);
        }

        if (field === "requirement") {
            const rows = yield* Effect.tryPromise({
                try: () =>
                    db
                        .select({ title: requirement.title })
                        .from(requirement)
                        .where(ilike(requirement.title, `%${prefix}%`))
                        .limit(10),
                catch: () => [] as { title: string }[]
            }).pipe(Effect.orElse(() => Effect.succeed([] as { title: string }[])));
            return rows.map(r => r.title);
        }

        return [];
    }).pipe(
        Effect.orElse(() => Effect.succeed([] as string[]))
    );
}
