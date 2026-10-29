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
    { headings: ["Consequences"], match: "prefix", target: "consequences" }
];

describe("parseHeadings", () => {
    it("extracts headings with correct levels", () => {
        // Given
        const md = `# Title\n## Context\n### Details\n`;
        // When
        const headings = parseHeadings(md);
        // Then
        expect(headings).toEqual([
            { text: "Title", level: 1 },
            { text: "Context", level: 2 },
            { text: "Details", level: 3 }
        ]);
    });

    it("returns empty array for doc with no headings", () => {
        // Given
        const md = `Just some plain text\nwith no headings at all.`;
        // When the operation is evaluated by the assertion.
        // Then
        expect(parseHeadings(md)).toEqual([]);
    });

    it("handles mixed heading levels in ADR markdown", () => {
        // Given the inline inputs and test fixtures.
        // When
        const headings = parseHeadings(ADR_MARKDOWN);
        // Then
        expect(headings).toContainEqual({ text: "ADR: Use Postgres", level: 1 });
        expect(headings).toContainEqual({ text: "Context", level: 2 });
        expect(headings).toContainEqual({ text: "Decision", level: 2 });
        expect(headings).toContainEqual({ text: "Alternatives", level: 2 });
        expect(headings).toContainEqual({ text: "Consequences", level: 2 });
    });
});

describe("extractFields", () => {
    it("extracts rationale from Decision section", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { extracted } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Then
        expect(extracted.rationale).toBe("We chose Postgres for its reliability.");
    });

    it("splits alternatives into string array from bullets", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { extracted } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Then
        expect(extracted.alternatives).toEqual(["MySQL", "MongoDB", "SQLite"]);
    });

    it("extracts consequences as trimmed string", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { extracted } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Then
        expect(extracted.consequences).toBe("Team needs Postgres expertise.");
    });

    it("builds remainingContent containing content-mapped sections", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { remainingContent } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Then
        // Context maps to "content" so stays in body
        expect(remainingContent).toContain("We need a relational database");
    });

    it("excludes extracted (non-content) sections from remainingContent", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { remainingContent } = extractFields(ADR_MARKDOWN, ADR_MAPPINGS);
        // Then
        // Decision → rationale (extracted out)
        expect(remainingContent).not.toContain("We chose Postgres");
        // Alternatives → alternatives (extracted out)
        expect(remainingContent).not.toContain("MySQL");
    });

    it("returns empty extracted fields when no mappings", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { extracted, remainingContent } = extractFields(ADR_MARKDOWN, []);
        // Then
        expect(extracted).toEqual({});
        expect(remainingContent).toBe(ADR_MARKDOWN);
    });

    it("wraps single-item non-bullet alternatives as array", () => {
        // Given
        const md = `# Doc\n\n## Alternatives\nJust one option\n`;
        const mappings: FieldMapping[] = [{ headings: ["Alternatives"], match: "exact", target: "alternatives" }];
        // When
        const { extracted } = extractFields(md, mappings);
        // Then
        expect(extracted.alternatives).toEqual(["Just one option"]);
    });

    it("handles scope extraction from key-value lines", () => {
        // Given
        const md = `# Doc\n\n## Scope\narea: backend\nteam: platform\n`;
        const mappings: FieldMapping[] = [{ headings: ["Scope"], match: "exact", target: "scope" }];
        // When
        const { extracted } = extractFields(md, mappings);
        // Then
        expect(extracted.scope).toEqual({ area: "backend", team: "platform" });
    });
});
