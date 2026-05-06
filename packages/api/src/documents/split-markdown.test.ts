import { describe, expect, it } from "vitest";
import { splitMarkdown } from "./split-markdown";

describe("splitMarkdown", () => {
    it("splits on H2 headings", () => {
        const md = `# My Document\n\nIntro paragraph.\n\n## First Section\n\nFirst content.\n\n## Second Section\n\nSecond content.\n`;
        const result = splitMarkdown(md, "docs/test.md");
        expect(result.title).toBe("My Document");
        expect(result.sections).toHaveLength(3);
        expect(result.sections[0]).toEqual({ title: "My Document \u2014 Introduction", content: "Intro paragraph.", order: 0 });
        expect(result.sections[1]).toEqual({ title: "First Section", content: "First content.", order: 1 });
        expect(result.sections[2]).toEqual({ title: "Second Section", content: "Second content.", order: 2 });
    });

    it("skips empty preamble", () => {
        const md = `# Title\n\n## Only Section\n\nContent here.\n`;
        const result = splitMarkdown(md, "test.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("Only Section");
        expect(result.sections[0]!.order).toBe(0);
    });

    it("falls back to filename for title", () => {
        const md = `## Section One\n\nContent.\n`;
        const result = splitMarkdown(md, "docs/my-cool-guide.md");
        expect(result.title).toBe("my cool guide");
    });

    it("preserves H3+ subheadings within sections", () => {
        const md = `# Doc\n\n## Main\n\n### Sub\n\nDetails.\n\n#### Deep\n\nMore.\n`;
        const result = splitMarkdown(md, "test.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.content).toContain("### Sub");
        expect(result.sections[0]!.content).toContain("#### Deep");
    });

    it("extracts frontmatter tags and description", () => {
        const md = `---\ntags:\n  - backend\n  - auth\ndescription: A guide to auth\n---\n\n# Auth Guide\n\n## Setup\n\nSteps here.\n`;
        const result = splitMarkdown(md, "docs/auth.md");
        expect(result.title).toBe("Auth Guide");
        expect(result.tags).toEqual(["backend", "auth", "docs"]);
        expect(result.description).toBe("A guide to auth");
    });

    it("treats whole file as single section when no H2s", () => {
        const md = `# Simple Note\n\nJust some content with no H2 headings.\n`;
        const result = splitMarkdown(md, "note.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("Simple Note \u2014 Introduction");
        expect(result.sections[0]!.content).toBe("Just some content with no H2 headings.");
    });

    it("auto-detects H3 as split level when no H2s exist", () => {
        const md = `# Title\n\n### First\n\nContent one.\n\n### Second\n\nContent two.\n`;
        const result = splitMarkdown(md, "test.md");
        expect(result.splitLevel).toBe(3);
        expect(result.sections).toHaveLength(2);
        expect(result.sections[0]!.title).toBe("First");
        expect(result.sections[1]!.title).toBe("Second");
    });

    it("uses explicit splitLevel override", () => {
        const md = `# Title\n\n## H2 Section\n\nContent.\n\n### H3 Section\n\nMore.\n`;
        const result = splitMarkdown(md, "test.md", 3);
        expect(result.splitLevel).toBe(3);
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("H3 Section");
    });

    it("returns splitLevel 2 for existing H2 documents", () => {
        const md = `# My Document\n\nIntro.\n\n## First Section\n\nFirst content.\n`;
        const result = splitMarkdown(md, "test.md");
        expect(result.splitLevel).toBe(2);
    });

    it("defaults splitLevel to 2 when no headings found", () => {
        const md = `# Title\n\nJust content with no sub-headings.\n`;
        const result = splitMarkdown(md, "test.md");
        expect(result.splitLevel).toBe(2);
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("Title \u2014 Introduction");
    });

    it("extracts decision context from trailing blockquotes", () => {
        const md = [
            "# Doc",
            "",
            "## Auth",
            "",
            "We use JWT for authentication.",
            "",
            "> **Rationale:** Stateless, no server-side sessions needed.",
            "",
            "> **Alternatives:**",
            "> - Session cookies",
            "> - OAuth tokens",
            "",
            "> **Consequences:** Requires token refresh logic.",
        ].join("\n");
        const result = splitMarkdown(md, "test.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.content).toBe("We use JWT for authentication.");
        expect(result.sections[0]!.rationale).toBe("Stateless, no server-side sessions needed.");
        expect(result.sections[0]!.alternatives).toEqual(["Session cookies", "OAuth tokens"]);
        expect(result.sections[0]!.consequences).toBe("Requires token refresh logic.");
    });

    it("does not extract blockquotes that are not decision context", () => {
        const md = [
            "# Doc",
            "",
            "## Notes",
            "",
            "> This is a regular blockquote in the middle.",
            "",
            "More content after the blockquote.",
        ].join("\n");
        const result = splitMarkdown(md, "test.md");
        expect(result.sections[0]!.content).toContain("> This is a regular blockquote");
        expect(result.sections[0]!.content).toContain("More content after the blockquote.");
        expect(result.sections[0]!.rationale).toBeUndefined();
    });

    it("handles partial decision context (only rationale)", () => {
        const md = [
            "# Doc",
            "",
            "## Design",
            "",
            "We chose X.",
            "",
            "> **Rationale:** Because Y.",
        ].join("\n");
        const result = splitMarkdown(md, "test.md");
        expect(result.sections[0]!.content).toBe("We chose X.");
        expect(result.sections[0]!.rationale).toBe("Because Y.");
        expect(result.sections[0]!.alternatives).toBeUndefined();
        expect(result.sections[0]!.consequences).toBeUndefined();
    });
});
