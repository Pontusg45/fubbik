import { getAppliesToForChunk, getChunkById, getRequirementsForChunks, listChunks, listCodebases, lookupChunksByFilePath } from "@fubbik/db/repository";
import { Effect } from "effect";

import { globMatch } from "./glob-match";

export interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to" | "dependency";
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

        // 1. Direct file-ref matches
        const fileRefMatches = yield* lookupChunksByFilePath(filePath, userId);
        for (const match of fileRefMatches) {
            if (results.has(match.chunkId)) continue;
            const full = yield* getChunkById(match.chunkId, userId);
            if (!full) continue;
            results.set(match.chunkId, {
                id: full.id,
                title: full.title,
                type: full.type,
                content: full.content,
                summary: full.summary,
                matchReason: "file-ref"
            });
        }

        // 2. Applies-to glob pattern matches
        const { chunks } = yield* listChunks({
            userId,
            codebaseId,
            limit: 1000,
            offset: 0
        });

        for (const c of chunks) {
            if (results.has(c.id)) continue;
            const patterns = yield* getAppliesToForChunk(c.id);
            if (patterns.length === 0) continue;

            const matches = patterns.some(p => globMatch(p.pattern, filePath));
            if (matches) {
                results.set(c.id, {
                    id: c.id,
                    title: c.title,
                    type: c.type,
                    content: c.content,
                    summary: c.summary,
                    matchReason: "applies-to"
                });
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
                    results.set(c.id, {
                        id: c.id,
                        title: c.title,
                        type: c.type,
                        content: c.content,
                        summary: c.summary,
                        matchReason: "dependency"
                    });
                }
            }
        }

        const matchedChunks = Array.from(results.values());

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
