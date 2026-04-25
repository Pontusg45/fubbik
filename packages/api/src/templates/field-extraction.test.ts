import { describe, it, expect } from "vitest";
import { parseHeadings, extractFields } from "./field-extraction";
import type { FieldMapping } from "./types";

const ADR_MARKDOWN = `# ADR: Use Postgres

## Context
We need a relational database.

## Decision
We chose Postgres for its reliability.

## Alternatives
- MySQL
- MongoDB
- SQLite

## Consequences
Team needs Postgres expertise.
`;

const ADR_MAPPINGS: FieldMapping[] = [
    { headings: ["Context", "Background"], match: "prefix", target: "content" },
    { headings: ["Decision", "Choice"], match: "prefix", target: "rationale" },
    { headings: ["Alternatives"], match: "prefix", target: "alternatives" },
    { headings: ["Consequences"], match: "prefix", target: "consequences" },
];

describe("parseHeadings", () => {
    it("extracts headings with correct levels", () => {
        const md = `# Title\n## Context\n### Details\n`;
        const headings = parseHeadings(md);
        expect(headings).toEqual([
            { text: "Title", level: 1 },
            { text: "Context", level: 2 },
            { text: "Details", level: 3 },
        ]);
    });

    it("returns empty array for doc with no headings", () => {
        const md = `Just some plain text\nwith no headings at all.`;
        expect(parseHeadings(md)).toEqual([]);
    });

    it("handles mixed heading levels in ADR markdown", () => {
        const headings = parseHeadings(ADR_MARKDOWN);
        expect(headings).toContainEqual({ text: "ADR: Use Postgres", level: 1 });
        expect(headings).toContainEqual({ text: "Context", level: 2 });
        expect(headings).toContainEqual({ text: "Decision", level: 2 });
        expect(headings).toContainEqual({ text: "Alternatives", level: 2 });
        expect(headings).toContainEqual({ text: "Consequences", level: 2 });
    });
});

describe("extractFields", () => {
    it("extracts rationale from Decision section", () => {
        const { extracted } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        expect(extracted.rationale).toBe("We chose Postgres for its reliability.");
    });

    it("splits alternatives into string array from bullets", () => {
        const { extracted } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        expect(extracted.alternatives).toEqual(["MySQL", "MongoDB", "SQLite"]);
    });

    it("extracts consequences as trimmed string", () => {
        const { extracted } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        expect(extracted.consequences).toBe("Team needs Postgres expertise.");
    });

    it("builds remainingContent containing content-mapped sections", () => {
        const { remainingContent } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Context maps to "content" so stays in body
        expect(remainingContent).toContain("We need a relational database");
    });

    it("excludes extracted (non-content) sections from remainingContent", () => {
        const { remainingContent } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Decision → rationale (extracted out)
        expect(remainingContent).not.toContain("We chose Postgres");
        // Alternatives → alternatives (extracted out)
        expect(remainingContent).not.toContain("MySQL");
    });

    it("returns empty extracted fields when no mappings", () => {
        const { extracted, remainingContent } = extractFields(ADR_MARKDOWN, []);
        expect(extracted).toEqual({});
        expect(remainingContent).toBe(ADR_MARKDOWN);
    });

    it("wraps single-item non-bullet alternatives as array", () => {
        const md = `# Doc\n\n## Alternatives\nJust one option\n`;
        const mappings: FieldMapping[] = [
            { headings: ["Alternatives"], match: "exact", target: "alternatives" },
        ];
        const { extracted } = extractFields(md, mappings);
        expect(extracted.alternatives).toEqual(["Just one option"]);
    });

    it("handles scope extraction from key-value lines", () => {
        const md = `# Doc\n\n## Scope\narea: backend\nteam: platform\n`;
        const mappings: FieldMapping[] = [
            { headings: ["Scope"], match: "exact", target: "scope" },
        ];
        const { extracted } = extractFields(md, mappings);
        expect(extracted.scope).toEqual({ area: "backend", team: "platform" });
    });
});
