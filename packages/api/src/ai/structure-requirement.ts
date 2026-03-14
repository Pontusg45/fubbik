import { listVocabulary } from "@fubbik/db/repository";
import { Effect } from "effect";

import { AiError } from "../errors";
import { generateJson, isOllamaAvailable } from "../ollama/client";

interface Step {
    keyword: "given" | "when" | "then" | "and";
    text: string;
}

interface StructureResult {
    steps: Step[];
}

export function structureRequirement(description: string, codebaseId?: string) {
    return isOllamaAvailable().pipe(
        Effect.flatMap(available => {
            if (!available) {
                return Effect.fail(new AiError({ cause: "Ollama is not available. Please ensure Ollama is running." }));
            }
            return Effect.succeed(undefined);
        }),
        Effect.flatMap(() => {
            if (codebaseId) {
                return listVocabulary(codebaseId).pipe(
                    Effect.map(entries => {
                        const byCategory = new Map<string, string[]>();
                        for (const entry of entries) {
                            const list = byCategory.get(entry.category) ?? [];
                            list.push(entry.word);
                            byCategory.set(entry.category, list);
                        }
                        const lines: string[] = [];
                        for (const [category, words] of byCategory) {
                            lines.push(`${category}: ${words.join(", ")}`);
                        }
                        return lines.length > 0 ? lines.join("\n") : "";
                    }),
                    Effect.catchAll(() => Effect.succeed(""))
                );
            }
            return Effect.succeed("");
        }),
        Effect.flatMap(vocabularyContext => {
            const vocabSection = vocabularyContext
                ? `\nUse this vocabulary if possible:\n${vocabularyContext}\n`
                : "";

            const prompt = `Convert this requirement description into structured Given/When/Then steps.
${vocabSection}
Description: ${description}

Return as JSON: { "steps": [{ "keyword": "given"|"when"|"then"|"and", "text": "..." }] }

Rules:
- Start with "given" steps for preconditions
- Then "when" steps for the action
- Then "then" steps for expected outcomes
- Use "and" for additional steps within a phase
- Keep step text concise and clear
- Do not include the keyword in the text field`;

            return generateJson<StructureResult>(prompt);
        }),
        Effect.flatMap(result => {
            if (!result.steps || !Array.isArray(result.steps)) {
                return Effect.fail(new AiError({ cause: "Invalid response from AI" }));
            }
            const validKeywords = new Set(["given", "when", "then", "and"]);
            const validSteps = result.steps.filter(
                (s): s is Step =>
                    typeof s === "object" &&
                    s !== null &&
                    typeof s.keyword === "string" &&
                    validKeywords.has(s.keyword) &&
                    typeof s.text === "string" &&
                    s.text.trim().length > 0
            );
            if (validSteps.length === 0) {
                return Effect.fail(new AiError({ cause: "AI did not produce valid steps" }));
            }
            return Effect.succeed({ steps: validSteps });
        })
    );
}
