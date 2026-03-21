import { Effect } from "effect";

import { generateDocumentEmbedding, isOllamaAvailable } from "../ollama/client";
import { findSimilarByEmbedding } from "@fubbik/db/repository";

export function checkSimilar(params: {
    title: string;
    content: string;
    userId: string;
    excludeId?: string;
}) {
    return Effect.gen(function* () {
        const available = yield* isOllamaAvailable();
        if (!available) return [];
        const embedding = yield* generateDocumentEmbedding(params.title, null, params.content);
        return yield* findSimilarByEmbedding({
            embedding,
            userId: params.userId,
            excludeId: params.excludeId,
            threshold: 0.75,
            limit: 3,
        });
    });
}
