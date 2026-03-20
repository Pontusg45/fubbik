import { getAppliesToForChunk, getChunkById, listChunks, lookupChunksByFilePath } from "@fubbik/db/repository";
import { Effect } from "effect";

import { globMatch } from "./glob-match";

export interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to";
}

export function getContextForFile(
    userId: string,
    filePath: string,
    codebaseId?: string
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

        return Array.from(results.values());
    });
}
