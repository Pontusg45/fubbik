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
        expect(result.tags).toEqual(["backend", "auth"]);
        expect(result.description).toBe("A guide to auth");
    });

    it("treats whole file as single section when no H2s", () => {
        const md = `# Simple Note\n\nJust some content with no H2 headings.\n`;
        const result = splitMarkdown(md, "note.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("Simple Note \u2014 Introduction");
        expect(result.sections[0]!.content).toBe("Just some content with no H2 headings.");
    });
});
