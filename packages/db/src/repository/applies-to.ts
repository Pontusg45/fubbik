import { eq, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkAppliesTo } from "../schema/applies-to";

export function getAppliesToForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkAppliesTo.id,
                    pattern: chunkAppliesTo.pattern,
                    note: chunkAppliesTo.note
                })
                .from(chunkAppliesTo)
                .where(eq(chunkAppliesTo.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getAppliesToForChunks(chunkIds: string[]) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkAppliesTo)
                .where(inArray(chunkAppliesTo.chunkId, chunkIds)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function setAppliesToForChunk(chunkId: string, patterns: { pattern: string; note?: string | null }[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkAppliesTo).where(eq(chunkAppliesTo.chunkId, chunkId));
            if (patterns.length === 0) return [];
            return db
                .insert(chunkAppliesTo)
                .values(
                    patterns.map(p => ({
                        id: crypto.randomUUID(),
                        chunkId,
                        pattern: p.pattern,
                        note: p.note ?? null
                    }))
                )
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}
