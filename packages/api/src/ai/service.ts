import { generateText } from "ai";
import { openai } from "@ai-sdk/openai";
import { getAllChunksMeta, getChunkById } from "@fubbik/db/repository";
import { Effect } from "effect";
import { DatabaseError } from "@fubbik/db/errors";
import { NotFoundError } from "../errors";

export function summarizeChunkById(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(chunk => chunk ? Effect.succeed(chunk) : Effect.fail(new NotFoundError({ resource: "Chunk" }))),
        Effect.flatMap(chunk => summarizeChunk(chunk.title, chunk.content))
    );
}

export function suggestConnectionsForChunk(chunkId: string, userId: string) {
    return Effect.all({
        chunk: getChunkById(chunkId, userId).pipe(
            Effect.flatMap(c => c ? Effect.succeed(c) : Effect.fail(new NotFoundError({ resource: "Chunk" })))
        ),
        allChunks: getAllChunksMeta(userId)
    }).pipe(
        Effect.flatMap(({ chunk, allChunks }) => {
            const others = allChunks.filter((c: { id: string }) => c.id !== chunk.id).map((c: { id: string; title: string }) => ({ id: c.id, title: c.title }));
            return suggestConnections(chunk.title, chunk.content, others);
        })
    );
}

function summarizeChunk(title: string, content: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await generateText({
                model: openai("gpt-4o-mini"),
                prompt: `Summarize this knowledge chunk in 2-3 sentences:\n\nTitle: ${title}\n\nContent: ${content}`,
                maxOutputTokens: 200
            });
            return { summary: result.text };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function suggestConnections(
    chunkTitle: string,
    chunkContent: string,
    otherChunks: { id: string; title: string }[]
) {
    return Effect.tryPromise({
        try: async () => {
            const chunkList = otherChunks.map(c => `- ${c.id}: ${c.title}`).join("\n");
            const result = await generateText({
                model: openai("gpt-4o-mini"),
                prompt: `Given this chunk:\nTitle: ${chunkTitle}\nContent: ${chunkContent}\n\nSuggest which of these chunks it should be connected to and why. Return a JSON array of objects with "id" and "relation" fields only:\n${chunkList}`,
                maxOutputTokens: 500
            });
            try {
                return JSON.parse(result.text) as { id: string; relation: string }[];
            } catch {
                return [];
            }
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function generateChunk(prompt: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await generateText({
                model: openai("gpt-4o-mini"),
                prompt: `Generate a knowledge chunk based on this prompt. Return valid JSON only with these fields: title (string), content (string), type (one of: note, document, reference, schema, checklist), tags (array of strings):\n\n${prompt}`,
                maxOutputTokens: 1000
            });
            try {
                return JSON.parse(result.text) as {
                    title: string;
                    content: string;
                    type: string;
                    tags: string[];
                };
            } catch {
                return { title: prompt, content: result.text, type: "note" as const, tags: [] as string[] };
            }
        },
        catch: cause => new DatabaseError({ cause })
    });
}
