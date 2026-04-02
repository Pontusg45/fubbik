import { describe, expect, it } from "vitest";
import { splitMarkdown } from "./split-markdown";

describe("document import flow", () => {
    it("splits a multi-section doc into ordered sections", () => {
        const md = `---\ntags:\n  - guide\n---\n\n# Getting Started\n\nWelcome to the guide.\n\n## Installation\n\nRun npm install.\n\n## Configuration\n\nEdit config.json.\n\n## Usage\n\nImport and call the function.\n`;
        const result = splitMarkdown(md, "docs/getting-started.md");

        expect(result.title).toBe("Getting Started");
        expect(result.tags).toEqual(["guide"]);
        expect(result.sections).toHaveLength(4);
        expect(result.sections[0]!.title).toBe("Getting Started \u2014 Introduction");
        expect(result.sections[0]!.order).toBe(0);
        expect(result.sections[1]!.title).toBe("Installation");
        expect(result.sections[1]!.order).toBe(1);
        expect(result.sections[2]!.title).toBe("Configuration");
        expect(result.sections[2]!.order).toBe(2);
        expect(result.sections[3]!.title).toBe("Usage");
        expect(result.sections[3]!.order).toBe(3);
    });

    it("handles markdown with only frontmatter and content", () => {
        const md = `---\ntitle: Quick Reference\ntags:\n  - reference\ndescription: A quick ref card\n---\n\nJust a simple reference document with no sections.\n`;
        const result = splitMarkdown(md, "ref.md");

        expect(result.title).toBe("Quick Reference");
        expect(result.description).toBe("A quick ref card");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.content).toBe("Just a simple reference document with no sections.");
    });

    it("handles empty file gracefully", () => {
        const result = splitMarkdown("", "empty.md");
        expect(result.title).toBe("empty");
        expect(result.sections).toHaveLength(0);
    });
});
