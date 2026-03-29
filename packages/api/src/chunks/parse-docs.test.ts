import { describe, it, expect } from "vitest";
import { parseDocFile } from "./parse-docs";

describe("parseDocFile", () => {
    it("extracts title from frontmatter", () => {
        const result = parseDocFile("guides/setup.md", "---\ntitle: Setup Guide\n---\n\nSome content here.");
        expect(result.title).toBe("Setup Guide");
        expect(result.content).toBe("Some content here.");
    });

    it("extracts title from first H1 heading when no frontmatter title", () => {
        const result = parseDocFile("docs/intro.md", "# Introduction\n\nWelcome to the project.");
        expect(result.title).toBe("Introduction");
        expect(result.content).toBe("Welcome to the project.");
    });

    it("falls back to filename when no frontmatter title or heading", () => {
        const result = parseDocFile("notes/my-cool-notes.md", "Just some text without a heading.");
        expect(result.title).toBe("my cool notes");
        expect(result.content).toBe("Just some text without a heading.");
    });

    it("extracts type from frontmatter", () => {
        const result = parseDocFile("api.md", "---\ntype: reference\n---\n\n# API Docs\n\nContent.");
        expect(result.type).toBe("reference");
    });

    it("defaults type to document", () => {
        const result = parseDocFile("readme.md", "# Readme\n\nHello.");
        expect(result.type).toBe("document");
    });

    it("extracts tags from frontmatter", () => {
        const result = parseDocFile("guide.md", "---\ntags:\n  - setup\n  - onboarding\n---\n\n# Guide\n\nContent.");
        expect(result.tags).toContain("setup");
        expect(result.tags).toContain("onboarding");
    });

    it("derives tags from folder path", () => {
        const result = parseDocFile("guides/api/auth.md", "# Auth\n\nContent.");
        expect(result.tags).toContain("guides");
        expect(result.tags).toContain("api");
    });

    it("merges frontmatter tags and folder tags without duplicates", () => {
        const result = parseDocFile("guides/setup.md", "---\ntags:\n  - guides\n  - extra\n---\n\n# Setup\n\nContent.");
        const guideCount = result.tags.filter(t => t === "guides").length;
        expect(guideCount).toBe(1);
        expect(result.tags).toContain("extra");
    });

    it("handles file with no content after frontmatter", () => {
        const result = parseDocFile("empty.md", "---\ntitle: Empty\n---\n");
        expect(result.title).toBe("Empty");
        expect(result.content).toBe("");
    });

    it("handles completely empty file", () => {
        const result = parseDocFile("blank.md", "");
        expect(result.title).toBe("blank");
        expect(result.content).toBe("");
    });

    it("extracts scope from frontmatter", () => {
        const result = parseDocFile("scoped.md", "---\nscope:\n  env: production\n---\n\n# Scoped\n\nContent.");
        expect(result.scope).toEqual({ env: "production" });
    });
});
