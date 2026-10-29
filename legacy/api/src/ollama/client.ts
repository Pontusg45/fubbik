import { env } from "@fubbik/env/server";
import { Effect } from "effect";

import { AiError } from "../errors";

const OLLAMA_URL = env.OLLAMA_URL ?? "http://localhost:11434";
const EMBED_MODEL = "nomic-embed-text";

interface OllamaGenerateResponse {
    response: string;
}

interface OllamaEmbeddingResponse {
    embedding: number[];
}

export function isOllamaAvailable(): Effect.Effect<boolean, never> {
    return Effect.tryPromise({
        try: async () => {
            const res = await fetch(`${OLLAMA_URL}/api/tags`, { signal: AbortSignal.timeout(2000) });
            return res.ok;
        },
        catch: () => false
    }).pipe(Effect.catchAll(() => Effect.succeed(false)));
}

export function generateJson<T>(prompt: string, model?: string): Effect.Effect<T, AiError> {
    return Effect.tryPromise({
        try: async () => {
            const res = await fetch(`${OLLAMA_URL}/api/generate`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    model: model ?? "llama3.2",
                    prompt,
                    format: "json",
                    stream: false
                })
            });
            if (!res.ok) throw new Error(`Ollama generate failed: ${res.status}`);
            const data = (await res.json()) as OllamaGenerateResponse;
            return JSON.parse(data.response) as T;
        },
        catch: cause => new AiError({ cause })
    });
}

export function generateEmbedding(text: string): Effect.Effect<number[], AiError> {
    return Effect.tryPromise({
        try: async () => {
            const res = await fetch(`${OLLAMA_URL}/api/embeddings`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    model: EMBED_MODEL,
                    prompt: text
                })
            });
            if (!res.ok) throw new Error(`Ollama embedding failed: ${res.status}`);
            const data = (await res.json()) as OllamaEmbeddingResponse;
            return data.embedding;
        },
        catch: cause => new AiError({ cause })
    });
}

export function generateQueryEmbedding(query: string): Effect.Effect<number[], AiError> {
    return generateEmbedding(`search_query: ${query}`);
}

export function generateDocumentEmbedding(title: string, summary: string | null, content: string): Effect.Effect<number[], AiError> {
    const text = `search_document: ${title}\n${summary ?? ""}\n${content}`.trim();
    return generateEmbedding(text);
}
