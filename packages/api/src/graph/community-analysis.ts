import { findDuplicatePairs } from "@fubbik/db/repository";
import type { Community } from "@fubbik/db/repository";
import { Effect } from "effect";

export interface CommunityRedundancy {
    communityId: string;
    memberCount: number;
    avgPairwiseSimilarity: number;
    maxPairwiseSimilarity: number;
    redundancyLevel: "low" | "medium" | "high";
    mergeCandidates: Array<{ idA: string; idB: string; similarity: number }>;
}

export function computeCommunityRedundancy(communities: Community[]) {
    return Effect.gen(function* () {
        const results: CommunityRedundancy[] = [];

        for (const community of communities) {
            if (community.members.length < 2) continue;

            const pairs = yield* findDuplicatePairs({
                chunkIds: community.members,
                threshold: 0.5,
                limit: 50,
            }).pipe(Effect.catchAll(() => Effect.succeed([])));

            if (pairs.length === 0) {
                results.push({
                    communityId: community.id,
                    memberCount: community.members.length,
                    avgPairwiseSimilarity: 0,
                    maxPairwiseSimilarity: 0,
                    redundancyLevel: "low",
                    mergeCandidates: [],
                });
                continue;
            }

            const avgSimilarity =
                pairs.reduce((sum, p) => sum + p.similarity, 0) / pairs.length;
            const maxSimilarity = Math.max(...pairs.map((p) => p.similarity));
            const redundancyLevel =
                avgSimilarity > 0.8
                    ? "high"
                    : avgSimilarity > 0.6
                      ? "medium"
                      : "low";
            const mergeCandidates = pairs
                .filter((p) => p.similarity > 0.8)
                .map((p) => ({
                    idA: p.idA,
                    idB: p.idB,
                    similarity: p.similarity,
                }));

            results.push({
                communityId: community.id,
                memberCount: community.members.length,
                avgPairwiseSimilarity: avgSimilarity,
                maxPairwiseSimilarity: maxSimilarity,
                redundancyLevel,
                mergeCandidates,
            });
        }

        return results.sort(
            (a, b) => b.avgPairwiseSimilarity - a.avgPairwiseSimilarity
        );
    });
}
