import { getChunkById, updateChunkEnrichment } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";
import { generateDocumentEmbedding, generateJson, isOllamaAvailable } from "../ollama/client";

interface EnrichmentResult {
    summary: string;
    aliases: string[];
    notAbout: string[];
}

export function enrichChunk(chunkId: string) {
    return isOllamaAvailable().pipe(
        Effect.flatMap(available => {
            if (!available) return Effect.succeed(null);

            return getChunkById(chunkId).pipe(
                Effect.flatMap(c => (c ? Effect.succeed(c) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
                Effect.flatMap(c =>
                    Effect.all({
                        metadata: generateJson<EnrichmentResult>(
                            `Analyze this knowledge chunk and return JSON with these fields:
- "summary": a 1-2 sentence TL;DR of the content
- "aliases": an array of 3-8 alternative names, abbreviations, or search terms someone might use to find this
- "notAbout": an array of 2-5 terms this chunk could be confused with but is NOT about

Title: ${c.title}
Type: ${c.type}
Tags:

Content:
${c.content}`
                        ),
                        embedding: generateDocumentEmbedding(c.title, c.summary, c.content)
                    })
                ),
                Effect.flatMap(({ metadata, embedding }) =>
                    updateChunkEnrichment(chunkId, {
                        summary: metadata.summary,
                        aliases: metadata.aliases,
                        notAbout: metadata.notAbout,
                        embedding
                    })
                )
            );
        })
    );
}

export function enrichChunkIfEmpty(chunkId: string) {
    return getChunkById(chunkId).pipe(
        Effect.flatMap(c => {
            if (!c) return Effect.succeed(null);
            if (c.summary) return Effect.succeed(c);
            return enrichChunk(chunkId);
        })
    );
}
