import { and, desc, eq, isNull, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { chunkStaleness, stalenessScan } from "../schema/staleness";

function codebaseConditions(codebaseId?: string) {
    if (!codebaseId) return [];
    const inCodebase = db
        .select({ chunkId: chunkCodebase.chunkId })
        .from(chunkCodebase)
        .where(eq(chunkCodebase.codebaseId, codebaseId));
    const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
    return [sql`(${chunk.id} IN (${inCodebase}) OR ${chunk.id} NOT IN (${inAnyCodebase}))`];
}

const undismissedUnsuppressed = [isNull(chunkStaleness.dismissedAt), isNull(chunkStaleness.suppressPair)];

export function getStaleFlags(
    userId: string,
    params?: { reason?: string; codebaseId?: string; limit?: number }
) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                ...undismissedUnsuppressed,
                ...(params?.reason ? [eq(chunkStaleness.reason, params.reason)] : []),
                ...codebaseConditions(params?.codebaseId)
            ];

            return db
                .select({
                    id: chunkStaleness.id,
                    chunkId: chunkStaleness.chunkId,
                    reason: chunkStaleness.reason,
                    detail: chunkStaleness.detail,
                    relatedChunkId: chunkStaleness.relatedChunkId,
                    detectedAt: chunkStaleness.detectedAt,
                    chunkTitle: chunk.title,
                    chunkType: chunk.type
                })
                .from(chunkStaleness)
                .innerJoin(chunk, eq(chunkStaleness.chunkId, chunk.id))
                .where(and(...conditions))
                .orderBy(desc(chunkStaleness.detectedAt))
                .limit(params?.limit ?? 50);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getStaleCount(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                ...undismissedUnsuppressed,
                ...codebaseConditions(codebaseId)
            ];

            const result = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkStaleness)
                .innerJoin(chunk, eq(chunkStaleness.chunkId, chunk.id))
                .where(and(...conditions));

            return Number(result[0]?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getStaleFlagsForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkStaleness.id,
                    reason: chunkStaleness.reason,
                    detail: chunkStaleness.detail,
                    relatedChunkId: chunkStaleness.relatedChunkId,
                    detectedAt: chunkStaleness.detectedAt
                })
                .from(chunkStaleness)
                .where(
                    and(eq(chunkStaleness.chunkId, chunkId), ...undismissedUnsuppressed)
                ),
        catch: cause => new DatabaseError({ cause })
    });
}

export function createStaleFlag(data: {
    id: string;
    chunkId: string;
    reason: string;
    detail?: string;
    relatedChunkId?: string;
}) {
    return Effect.tryPromise({
        try: () =>
            db
                .insert(chunkStaleness)
                .values({
                    id: data.id,
                    chunkId: data.chunkId,
                    reason: data.reason,
                    detail: data.detail ?? null,
                    relatedChunkId: data.relatedChunkId ?? null
                })
                .onConflictDoNothing(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function dismissStaleFlag(flagId: string, userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .update(chunkStaleness)
                .set({ dismissedAt: new Date(), dismissedBy: userId })
                .where(eq(chunkStaleness.id, flagId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function suppressDuplicatePair(chunkIdA: string, chunkIdB: string) {
    const pairKey = [chunkIdA, chunkIdB].sort().join(":");
    return Effect.tryPromise({
        try: () =>
            db
                .update(chunkStaleness)
                .set({ suppressPair: pairKey })
                .where(
                    and(
                        eq(chunkStaleness.reason, "diverged_duplicate"),
                        isNull(chunkStaleness.dismissedAt),
                        sql`(${chunkStaleness.chunkId} IN (${chunkIdA}, ${chunkIdB}) OR ${chunkStaleness.relatedChunkId} IN (${chunkIdA}, ${chunkIdB}))`
                    )
                ),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getLastScan(codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const rows = await db
                .select()
                .from(stalenessScan)
                .where(eq(stalenessScan.codebaseId, codebaseId))
                .orderBy(desc(stalenessScan.scannedAt))
                .limit(1);
            return rows[0] ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function upsertScan(data: { id: string; codebaseId: string; lastCommitSha: string }) {
    return Effect.tryPromise({
        try: () =>
            db
                .insert(stalenessScan)
                .values({
                    id: data.id,
                    codebaseId: data.codebaseId,
                    lastCommitSha: data.lastCommitSha
                })
                .onConflictDoUpdate({
                    target: stalenessScan.id,
                    set: {
                        lastCommitSha: data.lastCommitSha,
                        scannedAt: new Date()
                    }
                }),
        catch: cause => new DatabaseError({ cause })
    });
}
