import { computeImpactRipple } from "@fubbik/db/age/impact";
import { createStaleFlag, getStaleFlags } from "@fubbik/db/repository";
import { Effect } from "effect";

export function flagImpactRipple(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.gen(function* () {
        const targets = yield* computeImpactRipple(updatedChunkId);
        if (targets.length === 0) return { flagged: 0 };

        const existingFlags = yield* getStaleFlags(userId, { reason: "upstream_impact" });
        const alreadyFlagged = new Map(existingFlags.filter(f => f.relatedChunkId === updatedChunkId).map(f => [f.chunkId, f]));

        let flagged = 0;
        for (const target of targets) {
            if (alreadyFlagged.has(target.chunkId)) continue;
            yield* createStaleFlag({
                id: crypto.randomUUID(),
                chunkId: target.chunkId,
                reason: "upstream_impact",
                detail: `Impacted by change to "${updatedChunkTitle}" (degree: ${target.degree.toFixed(2)}, ${target.hops} hops via ${target.path.join(" → ")})`,
                relatedChunkId: updatedChunkId
            });
            flagged++;
        }

        return { flagged };
    });
}

// Keep backward compatibility — flagBidirectionalImpact is called from chunk-mutations.ts
export function flagBidirectionalImpact(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return flagImpactRipple(updatedChunkId, updatedChunkTitle, userId);
}
