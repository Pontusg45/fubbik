import { createStaleFlag, getDownstreamChunks, getUpstreamChunks, getStaleFlags } from "@fubbik/db/repository";
import { Effect } from "effect";

export function flagDownstreamStale(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.gen(function* () {
        const downstreamIds = yield* getDownstreamChunks(updatedChunkId, 3);
        if (downstreamIds.length === 0) return { flagged: 0 };

        const existingFlags = yield* getStaleFlags(userId, { reason: "upstream_changed" });
        const alreadyFlagged = new Set(existingFlags.map(f => f.chunkId));

        let flagged = 0;
        for (const chunkId of downstreamIds) {
            if (alreadyFlagged.has(chunkId)) continue;
            yield* createStaleFlag({
                id: crypto.randomUUID(),
                chunkId,
                reason: "upstream_changed",
                detail: `Upstream chunk "${updatedChunkTitle}" (${updatedChunkId}) was updated`,
                relatedChunkId: updatedChunkId
            });
            flagged++;
        }

        return { flagged };
    });
}

export function flagUpstreamStale(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.gen(function* () {
        const upstreamIds = yield* getUpstreamChunks(updatedChunkId, 3);
        if (upstreamIds.length === 0) return { flagged: 0 };

        const existingFlags = yield* getStaleFlags(userId, { reason: "downstream_changed" });
        const alreadyFlagged = new Set(existingFlags.map(f => f.chunkId));

        let flagged = 0;
        for (const chunkId of upstreamIds) {
            if (alreadyFlagged.has(chunkId)) continue;
            yield* createStaleFlag({
                id: crypto.randomUUID(),
                chunkId,
                reason: "downstream_changed",
                detail: `Downstream chunk "${updatedChunkTitle}" (${updatedChunkId}) was updated`,
                relatedChunkId: updatedChunkId
            });
            flagged++;
        }

        return { flagged };
    });
}

export function flagBidirectionalImpact(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.all({
        downstream: flagDownstreamStale(updatedChunkId, updatedChunkTitle, userId),
        upstream: flagUpstreamStale(updatedChunkId, updatedChunkTitle, userId)
    }).pipe(
        Effect.map(({ downstream, upstream }) => ({
            flagged: downstream.flagged + upstream.flagged
        }))
    );
}
