import { eq, inArray } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunkAppliesTo } from "../schema/applies-to";

export function getAppliesToForChunk(chunkId: string) {
    return dbEffect(() =>
            db
                .select({
                    id: chunkAppliesTo.id,
                    pattern: chunkAppliesTo.pattern,
                    note: chunkAppliesTo.note
                })
                .from(chunkAppliesTo)
                .where(eq(chunkAppliesTo.chunkId, chunkId)));
}

export function getAppliesToForChunks(chunkIds: string[]) {
    return dbEffect(() =>
            db
                .select()
                .from(chunkAppliesTo)
                .where(inArray(chunkAppliesTo.chunkId, chunkIds)));
}

export function setAppliesToForChunk(chunkId: string, patterns: { pattern: string; note?: string | null }[]) {
    return dbEffect(async () => {
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
        });
}
