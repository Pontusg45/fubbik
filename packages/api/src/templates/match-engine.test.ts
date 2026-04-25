import { describe, it, expect } from "vitest";
import { matchHeading, scoreTemplate, matchTemplates } from "./match-engine";
import type {
    MatchRules,
    ParsedHeading,
    TemplateWithRules,
} from "./types";

// Helper to build a TemplateWithRules with sensible defaults
function makeTemplate(
    overrides: Partial<TemplateWithRules> & { id: string; name: string },
): TemplateWithRules {
    return {
        id: overrides.id,
        name: overrides.name,
        type: overrides.type ?? "note",
        matchRules: overrides.matchRules ?? {
            minScore: 1,
            headings: [],
            frontmatter: [],
        },
        fieldMappings: overrides.fieldMappings ?? null,
        priority: overrides.priority ?? 0,
        tags: overrides.tags ?? null,
    };
}

// Helper to build a ParsedHeading
function h(text: string, level = 2): ParsedHeading {
    return { text, level };
}

// ---------------------------------------------------------------------------
// matchHeading
// ---------------------------------------------------------------------------

describe("matchHeading", () => {
    describe("exact mode", () => {
        it("matches when heading text equals pattern (same case)", () => {
            expect(matchHeading("Decision", h("Decision"), "exact")).toBe(true);
        });

        it("matches case-insensitively", () => {
            expect(matchHeading("Decision", h("decision"), "exact")).toBe(true);
            expect(matchHeading("decision", h("Decision"), "exact")).toBe(true);
        });

        it("does NOT match when heading has extra text", () => {
            expect(matchHeading("Decision", h("Decision Record"), "exact")).toBe(false);
        });

        it("does NOT match an unrelated heading", () => {
            expect(matchHeading("Decision", h("Context"), "exact")).toBe(false);
        });
    });

    describe("prefix mode", () => {
        it("matches when heading starts with the pattern", () => {
            expect(matchHeading("Decision", h("Decision: Use Postgres"), "prefix")).toBe(true);
        });

        it("matches exact heading text as a prefix", () => {
            expect(matchHeading("Decision", h("Decision"), "prefix")).toBe(true);
        });

        it("does NOT match when pattern appears mid-string but not at start", () => {
            expect(matchHeading("decision", h("Indecision"), "prefix")).toBe(false);
        });

        it("is case-insensitive", () => {
            expect(matchHeading("decision", h("Decision: Use Postgres"), "prefix")).toBe(true);
        });
    });

    describe("contains mode", () => {
        it("matches when pattern appears anywhere in the heading", () => {
            expect(matchHeading("decision", h("Our Final Decision"), "contains")).toBe(true);
        });

        it("matches at the start", () => {
            expect(matchHeading("decision", h("Decision Record"), "contains")).toBe(true);
        });

        it("matches in the middle", () => {
            expect(matchHeading("final", h("Our Final Decision"), "contains")).toBe(true);
        });

        it("is case-insensitive", () => {
            expect(matchHeading("DECISION", h("Our Final Decision"), "contains")).toBe(true);
        });

        it("does NOT match when pattern is absent", () => {
            expect(matchHeading("context", h("Our Final Decision"), "contains")).toBe(false);
        });
    });
});

// ---------------------------------------------------------------------------
// scoreTemplate
// ---------------------------------------------------------------------------

describe("scoreTemplate", () => {
    it("scores 2 required headings found = 2 pts, 1 optional found = 0.5 pts → total 2.5", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Context"], match: "exact", required: true },
                { patterns: ["Decision"], match: "exact", required: true },
                { patterns: ["Consequences"], match: "exact", required: false },
            ],
            frontmatter: [],
        };
        const headings: ParsedHeading[] = [
            h("Context"),
            h("Decision"),
            h("Consequences"),
        ];
        expect(scoreTemplate(rules, headings, {})).toBe(2.5);
    });

    it("returns 0 if any required heading is missing", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Context"], match: "exact", required: true },
                { patterns: ["Decision"], match: "exact", required: true },
            ],
            frontmatter: [],
        };
        // Only "Context" is present, "Decision" is absent
        const headings: ParsedHeading[] = [h("Context")];
        expect(scoreTemplate(rules, headings, {})).toBe(0);
    });

    it("scores frontmatter exact match (+1 point)", () => {
        const rules: MatchRules = {
            minScore: 0,
            headings: [],
            frontmatter: [{ key: "type", match: "exact", value: "adr" }],
        };
        expect(scoreTemplate(rules, [], { type: "adr" })).toBe(1);
    });

    it("does NOT score frontmatter exact match when value differs", () => {
        const rules: MatchRules = {
            minScore: 0,
            headings: [],
            frontmatter: [{ key: "type", match: "exact", value: "adr" }],
        };
        expect(scoreTemplate(rules, [], { type: "runbook" })).toBe(0);
    });

    it("scores frontmatter oneOf match (+1 point)", () => {
        const rules: MatchRules = {
            minScore: 0,
            headings: [],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["adr", "decision", "record"] },
            ],
        };
        expect(scoreTemplate(rules, [], { type: "decision" })).toBe(1);
    });

    it("does NOT score frontmatter oneOf when value not in list", () => {
        const rules: MatchRules = {
            minScore: 0,
            headings: [],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["adr", "decision", "record"] },
            ],
        };
        expect(scoreTemplate(rules, [], { type: "runbook" })).toBe(0);
    });

    it("scores frontmatter exists match (+1) when key is present", () => {
        const rules: MatchRules = {
            minScore: 0,
            headings: [],
            frontmatter: [{ key: "author", match: "exists" }],
        };
        expect(scoreTemplate(rules, [], { author: "alice" })).toBe(1);
    });

    it("scores 0 for frontmatter exists when key is absent", () => {
        const rules: MatchRules = {
            minScore: 0,
            headings: [],
            frontmatter: [{ key: "author", match: "exists" }],
        };
        expect(scoreTemplate(rules, [], {})).toBe(0);
    });

    it("respects heading level filter — level 3 rule does NOT match a level 2 heading", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Decision"], match: "exact", required: true, level: 3 },
            ],
            frontmatter: [],
        };
        // Heading "Decision" at level 2 should NOT satisfy a level-3 rule
        expect(scoreTemplate(rules, [h("Decision", 2)], {})).toBe(0);
    });

    it("matches when heading level matches the rule level", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Decision"], match: "exact", required: true, level: 3 },
            ],
            frontmatter: [],
        };
        expect(scoreTemplate(rules, [h("Decision", 3)], {})).toBe(1);
    });

    it("returns 0 when total score is below minScore", () => {
        const rules: MatchRules = {
            minScore: 3,
            headings: [
                { patterns: ["Context"], match: "exact", required: false },
            ],
            frontmatter: [],
        };
        // Only 0.5 score from one optional heading → below minScore 3
        expect(scoreTemplate(rules, [h("Context")], {})).toBe(0);
    });

    it("matches alternative patterns — any pattern in the list counts", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                {
                    patterns: ["Decision", "Resolution", "Outcome"],
                    match: "exact",
                    required: true,
                },
            ],
            frontmatter: [],
        };
        expect(scoreTemplate(rules, [h("Resolution")], {})).toBe(1);
        expect(scoreTemplate(rules, [h("Outcome")], {})).toBe(1);
    });
});

// ---------------------------------------------------------------------------
// matchTemplates
// ---------------------------------------------------------------------------

describe("matchTemplates", () => {
    it("returns the highest scoring template", () => {
        const lowScorer = makeTemplate({
            id: "t1",
            name: "Low",
            matchRules: {
                minScore: 0,
                headings: [{ patterns: ["Context"], match: "exact", required: false }],
                frontmatter: [],
            },
        });
        const highScorer = makeTemplate({
            id: "t2",
            name: "High",
            matchRules: {
                minScore: 0,
                headings: [
                    { patterns: ["Context"], match: "exact", required: true },
                    { patterns: ["Decision"], match: "exact", required: true },
                ],
                frontmatter: [],
            },
        });

        const doc = {
            headings: [h("Context"), h("Decision")],
            frontmatter: {},
        };

        const result = matchTemplates(doc, [lowScorer, highScorer]);
        expect(result).not.toBeNull();
        expect(result!.templateId).toBe("t2");
        expect(result!.score).toBe(2);
    });

    it("breaks ties by priority (higher priority wins)", () => {
        const lowPriority = makeTemplate({
            id: "t1",
            name: "Low Priority",
            priority: 1,
            matchRules: {
                minScore: 0,
                headings: [{ patterns: ["Context"], match: "exact", required: false }],
                frontmatter: [],
            },
        });
        const highPriority = makeTemplate({
            id: "t2",
            name: "High Priority",
            priority: 10,
            matchRules: {
                minScore: 0,
                headings: [{ patterns: ["Context"], match: "exact", required: false }],
                frontmatter: [],
            },
        });

        const doc = { headings: [h("Context")], frontmatter: {} };

        const result = matchTemplates(doc, [lowPriority, highPriority]);
        expect(result).not.toBeNull();
        expect(result!.templateId).toBe("t2");
    });

    it("returns null when no templates match", () => {
        const template = makeTemplate({
            id: "t1",
            name: "ADR",
            matchRules: {
                minScore: 1,
                headings: [{ patterns: ["Decision"], match: "exact", required: true }],
                frontmatter: [],
            },
        });
        const doc = { headings: [h("Introduction")], frontmatter: {} };
        expect(matchTemplates(doc, [template])).toBeNull();
    });

    it("returns null for an empty templates list", () => {
        const doc = { headings: [h("Context"), h("Decision")], frontmatter: {} };
        expect(matchTemplates(doc, [])).toBeNull();
    });

    it("returns the correct templateName, type, and tags in the result", () => {
        const template = makeTemplate({
            id: "adr-1",
            name: "Architecture Decision Record",
            type: "document",
            tags: ["adr", "architecture"],
            matchRules: {
                minScore: 1,
                headings: [{ patterns: ["Decision"], match: "exact", required: true }],
                frontmatter: [],
            },
        });
        const doc = { headings: [h("Decision")], frontmatter: {} };
        const result = matchTemplates(doc, [template]);
        expect(result).not.toBeNull();
        expect(result!.templateId).toBe("adr-1");
        expect(result!.templateName).toBe("Architecture Decision Record");
        expect(result!.type).toBe("document");
        expect(result!.tags).toEqual(["adr", "architecture"]);
        expect(result!.extractedFields).toEqual({});
    });

    it("returns empty tags array when template has null tags", () => {
        const template = makeTemplate({
            id: "t1",
            name: "Template",
            tags: null,
            matchRules: {
                minScore: 0,
                headings: [{ patterns: ["Context"], match: "exact", required: false }],
                frontmatter: [],
            },
        });
        const doc = { headings: [h("Context")], frontmatter: {} };
        const result = matchTemplates(doc, [template]);
        expect(result!.tags).toEqual([]);
    });
});
