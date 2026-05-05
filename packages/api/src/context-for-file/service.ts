import { getAppliesToForChunks, getChunkById, getConnectionsForChunks, getRequirementsForChunks, listChunks, listCodebases, lookupChunksByFilePath } from "@fubbik/db/repository";
import { chunk as chunkTable } from "@fubbik/db/schema/chunk";
import { Effect } from "effect";

import { scoreChunk } from "../context/utils";
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
        const fileRefMatches = yield* lookupChunksByFilePath(filePath, userId);
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

        const STRATEGY_BONUS: Record<string, number> = {
            "file-ref": 20,
            "applies-to": 10,
            "dependency": 3,
            "semantic": 5,
            "connected": 2,
        };

        for (const chunk of matchedChunks) {
            const rawRow = chunkRows.get(chunk.id);
            const connectionCount = connCountMap.get(chunk.id) ?? 0;
            const baseScore = rawRow ? scoreChunk(rawRow, connectionCount) : 0;
            chunk.score = baseScore + (STRATEGY_BONUS[chunk.matchReason] ?? 0);
        }

        matchedChunks.sort((a, b) => b.score - a.score);

        // 4. Find requirements linked to matched chunks
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

        return { chunks: matchedChunks, requirements };
    });
}
