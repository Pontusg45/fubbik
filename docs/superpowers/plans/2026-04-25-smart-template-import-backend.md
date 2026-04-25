# Smart Template Import — Backend Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the template system with matching rules and field extraction so the import pipeline can classify docs against templates and extract structured fields into chunk metadata.

**Architecture:** Four new columns on `chunkTemplate` (matchRules, fieldMappings, priority, tags). A pure-function matching engine scores docs against templates. A field extraction module maps heading sections to chunk fields. A new preview endpoint returns template suggestions without creating chunks. The existing import-docs endpoint gains `templateOverrides` for confirmed template application.

**Tech Stack:** Drizzle ORM (schema), Effect (services), Elysia + `t` schema (routes), vitest (tests)

**Spec:** `docs/superpowers/specs/2026-04-25-smart-template-import-design.md`

**Scope:** Backend only (DB, matching engine, extraction, API endpoints, seed data). Three follow-up plans needed: (1) template editor UI with match rules/field mappings/test panel, (2) import page UI with template suggestion preview, (3) CLI integration (`--dry-run` template matches, `--yes` auto-accept).

---

## File Structure

```
packages/db/src/
├── schema/
│   └── template.ts                         # MODIFIED: add matchRules, fieldMappings, priority, tags columns
└── seed.ts                                 # MODIFIED: seed built-in templates with matchRules + fieldMappings

packages/api/src/
├── templates/
│   ├── routes.ts                           # MODIFIED: extend POST/PATCH body schemas for new fields
│   ├── service.ts                          # MODIFIED: pass through new fields in create/update
│   ├── match-engine.ts                     # NEW: scoring engine (matchTemplates, scoreTemplate)
│   ├── match-engine.test.ts                # NEW: unit tests for scoring
│   ├── field-extraction.ts                 # NEW: extract sections into chunk fields
│   ├── field-extraction.test.ts            # NEW: unit tests for extraction
│   └── types.ts                            # NEW: shared types (MatchRules, FieldMapping, etc.)
├── documents/
│   ├── service.ts                          # MODIFIED: use template matching + extraction in importDocument
│   └── service.test.ts                     # MODIFIED: add template-aware import tests
└── chunks/
    └── routes.ts                           # MODIFIED: add preview endpoint, templateOverrides to import-docs
```

---

## Task 1: Shared Types

**Files:**
- Create: `packages/api/src/templates/types.ts`

- [ ] **Step 1: Create the types file**

```typescript
// packages/api/src/templates/types.ts

// --- Match Rules ---

export interface HeadingRule {
  /** One or more heading text patterns (alternatives — any match counts) */
  patterns: string[];
  /** How to match. Default: "prefix" */
  match: "exact" | "prefix" | "contains";
  /** Heading level (2 = ##, 3 = ###). Omit to match any level. */
  level?: number;
  /** If true, doc must have this heading to match. Default: true */
  required: boolean;
}

export interface FrontmatterRule {
  /** Frontmatter key to check */
  key: string;
  /** Match mode. Default: "exact" */
  match: "exact" | "oneOf" | "exists";
  /** Expected value for "exact" mode */
  value?: string;
  /** Expected values for "oneOf" mode */
  values?: string[];
}

export interface MatchRules {
  /** Minimum total score to qualify as a match */
  minScore: number;
  /** Heading patterns to look for */
  headings: HeadingRule[];
  /** Frontmatter field expectations */
  frontmatter: FrontmatterRule[];
}

// --- Field Extraction ---

export type ExtractionTarget =
  | "rationale"
  | "alternatives"
  | "consequences"
  | "summary"
  | "scope"
  | "content";

export interface FieldMapping {
  /** Heading patterns to match (same alias approach as matchRules) */
  headings: string[];
  /** How to match. Default: "prefix" */
  match: "exact" | "prefix" | "contains";
  /** Chunk field to populate */
  target: ExtractionTarget;
}

// --- Matching result ---

export interface ParsedHeading {
  text: string;
  level: number;
}

export interface TemplateMatch {
  templateId: string;
  templateName: string;
  score: number;
  type: string;
  tags: string[];
  extractedFields: ExtractedFields;
}

export interface ExtractedFields {
  rationale?: string;
  alternatives?: string[];
  consequences?: string;
  summary?: string;
  scope?: Record<string, string>;
  content?: string;
}

export interface TemplateWithRules {
  id: string;
  name: string;
  type: string;
  matchRules: MatchRules;
  fieldMappings: FieldMapping[] | null;
  priority: number;
  tags: string[] | null;
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/templates/types.ts
git commit -m "feat(templates): add shared types for match rules and field extraction"
```

---

## Task 2: Database Schema Changes

**Files:**
- Modify: `packages/db/src/schema/template.ts`

- [ ] **Step 1: Read the current schema**

Read: `packages/db/src/schema/template.ts`

- [ ] **Step 2: Add new columns**

Add four columns to the `chunkTemplate` table: `matchRules`, `fieldMappings`, `priority`, `tags`. Place them after the `isBuiltIn` column:

```typescript
matchRules: jsonb("match_rules"),
fieldMappings: jsonb("field_mappings"),
priority: integer("priority").notNull().default(0),
tags: text("tags").array(),
```

These use the existing Drizzle imports (`jsonb`, `integer`, `text` from `drizzle-orm/pg-core`). The `matchRules` and `fieldMappings` columns are nullable JSONB — null means the template has no import matching enabled.

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`
Expected: Schema updated successfully, 4 new columns added.

- [ ] **Step 4: Verify with existing tests**

Run: `cd packages/db && pnpm vitest run`
Expected: All existing tests pass. The template column test may need updating to expect the new column count.

- [ ] **Step 5: Update template column test if needed**

If `packages/db/src/__tests__/template.test.ts` checks column count, update the expected count from 8 to 12 (adding matchRules, fieldMappings, priority, tags).

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/template.ts packages/db/src/__tests__/template.test.ts
git commit -m "feat(db): add matchRules, fieldMappings, priority, tags to chunkTemplate"
```

---

## Task 3: Template Matching Engine

**Files:**
- Create: `packages/api/src/templates/match-engine.ts`
- Create: `packages/api/src/templates/match-engine.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// packages/api/src/templates/match-engine.test.ts

import { describe, expect, it } from "vitest";
import { matchTemplates, scoreTemplate, matchHeading } from "./match-engine";
import type { MatchRules, ParsedHeading, TemplateWithRules } from "./types";

describe("matchHeading", () => {
    it("matches exact (case-insensitive)", () => {
        expect(matchHeading("Decision", { text: "Decision", level: 2 }, "exact")).toBe(true);
        expect(matchHeading("Decision", { text: "decision", level: 2 }, "exact")).toBe(true);
        expect(matchHeading("Decision", { text: "Decision Record", level: 2 }, "exact")).toBe(false);
    });

    it("matches prefix (case-insensitive)", () => {
        expect(matchHeading("Decision", { text: "Decision: Use Postgres", level: 2 }, "prefix")).toBe(true);
        expect(matchHeading("Decision", { text: "Indecision", level: 2 }, "prefix")).toBe(false);
    });

    it("matches contains (case-insensitive)", () => {
        expect(matchHeading("decision", { text: "Our Final Decision", level: 2 }, "contains")).toBe(true);
    });
});

describe("scoreTemplate", () => {
    const headings: ParsedHeading[] = [
        { text: "Context", level: 2 },
        { text: "Decision", level: 2 },
        { text: "Alternatives Considered", level: 2 },
        { text: "Consequences", level: 2 },
    ];

    it("scores required + optional headings correctly", () => {
        const rules: MatchRules = {
            minScore: 2,
            headings: [
                { patterns: ["Decision"], match: "prefix", level: 2, required: true },
                { patterns: ["Alternatives"], match: "prefix", level: 2, required: true },
                { patterns: ["Consequences"], match: "prefix", level: 2, required: false },
            ],
            frontmatter: [],
        };
        const score = scoreTemplate(rules, headings, {});
        // 2 required (2 pts) + 1 optional (0.5 pts) = 2.5
        expect(score).toBe(2.5);
    });

    it("returns 0 if any required heading is missing", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Decision"], match: "prefix", level: 2, required: true },
                { patterns: ["Rollback"], match: "prefix", level: 2, required: true },
            ],
            frontmatter: [],
        };
        expect(scoreTemplate(rules, headings, {})).toBe(0);
    });

    it("scores frontmatter exact match", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [],
            frontmatter: [{ key: "type", match: "exact", value: "adr" }],
        };
        expect(scoreTemplate(rules, [], { type: "adr" })).toBe(1);
        expect(scoreTemplate(rules, [], { type: "runbook" })).toBe(0);
    });

    it("scores frontmatter oneOf match", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [],
            frontmatter: [{ key: "type", match: "oneOf", values: ["adr", "decision"] }],
        };
        expect(scoreTemplate(rules, [], { type: "decision" })).toBe(1);
    });

    it("scores frontmatter exists match", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [],
            frontmatter: [{ key: "status", match: "exists" }],
        };
        expect(scoreTemplate(rules, [], { status: "accepted" })).toBe(1);
        expect(scoreTemplate(rules, [], {})).toBe(0);
    });

    it("respects heading level filter", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Decision"], match: "prefix", level: 3, required: true },
            ],
            frontmatter: [],
        };
        // Heading is level 2, rule requires level 3
        expect(scoreTemplate(rules, headings, {})).toBe(0);
    });

    it("returns 0 if score below minScore", () => {
        const rules: MatchRules = {
            minScore: 5,
            headings: [
                { patterns: ["Decision"], match: "prefix", level: 2, required: true },
            ],
            frontmatter: [],
        };
        // Only 1 point, minScore is 5
        expect(scoreTemplate(rules, headings, {})).toBe(0);
    });

    it("matches alternative patterns (any of the patterns list)", () => {
        const rules: MatchRules = {
            minScore: 1,
            headings: [
                { patterns: ["Choice", "Decision", "Selected"], match: "prefix", level: 2, required: true },
            ],
            frontmatter: [],
        };
        expect(scoreTemplate(rules, headings, {})).toBe(1);
    });
});

describe("matchTemplates", () => {
    const makeTemplate = (overrides: Partial<TemplateWithRules> & { id: string }): TemplateWithRules => ({
        name: "Test",
        type: "document",
        matchRules: { minScore: 1, headings: [], frontmatter: [] },
        fieldMappings: null,
        priority: 0,
        tags: null,
        ...overrides,
    });

    it("returns the highest scoring template", () => {
        const headings: ParsedHeading[] = [
            { text: "Decision", level: 2 },
            { text: "Alternatives", level: 2 },
        ];
        const templates = [
            makeTemplate({
                id: "adr",
                name: "ADR",
                priority: 0,
                matchRules: {
                    minScore: 2,
                    headings: [
                        { patterns: ["Decision"], match: "prefix", required: true },
                        { patterns: ["Alternatives"], match: "prefix", required: true },
                    ],
                    frontmatter: [],
                },
            }),
            makeTemplate({
                id: "generic",
                name: "Generic",
                priority: 0,
                matchRules: {
                    minScore: 1,
                    headings: [
                        { patterns: ["Decision"], match: "prefix", required: true },
                    ],
                    frontmatter: [],
                },
            }),
        ];
        const result = matchTemplates({ headings, frontmatter: {} }, templates);
        expect(result).not.toBeNull();
        expect(result!.templateId).toBe("adr");
        expect(result!.score).toBe(2);
    });

    it("breaks ties by priority", () => {
        const headings: ParsedHeading[] = [{ text: "Decision", level: 2 }];
        const templates = [
            makeTemplate({
                id: "low",
                priority: 0,
                matchRules: { minScore: 1, headings: [{ patterns: ["Decision"], match: "prefix", required: true }], frontmatter: [] },
            }),
            makeTemplate({
                id: "high",
                priority: 10,
                matchRules: { minScore: 1, headings: [{ patterns: ["Decision"], match: "prefix", required: true }], frontmatter: [] },
            }),
        ];
        const result = matchTemplates({ headings, frontmatter: {} }, templates);
        expect(result!.templateId).toBe("high");
    });

    it("returns null when no templates match", () => {
        const result = matchTemplates(
            { headings: [{ text: "Introduction", level: 2 }], frontmatter: {} },
            [makeTemplate({
                id: "adr",
                matchRules: { minScore: 1, headings: [{ patterns: ["Decision"], match: "prefix", required: true }], frontmatter: [] },
            })],
        );
        expect(result).toBeNull();
    });

    it("returns null for empty templates list", () => {
        expect(matchTemplates({ headings: [], frontmatter: {} }, [])).toBeNull();
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && pnpm vitest run src/templates/match-engine.test.ts`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement the matching engine**

```typescript
// packages/api/src/templates/match-engine.ts

import type {
    ExtractedFields,
    FieldMapping,
    MatchRules,
    ParsedHeading,
    TemplateMatch,
    TemplateWithRules,
} from "./types";
import { extractFields } from "./field-extraction";

/**
 * Check if a single pattern matches a heading's text.
 */
export function matchHeading(
    pattern: string,
    heading: ParsedHeading,
    mode: "exact" | "prefix" | "contains",
): boolean {
    const p = pattern.toLowerCase();
    const h = heading.text.toLowerCase();
    switch (mode) {
        case "exact":
            return h === p;
        case "prefix":
            return h.startsWith(p);
        case "contains":
            return h.includes(p);
    }
}

/**
 * Score a single template's matchRules against a parsed document.
 * Returns 0 if any required rule fails or score < minScore.
 */
export function scoreTemplate(
    rules: MatchRules,
    headings: ParsedHeading[],
    frontmatter: Record<string, unknown>,
): number {
    let score = 0;

    // Check heading rules
    for (const rule of rules.headings) {
        const matched = headings.some(h => {
            if (rule.level !== undefined && h.level !== rule.level) return false;
            return rule.patterns.some(p => matchHeading(p, h, rule.match));
        });

        if (rule.required && !matched) return 0;
        if (matched) {
            score += rule.required ? 1 : 0.5;
        }
    }

    // Check frontmatter rules
    for (const rule of rules.frontmatter) {
        const value = frontmatter[rule.key];
        let matched = false;

        switch (rule.match) {
            case "exists":
                matched = value !== undefined;
                break;
            case "exact":
                matched = String(value) === rule.value;
                break;
            case "oneOf":
                matched = rule.values?.includes(String(value)) ?? false;
                break;
        }

        if (matched) {
            score += 1;
        }
    }

    return score >= rules.minScore ? score : 0;
}

/**
 * Match a parsed document against all templates with matchRules.
 * Returns the best match above threshold, or null.
 */
export function matchTemplates(
    doc: { headings: ParsedHeading[]; frontmatter: Record<string, unknown> },
    templates: TemplateWithRules[],
): TemplateMatch | null {
    if (templates.length === 0) return null;

    const scored = templates
        .map(t => ({
            template: t,
            score: scoreTemplate(t.matchRules, doc.headings, doc.frontmatter),
        }))
        .filter(s => s.score > 0)
        .sort((a, b) => {
            // Highest score first
            if (b.score !== a.score) return b.score - a.score;
            // Then by priority
            if (b.template.priority !== a.template.priority) return b.template.priority - a.template.priority;
            // Then by number of required rules (more specific wins)
            const aRequired = a.template.matchRules.headings.filter(h => h.required).length;
            const bRequired = b.template.matchRules.headings.filter(h => h.required).length;
            return bRequired - aRequired;
        });

    if (scored.length === 0) return null;

    const best = scored[0]!;
    return {
        templateId: best.template.id,
        templateName: best.template.name,
        score: best.score,
        type: best.template.type,
        tags: best.template.tags ?? [],
        extractedFields: {},  // Populated by caller with field-extraction module
    };
}
```

Note: `extractFields` import will be used in Task 5 when we wire it up. For now the `extractedFields` returns empty.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/api && pnpm vitest run src/templates/match-engine.test.ts`
Expected: PASS — all tests green.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/templates/match-engine.ts packages/api/src/templates/match-engine.test.ts
git commit -m "feat(templates): add template matching engine with scoring"
```

---

## Task 4: Field Extraction Engine

**Files:**
- Create: `packages/api/src/templates/field-extraction.ts`
- Create: `packages/api/src/templates/field-extraction.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// packages/api/src/templates/field-extraction.test.ts

import { describe, expect, it } from "vitest";
import { extractFields, parseHeadings } from "./field-extraction";
import type { FieldMapping } from "./types";

describe("parseHeadings", () => {
    it("extracts headings with levels from markdown", () => {
        const md = "# Title\n\nIntro\n\n## Context\n\nSome context\n\n### Details\n\nMore\n\n## Decision\n\nWe chose X";
        const headings = parseHeadings(md);
        expect(headings).toEqual([
            { text: "Title", level: 1 },
            { text: "Context", level: 2 },
            { text: "Details", level: 3 },
            { text: "Decision", level: 2 },
        ]);
    });

    it("returns empty for doc with no headings", () => {
        expect(parseHeadings("Just some text\nno headings")).toEqual([]);
    });
});

describe("extractFields", () => {
    const markdown = [
        "# ADR: Use Postgres",
        "",
        "## Context",
        "We need a relational database.",
        "",
        "## Decision",
        "We chose Postgres for its reliability.",
        "",
        "## Alternatives",
        "- MySQL",
        "- MongoDB",
        "- SQLite",
        "",
        "## Consequences",
        "Team needs Postgres expertise.",
    ].join("\n");

    const mappings: FieldMapping[] = [
        { headings: ["Context", "Background"], match: "prefix", target: "content" },
        { headings: ["Decision", "Choice"], match: "prefix", target: "rationale" },
        { headings: ["Alternatives"], match: "prefix", target: "alternatives" },
        { headings: ["Consequences"], match: "prefix", target: "consequences" },
    ];

    it("extracts rationale from Decision section", () => {
        const result = extractFields(markdown, mappings);
        expect(result.extracted.rationale).toBe("We chose Postgres for its reliability.");
    });

    it("splits alternatives into string array from bullets", () => {
        const result = extractFields(markdown, mappings);
        expect(result.extracted.alternatives).toEqual(["MySQL", "MongoDB", "SQLite"]);
    });

    it("extracts consequences", () => {
        const result = extractFields(markdown, mappings);
        expect(result.extracted.consequences).toBe("Team needs Postgres expertise.");
    });

    it("builds remaining content from unextracted sections", () => {
        const result = extractFields(markdown, mappings);
        // Context maps to "content" target, so it stays in body
        // Decision, Alternatives, Consequences are extracted OUT of body
        expect(result.remainingContent).toContain("We need a relational database");
        expect(result.remainingContent).not.toContain("We chose Postgres");
        expect(result.remainingContent).not.toContain("MySQL");
    });

    it("returns empty extracted fields when no mappings", () => {
        const result = extractFields(markdown, []);
        expect(result.extracted).toEqual({});
        expect(result.remainingContent).toContain("We need a relational database");
        expect(result.remainingContent).toContain("We chose Postgres");
    });

    it("handles scope extraction from key-value lines", () => {
        const md = "## Metadata\narea: backend\nteam: platform\npriority: high";
        const result = extractFields(md, [
            { headings: ["Metadata"], match: "exact", target: "scope" },
        ]);
        expect(result.extracted.scope).toEqual({
            area: "backend",
            team: "platform",
            priority: "high",
        });
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && pnpm vitest run src/templates/field-extraction.test.ts`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement field extraction**

```typescript
// packages/api/src/templates/field-extraction.ts

import type { ExtractedFields, FieldMapping, ParsedHeading } from "./types";
import { matchHeading } from "./match-engine";

interface ExtractionResult {
    extracted: ExtractedFields;
    remainingContent: string;
}

interface Section {
    heading: string;
    level: number;
    content: string;
}

/**
 * Parse all headings from markdown (for template matching input).
 */
export function parseHeadings(markdown: string): ParsedHeading[] {
    const headings: ParsedHeading[] = [];
    for (const line of markdown.split("\n")) {
        const match = line.match(/^(#{1,6})\s+(.+)$/);
        if (match) {
            headings.push({ text: match[2]!.trim(), level: match[1]!.length });
        }
    }
    return headings;
}

/**
 * Extract structured fields from markdown based on field mappings.
 * Returns extracted fields and remaining content (sections not extracted).
 */
export function extractFields(
    markdown: string,
    mappings: FieldMapping[],
): ExtractionResult {
    if (mappings.length === 0) {
        return { extracted: {}, remainingContent: markdown };
    }

    const sections = splitIntoSections(markdown);
    const extracted: ExtractedFields = {};
    const extractedIndices = new Set<number>();

    for (const mapping of mappings) {
        for (let i = 0; i < sections.length; i++) {
            if (extractedIndices.has(i)) continue;
            const section = sections[i]!;
            const heading: ParsedHeading = { text: section.heading, level: section.level };

            const matched = mapping.headings.some(p => matchHeading(p, heading, mapping.match));
            if (!matched) continue;

            const content = section.content.trim();

            switch (mapping.target) {
                case "rationale":
                    extracted.rationale = content;
                    extractedIndices.add(i);
                    break;
                case "alternatives":
                    extracted.alternatives = splitBullets(content);
                    extractedIndices.add(i);
                    break;
                case "consequences":
                    extracted.consequences = content;
                    extractedIndices.add(i);
                    break;
                case "summary":
                    extracted.summary = content;
                    extractedIndices.add(i);
                    break;
                case "scope":
                    extracted.scope = parseKeyValueLines(content);
                    extractedIndices.add(i);
                    break;
                case "content":
                    // "content" target means this section stays in the body
                    // Don't mark as extracted
                    break;
            }

            break; // First matching section wins per mapping
        }
    }

    // Build remaining content from unextracted sections
    const remaining = sections
        .filter((_, i) => !extractedIndices.has(i))
        .map(s => {
            if (s.heading) {
                return `${"#".repeat(s.level)} ${s.heading}\n\n${s.content}`;
            }
            return s.content;
        })
        .join("\n\n")
        .trim();

    return { extracted, remainingContent: remaining };
}

/**
 * Split markdown into sections by H2+ headings.
 */
function splitIntoSections(markdown: string): Section[] {
    const lines = markdown.split("\n");
    const sections: Section[] = [];
    let currentHeading = "";
    let currentLevel = 0;
    let currentLines: string[] = [];

    for (const line of lines) {
        const match = line.match(/^(#{1,6})\s+(.+)$/);
        if (match && match[1]!.length >= 2) {
            // Flush previous section
            const content = currentLines.join("\n").trim();
            if (content || currentHeading) {
                sections.push({ heading: currentHeading, level: currentLevel, content });
            }
            currentHeading = match[2]!.trim();
            currentLevel = match[1]!.length;
            currentLines = [];
        } else {
            currentLines.push(line);
        }
    }

    // Flush last section
    const content = currentLines.join("\n").trim();
    if (content || currentHeading) {
        sections.push({ heading: currentHeading, level: currentLevel, content });
    }

    return sections;
}

/**
 * Split bullet list content into string array entries.
 */
function splitBullets(content: string): string[] {
    const lines = content.split("\n");
    const items: string[] = [];
    for (const line of lines) {
        const match = line.match(/^\s*[-*]\s+(.+)$/);
        if (match) {
            items.push(match[1]!.trim());
        }
    }
    return items.length > 0 ? items : [content];
}

/**
 * Parse key: value lines into a Record.
 */
function parseKeyValueLines(content: string): Record<string, string> {
    const result: Record<string, string> = {};
    for (const line of content.split("\n")) {
        const match = line.match(/^(\w[\w\s]*?):\s*(.+)$/);
        if (match) {
            result[match[1]!.trim()] = match[2]!.trim();
        }
    }
    return result;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/api && pnpm vitest run src/templates/field-extraction.test.ts`
Expected: PASS — all tests green.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/templates/field-extraction.ts packages/api/src/templates/field-extraction.test.ts
git commit -m "feat(templates): add field extraction engine for structured import"
```

---

## Task 5: Template API Extensions

**Files:**
- Modify: `packages/api/src/templates/routes.ts`
- Modify: `packages/api/src/templates/service.ts`

- [ ] **Step 1: Read current routes and service**

Read: `packages/api/src/templates/routes.ts` and `packages/api/src/templates/service.ts`

- [ ] **Step 2: Extend POST /templates body schema**

In `routes.ts`, add the new optional fields to the POST body schema. After the existing `content` field, add:

```typescript
matchRules: t.Optional(t.Any()),
fieldMappings: t.Optional(t.Any()),
priority: t.Optional(t.Number()),
tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 })),
```

Using `t.Any()` for the JSONB fields since Elysia's `t` schema doesn't validate deeply-nested JSON structures at the route level — the matching engine validates the shape when it consumes them.

- [ ] **Step 3: Extend PATCH /templates body schema**

Same four fields, all optional, added to the PATCH body schema.

- [ ] **Step 4: Update service createTemplate**

In `service.ts`, update the `createTemplate` function's body type and the `createTemplateRepo` call to pass through the new fields:

```typescript
// Add to body type:
matchRules?: unknown;
fieldMappings?: unknown;
priority?: number;
tags?: string[];
```

Pass them through to the repo call alongside existing fields.

- [ ] **Step 5: Update service updateTemplate**

Same pattern — add the new fields to the update body type and pass through to `updateTemplateRepo`.

- [ ] **Step 6: Run existing tests**

Run: `cd packages/api && pnpm vitest run`
Expected: All existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add packages/api/src/templates/routes.ts packages/api/src/templates/service.ts
git commit -m "feat(templates): extend template API with matchRules, fieldMappings, priority, tags"
```

---

## Task 6: Preview Endpoint

**Files:**
- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/chunks/service.ts` (or create a dedicated preview service function)

- [ ] **Step 1: Read current import-docs route**

Read: `packages/api/src/chunks/routes.ts` (lines 66-96) for the existing import-docs handler pattern.

- [ ] **Step 2: Add preview service function**

Add to `packages/api/src/chunks/service.ts` (or a new file if service.ts is too large — check its size first):

```typescript
import { parseHeadings, extractFields } from "../templates/field-extraction";
import { matchTemplates } from "../templates/match-engine";
import { parseDocFile, extractFrontmatter } from "./parse-docs";
import type { TemplateWithRules } from "../templates/types";

export function previewImportDocs(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string,
) {
    return Effect.gen(function* () {
        // Fetch all templates with matchRules
        const allTemplates = yield* listTemplates(userId);
        const matchableTemplates: TemplateWithRules[] = allTemplates
            .filter((t: any) => t.matchRules != null)
            .map((t: any) => ({
                id: t.id,
                name: t.name,
                type: t.type,
                matchRules: t.matchRules,
                fieldMappings: t.fieldMappings ?? null,
                priority: t.priority ?? 0,
                tags: t.tags ?? null,
            }));

        const results = files.map(file => {
            const parsed = parseDocFile(file.path, file.content);
            const { frontmatter } = extractFrontmatter(file.content);
            const headings = parseHeadings(file.content);

            const templateMatch = matchTemplates({ headings, frontmatter }, matchableTemplates);

            let suggestedTemplate = null;
            if (templateMatch) {
                // Run field extraction if template has fieldMappings
                const matchedTemplate = matchableTemplates.find(t => t.id === templateMatch.templateId);
                const extractedFields = matchedTemplate?.fieldMappings
                    ? extractFields(file.content, matchedTemplate.fieldMappings).extracted
                    : {};

                // Merge tags: template tags ∪ frontmatter tags ∪ path tags
                const templateTags = matchedTemplate?.tags ?? [];
                const mergedTags = [...new Set([...templateTags, ...parsed.tags])];

                suggestedTemplate = {
                    id: templateMatch.templateId,
                    name: templateMatch.templateName,
                    score: templateMatch.score,
                    type: templateMatch.type,
                    tags: mergedTags,
                    extractedFields,
                };
            }

            return {
                path: file.path,
                title: parsed.title,
                suggestedTemplate,
                parsed: {
                    title: parsed.title,
                    type: parsed.type,
                    tags: parsed.tags,
                    content: parsed.content,
                },
            };
        });

        return results;
    });
}
```

Note: The actual import will need the `listTemplates` function from the template service. Import it at the top of the file.

- [ ] **Step 3: Add the preview route**

In `packages/api/src/chunks/routes.ts`, add the new endpoint before the existing `import-docs` POST handler:

```typescript
.post(
    "/chunks/import-docs/preview",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    chunkService.previewImportDocs(session.user.id, ctx.body.files, ctx.body.codebaseId)
                )
            )
        ),
    {
        body: t.Object({
            files: t.Array(
                t.Object({
                    path: t.String({ maxLength: 500 }),
                    content: t.String({ maxLength: 100000 }),
                }),
                { maxItems: 500 }
            ),
            codebaseId: t.String(),
        }),
    }
)
```

- [ ] **Step 4: Run existing tests**

Run: `cd packages/api && pnpm vitest run`
Expected: All existing tests pass. No regressions.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/routes.ts packages/api/src/chunks/service.ts
git commit -m "feat(import): add import-docs preview endpoint with template matching"
```

---

## Task 7: Template-Aware Import

**Files:**
- Modify: `packages/api/src/chunks/routes.ts` (add templateOverrides to body schema)
- Modify: `packages/api/src/documents/service.ts` (use template matching + extraction in importDocument)

- [ ] **Step 1: Extend import-docs body schema**

In the existing `POST /chunks/import-docs` handler in `routes.ts`, add `templateOverrides` as an optional field to the body schema:

```typescript
templateOverrides: t.Optional(t.Record(t.String(), t.Union([t.String(), t.Null()]))),
```

Pass it through to the service call: `chunkService.importDocs(session.user.id, ctx.body.files, ctx.body.codebaseId, ctx.body.templateOverrides)`

- [ ] **Step 2: Read the current importDocs service function**

Read: `packages/api/src/chunks/service.ts` — find the `importDocs` function and understand how it delegates to the document service.

- [ ] **Step 3: Modify importDocs to pass templateOverrides**

Update `importDocs` to accept and forward `templateOverrides`:

```typescript
export function importDocs(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string,
    templateOverrides?: Record<string, string | null>,
) {
    // ... existing logic, but pass templateOverrides to importDocument
}
```

- [ ] **Step 4: Modify importDocument to apply template**

In `packages/api/src/documents/service.ts`, update `importDocument` to accept an optional `templateId`. When provided:

1. Look up the template by ID
2. Run `extractFields(rawContent, template.fieldMappings)` to get extracted fields and remaining content
3. When creating chunks, use:
   - `type` from template instead of hardcoded `"document"`
   - `rationale`, `alternatives`, `consequences`, `summary`, `scope` from extracted fields
   - `content` from `remainingContent` instead of section content
   - Merged tags (template tags ∪ existing tags)

For the section-based chunking that already exists: when a template is applied, create a **single chunk** from the whole document (with extracted fields) instead of splitting by H2. This is because field extraction treats the doc as a cohesive unit.

- [ ] **Step 5: Run all tests**

Run: `cd packages/api && pnpm vitest run`
Expected: All tests pass. Existing import tests should still work since `templateOverrides` is optional — when not provided, behavior is unchanged.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/chunks/routes.ts packages/api/src/chunks/service.ts packages/api/src/documents/service.ts
git commit -m "feat(import): apply template matching and field extraction during import"
```

---

## Task 8: Seed Built-in Templates with Match Rules

**Files:**
- Modify: `packages/db/src/seed.ts`

- [ ] **Step 1: Read current seed.ts**

Read: `packages/db/src/seed.ts` — understand the seeding pattern (especially `onConflictDoUpdate`).

- [ ] **Step 2: Add built-in template seeding**

Add a new section after the existing chunk type seeding that creates/updates built-in templates with matchRules and fieldMappings. Use the same `onConflictDoUpdate` pattern for idempotency.

```typescript
const BUILTIN_TEMPLATES = [
    {
        id: "builtin-decision-record",
        name: "Decision Record",
        description: "Architecture Decision Record (ADR)",
        type: "document",
        content: "## Context\n\n## Decision\n\n## Alternatives\n\n## Consequences\n",
        isBuiltIn: true,
        priority: 10,
        tags: ["architecture", "decision"],
        matchRules: {
            minScore: 2,
            headings: [
                { patterns: ["Decision", "Choice", "Selected Option"], match: "prefix", level: 2, required: true },
                { patterns: ["Alternatives", "Options Considered"], match: "prefix", level: 2, required: true },
                { patterns: ["Consequences", "Impact"], match: "prefix", level: 2, required: false },
                { patterns: ["Context", "Background"], match: "prefix", level: 2, required: false },
            ],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["adr", "decision", "architecture-decision"] },
            ],
        },
        fieldMappings: [
            { headings: ["Context", "Background"], match: "prefix", target: "content" },
            { headings: ["Decision", "Choice"], match: "prefix", target: "rationale" },
            { headings: ["Alternatives", "Options Considered"], match: "prefix", target: "alternatives" },
            { headings: ["Consequences", "Impact"], match: "prefix", target: "consequences" },
        ],
    },
    {
        id: "builtin-api-reference",
        name: "API Reference",
        description: "API endpoint documentation",
        type: "reference",
        content: "## Endpoint\n\n## Request\n\n## Response\n\n## Errors\n",
        isBuiltIn: true,
        priority: 5,
        tags: ["api", "reference"],
        matchRules: {
            minScore: 2,
            headings: [
                { patterns: ["Endpoint", "URL", "Route"], match: "prefix", level: 2, required: true },
                { patterns: ["Request", "Parameters", "Payload"], match: "prefix", level: 2, required: true },
                { patterns: ["Response", "Returns"], match: "prefix", level: 2, required: false },
                { patterns: ["Errors", "Error Codes"], match: "prefix", level: 2, required: false },
            ],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["api", "endpoint", "reference"] },
            ],
        },
        fieldMappings: null,
    },
    {
        id: "builtin-meeting-notes",
        name: "Meeting Notes",
        description: "Meeting notes with attendees and action items",
        type: "note",
        content: "## Attendees\n\n## Agenda\n\n## Notes\n\n## Action Items\n",
        isBuiltIn: true,
        priority: 5,
        tags: ["meeting"],
        matchRules: {
            minScore: 2,
            headings: [
                { patterns: ["Attendees", "Participants", "Present"], match: "prefix", level: 2, required: true },
                { patterns: ["Action Items", "Actions", "TODOs", "Next Steps"], match: "prefix", level: 2, required: true },
                { patterns: ["Agenda"], match: "prefix", level: 2, required: false },
                { patterns: ["Notes", "Discussion"], match: "prefix", level: 2, required: false },
            ],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["meeting", "meeting-notes"] },
            ],
        },
        fieldMappings: [
            { headings: ["Summary", "TLDR"], match: "prefix", target: "summary" },
        ],
    },
    {
        id: "builtin-checklist",
        name: "Checklist",
        description: "Checklist or procedure",
        type: "checklist",
        content: "## Prerequisites\n\n## Steps\n\n- [ ] Step 1\n- [ ] Step 2\n",
        isBuiltIn: true,
        priority: 3,
        tags: ["checklist"],
        matchRules: {
            minScore: 1,
            headings: [],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["checklist", "procedure"] },
            ],
        },
        fieldMappings: null,
    },
    {
        id: "builtin-schema",
        name: "Schema",
        description: "Data schema or model definition",
        type: "schema",
        content: "## Fields\n\n## Relationships\n\n## Constraints\n",
        isBuiltIn: true,
        priority: 3,
        tags: ["schema"],
        matchRules: {
            minScore: 1,
            headings: [],
            frontmatter: [
                { key: "type", match: "oneOf", values: ["schema", "model", "data-model"] },
            ],
        },
        fieldMappings: null,
    },
];
```

Insert using:
```typescript
for (const tmpl of BUILTIN_TEMPLATES) {
    await db
        .insert(chunkTemplate)
        .values(tmpl)
        .onConflictDoUpdate({
            target: chunkTemplate.id,
            set: {
                name: tmpl.name,
                description: tmpl.description,
                type: tmpl.type,
                content: tmpl.content,
                matchRules: tmpl.matchRules,
                fieldMappings: tmpl.fieldMappings,
                priority: tmpl.priority,
                tags: tmpl.tags,
            },
        });
}
```

- [ ] **Step 3: Run the seed**

Run: `pnpm seed`
Expected: Seed completes successfully. Built-in templates created with matchRules.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/seed.ts
git commit -m "feat(db): seed built-in templates with match rules and field mappings"
```

---

## Task 9: Type Check and Final Validation

**Files:** None new — validation step.

- [ ] **Step 1: Run type checking**

Run: `pnpm run check-types`
Expected: No type errors.

- [ ] **Step 2: Run all API tests**

Run: `cd packages/api && pnpm vitest run`
Expected: All tests pass.

- [ ] **Step 3: Run all DB tests**

Run: `cd packages/db && pnpm vitest run`
Expected: All tests pass.

- [ ] **Step 4: Run linting**

Run: `pnpm --filter @fubbik/api lint`
Expected: No new errors.

- [ ] **Step 5: Manual API test (if server running)**

Test the preview endpoint:
```bash
curl -X POST http://localhost:3000/api/chunks/import-docs/preview \
  -H "Content-Type: application/json" \
  -d '{
    "files": [{
      "path": "docs/adr-001.md",
      "content": "---\ntype: adr\n---\n# Use Postgres\n\n## Context\nWe need a DB.\n\n## Decision\nPostgres.\n\n## Alternatives\n- MySQL\n- SQLite\n\n## Consequences\nNeed expertise."
    }],
    "codebaseId": "your-codebase-id"
  }'
```

Expected: Response includes `suggestedTemplate` with the Decision Record template match, score >= 2, and extracted fields.

- [ ] **Step 6: Commit if any fixes were needed**

```bash
git add -A && git commit -m "fix: address type/lint issues in smart template import"
```

Only run this step if Steps 1-4 required fixes.
