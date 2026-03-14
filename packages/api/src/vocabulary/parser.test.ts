import { describe, expect, it } from "vitest";

import { parseStepText, type VocabEntry } from "./parser";

const vocab: VocabEntry[] = [
    { word: "click", category: "action", expects: ["target"] },
    { word: "type", category: "action", expects: ["target"] },
    { word: "logged in", category: "state", expects: null },
    { word: "log", category: "action", expects: ["target"] },
    { word: "button", category: "target", expects: null },
    { word: "field", category: "target", expects: null },
    { word: "submit", category: "target", expects: null },
    { word: "the", category: "modifier", expects: null },
    { word: "is", category: "connector", expects: ["state"] }
];

describe("parseStepText", () => {
    it("parses a valid step with no warnings", () => {
        const result = parseStepText("click the button", vocab);
        expect(result.warnings).toHaveLength(0);
        expect(result.tokens).toHaveLength(3);
        expect(result.tokens[0]).toMatchObject({ text: "click", category: "action" });
        expect(result.tokens[1]).toMatchObject({ text: "the", category: "modifier" });
        expect(result.tokens[2]).toMatchObject({ text: "button", category: "target" });
    });

    it("matches multi-word entries greedily", () => {
        const result = parseStepText("user is logged in", vocab);
        // "logged in" should be matched as one token, not "log" + unknown
        const loggedInToken = result.tokens.find(t => t.text.toLowerCase() === "logged in");
        expect(loggedInToken).toBeDefined();
        expect(loggedInToken!.category).toBe("state");

        // "log" should NOT appear as a separate token
        const logToken = result.tokens.find(t => t.text.toLowerCase() === "log");
        expect(logToken).toBeUndefined();
    });

    it("produces warnings for unknown words", () => {
        const result = parseStepText("click the foobar button", vocab);
        const unknownWarnings = result.warnings.filter(w => w.type === "unknown_word");
        expect(unknownWarnings).toHaveLength(1);
        expect(unknownWarnings[0].word).toBe("foobar");
    });

    it("produces warning for unexpected category after action", () => {
        // "click" expects "target", but "click" is followed by another action "type"
        const result = parseStepText("click type", vocab);
        const catWarning = result.warnings.find(w => w.type === "unexpected_category");
        expect(catWarning).toBeDefined();
        expect(catWarning!.word).toBe("type");
    });

    it("produces warning for dangling expects at end of step", () => {
        // "click" expects "target" but step ends
        const result = parseStepText("click", vocab);
        const danglingWarning = result.warnings.find(w => w.type === "expects_not_satisfied");
        expect(danglingWarning).toBeDefined();
        expect(danglingWarning!.word).toBe("click");
    });

    it("does not produce unknown warnings for quoted literals", () => {
        const result = parseStepText('type "hello world" field', vocab);
        expect(result.warnings).toHaveLength(0);
        const literalToken = result.tokens.find(t => t.text === '"hello world"');
        expect(literalToken).toBeDefined();
        expect(literalToken!.category).toBe("literal");
    });

    it("matches case insensitively", () => {
        const result = parseStepText("Click THE Button", vocab);
        expect(result.warnings).toHaveLength(0);
        expect(result.tokens[0]).toMatchObject({ text: "Click", category: "action" });
        expect(result.tokens[2]).toMatchObject({ text: "Button", category: "target" });
    });

    it("returns empty result for empty text", () => {
        const result = parseStepText("", vocab);
        expect(result.tokens).toHaveLength(0);
        expect(result.warnings).toHaveLength(0);
    });
});
