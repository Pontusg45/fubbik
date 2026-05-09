import { getNeighborhood, getTagsForChunks, getTagsForChunk } from "@fubbik/db/repository";
import { Effect } from "effect";

export interface TagSuggestion {
    tagId: string;
    tagName: string;
    frequency: number;
    neighborCount: number;
}

export function suggestTagsFromGraph(chunkId: string, maxHops = 1, minFrequency = 0.5) {
    return Effect.gen(function* () {
        const neighborIds = yield* getNeighborhood(chunkId, maxHops).pipe(
            Effect.catchAll(() => Effect.succeed([] as string[]))
        );
        if (neighborIds.length === 0) return [] as TagSuggestion[];

        const existingTags = yield* getTagsForChunk(chunkId).pipe(
            Effect.catchAll(() => Effect.succeed([]))
        );
        const existingTagIds = new Set(existingTags.map(t => t.id));

        const neighborTags = yield* getTagsForChunks(neighborIds).pipe(
            Effect.catchAll(() => Effect.succeed([]))
        );

        const tagCounts = new Map<string, { tagName: string; count: number }>();
        for (const nt of neighborTags) {
            if (existingTagIds.has(nt.tagId)) continue;
            const existing = tagCounts.get(nt.tagId);
            if (existing) {
                existing.count++;
            } else {
                tagCounts.set(nt.tagId, { tagName: nt.tagName, count: 1 });
            }
        }

        const suggestions: TagSuggestion[] = [];
        for (const [tagId, { tagName, count }] of tagCounts) {
            const frequency = count / neighborIds.length;
            if (frequency >= minFrequency) {
                suggestions.push({ tagId, tagName, frequency, neighborCount: neighborIds.length });
            }
        }

        return suggestions.sort((a, b) => b.frequency - a.frequency);
    });
}
