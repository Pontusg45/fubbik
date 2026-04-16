import { and, eq, inArray, isNull } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { chunkAppliesTo } from "../schema/applies-to";
import { chunkFileRef } from "../schema/file-ref";

export interface DensityPath {
    path: string;
    chunkId: string;
    chunkTitle: string;
    chunkType: string;
    source: "applies_to" | "file_ref";
}

export function fetchDensityPaths(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async (): Promise<DensityPath[]> => {
            const chunkFilter = [eq(chunk.userId, userId), isNull(chunk.archivedAt)];
            let chunkIds: string[];
            if (codebaseId) {
                const rows = await db
                    .select({ id: chunk.id })
                    .from(chunk)
                    .innerJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                    .where(and(...chunkFilter, eq(chunkCodebase.codebaseId, codebaseId)));
                chunkIds = rows.map(r => r.id);
            } else {
                const rows = await db
                    .select({ id: chunk.id })
                    .from(chunk)
                    .where(and(...chunkFilter));
                chunkIds = rows.map(r => r.id);
            }

            if (chunkIds.length === 0) return [];

            const [appliesRows, fileRefRows] = await Promise.all([
                db
                    .select({
                        chunkId: chunkAppliesTo.chunkId,
                        pattern: chunkAppliesTo.pattern,
                        title: chunk.title,
                        type: chunk.type
                    })
                    .from(chunkAppliesTo)
                    .innerJoin(chunk, eq(chunk.id, chunkAppliesTo.chunkId))
                    .where(inArray(chunkAppliesTo.chunkId, chunkIds)),
                db
                    .select({
                        chunkId: chunkFileRef.chunkId,
                        path: chunkFileRef.path,
                        title: chunk.title,
                        type: chunk.type
                    })
                    .from(chunkFileRef)
                    .innerJoin(chunk, eq(chunk.id, chunkFileRef.chunkId))
                    .where(inArray(chunkFileRef.chunkId, chunkIds))
            ]);

            const results: DensityPath[] = [];
            for (const r of appliesRows) {
                const prefix = globPrefix(r.pattern);
                if (!prefix) continue;
                results.push({
                    path: prefix,
                    chunkId: r.chunkId,
                    chunkTitle: r.title,
                    chunkType: r.type,
                    source: "applies_to"
                });
            }
            for (const r of fileRefRows) {
                results.push({
                    path: r.path,
                    chunkId: r.chunkId,
                    chunkTitle: r.title,
                    chunkType: r.type,
                    source: "file_ref"
                });
            }
            return results;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

function globPrefix(pattern: string): string | null {
    const globStart = pattern.search(/[*?[{]/);
    const prefix = globStart === -1 ? pattern : pattern.slice(0, globStart);
    const trimmed = prefix.replace(/\/+$/, "");
    return trimmed || null;
}
