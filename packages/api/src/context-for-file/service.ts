import { getAppliesToForChunks, getChunkById, getConnectionDegrees, getConnectionsForChunks, getGraphProximityBoost, getRequirementsForChunks, incrementConnectionWeights, listChunks, listCodebases, lookupChunksByFilePath, semanticSearch as semanticSearchRepo } from "@fubbik/db/repository";
import { chunk as chunkTable } from "@fubbik/db/schema/chunk";
import { Effect } from "effect";

import { scoreChunk } from "../context/utils";
import { generateQueryEmbedding } from "../ollama/client";
import { globMatch } from "./glob-match";

export interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to" | "dependency" | "semantic" | "connected";
    score: number;
}

export interface ContextRequirement {
    id: string;
    title: string;
    status: string;
    priority: string | null;
    steps: Array<{ keyword: string; text: string }>;
    matchedChunkIds: string[];
}

export interface FileContext {
    chunks: ContextChunk[];
    requirements: ContextRequirement[];
}

const IGNORED_SEGMENTS = new Set(["src", "lib", "dist", "build", "index", "node_modules", "packages", "apps"]);
const IGNORED_EXTENSIONS = new Set(["ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "md"]);

function pathToSearchText(filePath: string): string {
    return filePath
        .split(/[/.]/)
        .filter(seg => seg.length > 0 && !IGNORED_SEGMENTS.has(seg) && !IGNORED_EXTENSIONS.has(seg))
        .join(" ");
}

/**
 * Check if a dependency name matches a codebase name.
 * Supports partial matching: "@acme/auth" matches codebase named "auth".
 */
function depMatchesCodebase(dep: string, codebaseName: string): boolean {
    const depLower = dep.toLowerCase();
    const cbLower = codebaseName.toLowerCase();
    if (depLower === cbLower) return true;
    // Extract last segment after / for scoped packages
    const lastSegment = depLower.split("/").pop()!;
    return lastSegment === cbLower;
}

export function getContextForFile(
    userId: string,
    filePath: string,
    codebaseId?: string,
    deps?: string[]
) {
    return Effect.gen(function* () {
        const results = new Map<string, ContextChunk>();
        type ChunkRow = typeof chunkTable.$inferSelect;
        const chunkRows = new Map<string, ChunkRow>();

        // 1. Direct file-ref matches
        const fileRefMatches = yield* lookupChunksByFilePath(filePath, userId, codebaseId);
        for (const match of fileRefMatches) {
            if (results.has(match.chunkId)) continue;
            const full = yield* getChunkById(match.chunkId, userId);
            if (!full) continue;
            chunkRows.set(full.id, full);
            results.set(match.chunkId, {
                id: full.id,
                title: full.title,
                type: full.type,
                content: full.content,
                summary: full.summary,
                matchReason: "file-ref",
                score: 0
            });
        }

        // 2. Applies-to glob pattern matches
        const { chunks } = yield* listChunks({
            userId,
            codebaseId,
            limit: 1000,
            offset: 0
        });

        const uncheckedIds = chunks.filter(c => !results.has(c.id)).map(c => c.id);
        if (uncheckedIds.length > 0) {
            const allPatterns = yield* getAppliesToForChunks(uncheckedIds);

            // Group patterns by chunkId
            const patternsByChunk = new Map<string, Array<{ pattern: string }>>();
            for (const p of allPatterns) {
                const existing = patternsByChunk.get(p.chunkId) ?? [];
                existing.push(p);
                patternsByChunk.set(p.chunkId, existing);
            }

            for (const c of chunks) {
                if (results.has(c.id)) continue;
                const patterns = patternsByChunk.get(c.id);
                if (!patterns || patterns.length === 0) continue;

                const matches = patterns.some(p => globMatch(p.pattern, filePath));
                if (matches) {
                    chunkRows.set(c.id, c);
                    results.set(c.id, {
                        id: c.id,
                        title: c.title,
                        type: c.type,
                        content: c.content,
                        summary: c.summary,
                        matchReason: "applies-to",
                        score: 0
                    });
                }
            }
        }

        // 3. Dependency-based matches
        if (deps && deps.length > 0) {
            const allCodebases = yield* listCodebases(userId);
            const matchedCodebaseIds: string[] = [];
            for (const cb of allCodebases) {
                if (deps.some(dep => depMatchesCodebase(dep, cb.name))) {
                    matchedCodebaseIds.push(cb.id);
                }
            }

            for (const cbId of matchedCodebaseIds) {
                const { chunks: depChunks } = yield* listChunks({
                    userId,
                    codebaseId: cbId,
                    limit: 5,
                    offset: 0,
                    sort: "updated"
                });
                for (const c of depChunks) {
                    if (results.has(c.id)) continue;
                    chunkRows.set(c.id, c);
                    results.set(c.id, {
                        id: c.id,
                        title: c.title,
                        type: c.type,
                        content: c.content,
                        summary: c.summary,
                        matchReason: "dependency",
                        score: 0
                    });
                }
            }
        }

        // 4. Semantic similarity (requires Ollama; skip silently if unavailable)
        const searchText = pathToSearchText(filePath);
        if (searchText.length > 0) {
            const semanticChunks = yield* generateQueryEmbedding(searchText).pipe(
                Effect.flatMap(embedding =>
                    semanticSearchRepo({ embedding, userId, limit: 10 }),
                ),
                Effect.catchAll(() => Effect.succeed([] as Array<{ id: string; title: string; type: string; content: string; summary: string | null; similarity: number }>)),
            );

            for (const sc of semanticChunks) {
                if (results.has(sc.id)) continue;
                results.set(sc.id, {
                    id: sc.id,
                    title: sc.title,
                    type: sc.type,
                    content: sc.content,
                    summary: sc.summary,
                    matchReason: "semantic",
                    score: 0,
                });
            }
        }

        // 5. Connection expansion — add tightly-coupled chunks
        const currentIds = Array.from(results.keys());
        if (currentIds.length > 0) {
            const allConnections = yield* getConnectionsForChunks(currentIds).pipe(
                Effect.catchAll(() => Effect.succeed([] as Array<{ id: string; sourceId: string; targetId: string; relation: string }>)),
            );

            // Prioritize by relation type
            const RELATION_PRIORITY: Record<string, number> = {
                part_of: 4,
                depends_on: 3,
                extends: 2,
                references: 1,
                related_to: 0,
            };

            // Collect candidate connected chunk IDs
            const candidates: Array<{ chunkId: string; priority: number }> = [];
            for (const conn of allConnections) {
                const connectedId = currentIds.includes(conn.sourceId) ? conn.targetId : conn.sourceId;
                if (!results.has(connectedId)) {
                    candidates.push({
                        chunkId: connectedId,
                        priority: RELATION_PRIORITY[conn.relation] ?? 0,
                    });
                }
            }

            // Sort by priority descending, take top 5
            candidates.sort((a, b) => b.priority - a.priority);
            const topCandidates = candidates.slice(0, 5);

            for (const candidate of topCandidates) {
                const full = yield* getChunkById(candidate.chunkId, userId).pipe(
                    Effect.catchAll(() => Effect.succeed(null)),
                );
                if (!full) continue;
                results.set(candidate.chunkId, {
                    id: full.id,
                    title: full.title,
                    type: full.type,
                    content: full.content,
                    summary: full.summary,
                    matchReason: "connected",
                    score: 0,
                });
                chunkRows.set(full.id, full);
            }
        }

        // Score and sort results
        const matchedChunks = Array.from(results.values());

        // Fetch connection counts for scoring
        const chunkIdsForScoring = matchedChunks.map(c => c.id);
        const connections = chunkIdsForScoring.length > 0
            ? yield* getConnectionsForChunks(chunkIdsForScoring).pipe(
                  Effect.catchAll(() => Effect.succeed([] as Array<{ sourceId: string; targetId: string }>)),
              )
            : [];

        const connCountMap = new Map<string, number>();
        for (const conn of connections) {
            connCountMap.set(conn.sourceId, (connCountMap.get(conn.sourceId) ?? 0) + 1);
            connCountMap.set(conn.targetId, (connCountMap.get(conn.targetId) ?? 0) + 1);
        }

        // Fetch centrality degrees from AGE graph
        const degreeMap = chunkIdsForScoring.length > 0
            ? yield* getConnectionDegrees(chunkIdsForScoring).pipe(
                  Effect.catchAll(() => Effect.succeed(new Map<string, number>())),
              )
            : new Map<string, number>();

        // Hybrid boost: if we have high-confidence anchors (file-ref or applies-to),
        // boost semantic matches that are also graph-connected to them
        const anchorIds = matchedChunks
            .filter(c => c.matchReason === "file-ref" || c.matchReason === "applies-to")
            .map(c => c.id);
        const semanticIds = matchedChunks
            .filter(c => c.matchReason === "semantic")
            .map(c => c.id);

        let graphBoosts = new Map<string, number>();
        if (anchorIds.length > 0 && semanticIds.length > 0) {
            graphBoosts = yield* getGraphProximityBoost(anchorIds[0]!, semanticIds, 3).pipe(
                Effect.catchAll(() => Effect.succeed(new Map<string, number>())),
            );
        }

        const STRATEGY_BONUS: Record<string, number> = {
            "file-ref": 20,
            "applies-to": 10,
            "dependency": 3,
            "semantic": 5,
            "connected": 2,
        };
        const GRAPH_PROXIMITY_WEIGHT = 5;

        for (const chunk of matchedChunks) {
            const rawRow = chunkRows.get(chunk.id);
            const connectionCount = connCountMap.get(chunk.id) ?? 0;
            const centralityDegree = degreeMap.get(chunk.id) ?? 0;
            const baseScore = rawRow ? scoreChunk(rawRow, connectionCount, centralityDegree) : 0;
            const strategyBonus = STRATEGY_BONUS[chunk.matchReason] ?? 0;
            const proximityBonus = (graphBoosts.get(chunk.id) ?? 0) * GRAPH_PROXIMITY_WEIGHT;
            chunk.score = baseScore + strategyBonus + proximityBonus;
        }

        matchedChunks.sort((a, b) => b.score - a.score);

        // 6. Find requirements linked to matched chunks
        const matchedChunkIds = matchedChunks.map(c => c.id);
        const requirements: ContextRequirement[] = [];

        if (matchedChunkIds.length > 0) {
            const rows = yield* getRequirementsForChunks(matchedChunkIds);

            const reqMap = new Map<string, ContextRequirement>();
            for (const row of rows) {
                const existing = reqMap.get(row.id);
                if (existing) {
                    existing.matchedChunkIds.push(row.chunkId);
                } else {
                    reqMap.set(row.id, {
                        id: row.id,
                        title: row.title,
                        status: row.status,
                        priority: row.priority,
                        steps: (row.steps ?? []).map(s => ({ keyword: s.keyword, text: s.text })),
                        matchedChunkIds: [row.chunkId]
                    });
                }
            }
            requirements.push(...reqMap.values());
        }

        // Fire-and-forget: increment edge weights for co-accessed chunks
        if (matchedChunks.length >= 2) {
            const coAccessedIds = matchedChunks.slice(0, 10).map(c => c.id);
            Effect.runPromise(
                incrementConnectionWeights(coAccessedIds).pipe(
                    Effect.catchAll(() => Effect.succeed(0))
                )
            ).catch(() => {});
        }

        return { chunks: matchedChunks, requirements };
    });
}
