import { env } from "@fubbik/env/server";
import { Effect } from "effect";

const OLLAMA_URL = env.OLLAMA_URL ?? "http://localhost:11434";

const VALID_CATEGORIES = new Set(["actor", "action", "target", "outcome", "state", "modifier"]);

export interface SuggestedEntry {
    word: string;
    category: string;
    expects?: string[];
}

interface OllamaGenerateResponse {
    response: string;
}

const PROMPT_TEMPLATE = `You are analyzing code documentation chunks to extract a controlled vocabulary for behavior-driven requirements (BDD/Gherkin style).

Given the following chunks of documentation/code, extract meaningful vocabulary entries. Each entry has:
- "word": a short word or phrase (1-3 words, lowercase)
- "category": one of "actor", "action", "target", "outcome", "state", "modifier"
- "expects" (optional): array of category names that should follow this word

Categories:
- actor: who performs the action (e.g., "user", "admin", "system")
- action: what is done (e.g., "click", "submit", "navigate")
- target: what the action is performed on (e.g., "button", "form", "page")
- outcome: what should result (e.g., "displayed", "saved", "redirected")
- state: a condition (e.g., "logged in", "visible", "enabled")
- modifier: connecting/clarifying words (e.g., "the", "a", "should")

Actions typically expect ["target"]. Actors typically expect ["action"].

Return a JSON array of objects. Only return the JSON array, no other text.

CHUNKS:
`;

export function suggestVocabulary(
    chunks: Array<{ title: string; content: string }>,
    ollamaUrl?: string
): Effect.Effect<SuggestedEntry[], never> {
    return Effect.tryPromise({
        try: async () => {
            const url = ollamaUrl ?? OLLAMA_URL;

            // Truncate chunk content to fit context (~8000 chars total)
            let totalChars = 0;
            const truncatedChunks: string[] = [];
            for (const chunk of chunks) {
                const entry = `### ${chunk.title}\n${chunk.content}`;
                if (totalChars + entry.length > 8000) {
                    const remaining = 8000 - totalChars;
                    if (remaining > 100) {
                        truncatedChunks.push(entry.substring(0, remaining));
                    }
                    break;
                }
                truncatedChunks.push(entry);
                totalChars += entry.length;
            }

            const prompt = PROMPT_TEMPLATE + truncatedChunks.join("\n\n");

            const res = await fetch(`${url}/api/generate`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    model: "llama3.2",
                    prompt,
                    stream: false
                })
            });

            if (!res.ok) return [];

            const data = (await res.json()) as OllamaGenerateResponse;
            const responseText = data.response.trim();

            // Try to extract JSON array from response
            const jsonMatch = responseText.match(/\[[\s\S]*\]/);
            if (!jsonMatch) return [];

            const parsed = JSON.parse(jsonMatch[0]) as unknown[];
            if (!Array.isArray(parsed)) return [];

            // Validate and clean entries
            const entries: SuggestedEntry[] = [];
            for (const item of parsed) {
                if (typeof item !== "object" || item === null) continue;
                const obj = item as Record<string, unknown>;
                if (typeof obj.word !== "string" || typeof obj.category !== "string") continue;

                const word = obj.word.trim().toLowerCase();
                const category = obj.category.trim().toLowerCase();

                if (!word || !VALID_CATEGORIES.has(category)) continue;

                const entry: SuggestedEntry = { word, category };
                if (Array.isArray(obj.expects)) {
                    const validExpects = (obj.expects as unknown[])
                        .filter((e): e is string => typeof e === "string" && VALID_CATEGORIES.has(e.toLowerCase()))
                        .map(e => e.toLowerCase());
                    if (validExpects.length > 0) {
                        entry.expects = validExpects;
                    }
                }

                entries.push(entry);
            }

            return entries;
        },
        catch: () => [] as SuggestedEntry[]
    }).pipe(Effect.catchAll(() => Effect.succeed([] as SuggestedEntry[])));
}
