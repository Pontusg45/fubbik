import { describe, it, expect } from "vitest";

import { parseDocFile } from "./parse-docs";

describe("parseDocFile", () => {
    it("extracts title from frontmatter", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("guides/setup.md", "---\ntitle: Setup Guide\n---\n\nSome content here.");
        // Then
        expect(result.title).toBe("Setup Guide");
        expect(result.content).toBe("Some content here.");
    });

    it("extracts title from first H1 heading when no frontmatter title", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("docs/intro.md", "# Introduction\n\nWelcome to the project.");
        // Then
        expect(result.title).toBe("Introduction");
        expect(result.content).toBe("Welcome to the project.");
    });

    it("falls back to filename when no frontmatter title or heading", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("notes/my-cool-notes.md", "Just some text without a heading.");
        // Then
        expect(result.title).toBe("my cool notes");
        expect(result.content).toBe("Just some text without a heading.");
    });

    it("extracts type from frontmatter", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("api.md", "---\ntype: reference\n---\n\n# API Docs\n\nContent.");
        // Then
        expect(result.type).toBe("reference");
    });

    it("defaults type to document", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("readme.md", "# Readme\n\nHello.");
        // Then
        expect(result.type).toBe("document");
    });

    it("extracts tags from frontmatter", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("guide.md", "---\ntags:\n  - setup\n  - onboarding\n---\n\n# Guide\n\nContent.");
        // Then
        expect(result.tags).toContain("setup");
        expect(result.tags).toContain("onboarding");
    });

    it("derives tags from folder path", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("guides/api/auth.md", "# Auth\n\nContent.");
        // Then
        expect(result.tags).toContain("guides");
        expect(result.tags).toContain("api");
    });

    it("merges frontmatter tags and folder tags without duplicates", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("guides/setup.md", "---\ntags:\n  - guides\n  - extra\n---\n\n# Setup\n\nContent.");
        const guideCount = result.tags.filter(t => t === "guides").length;
        // Then
        expect(guideCount).toBe(1);
        expect(result.tags).toContain("extra");
    });

    it("handles file with no content after frontmatter", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("empty.md", "---\ntitle: Empty\n---\n");
        // Then
        expect(result.title).toBe("Empty");
        expect(result.content).toBe("");
    });

    it("handles completely empty file", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("blank.md", "");
        // Then
        expect(result.title).toBe("blank");
        expect(result.content).toBe("");
    });

    it("extracts scope from frontmatter", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = parseDocFile("scoped.md", "---\nscope:\n  env: production\n---\n\n# Scoped\n\nContent.");
        // Then
        expect(result.scope).toEqual({ env: "production" });
    });
});
