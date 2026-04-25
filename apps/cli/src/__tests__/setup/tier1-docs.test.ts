import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { scanDocs } from "../../lib/setup/tier1-docs";

let tempDir: string;

beforeEach(() => {
    tempDir = mkdtempSync(join(tmpdir(), "fubbik-tier1-docs-test-"));
});

afterEach(() => {
    rmSync(tempDir, { recursive: true, force: true });
});

describe("scanDocs", () => {
    it("finds root README.md", () => {
        writeFileSync(join(tempDir, "README.md"), "# My Project\n\nThis is the readme.");

        const chunks = scanDocs(tempDir);

        expect(chunks).toHaveLength(1);
        const chunk = chunks[0]!;
        expect(chunk.title).toBe("Project README");
        expect(chunk.tier).toBe(1);
        expect(chunk.category).toBe("documents");
        expect(chunk.tags).toContain("documentation");
    });

    it("finds markdown files in docs/ directory", () => {
        const docsDir = join(tempDir, "docs");
        mkdirSync(docsDir);
        writeFileSync(join(docsDir, "guide.md"), "# Guide\n\nSome guide content.");

        const chunks = scanDocs(tempDir);

        expect(chunks).toHaveLength(1);
        const chunk = chunks[0]!;
        expect(chunk.title).toBe("Guide");
        expect(chunk.tags).toContain("docs");
        expect(chunk.tier).toBe(1);
        expect(chunk.category).toBe("documents");
    });

    it("returns empty array for project with no docs", () => {
        const chunks = scanDocs(tempDir);
        expect(chunks).toHaveLength(0);
    });

    it("ignores node_modules", () => {
        const nodeModulesDir = join(tempDir, "node_modules", "some-pkg");
        mkdirSync(nodeModulesDir, { recursive: true });
        writeFileSync(join(nodeModulesDir, "README.md"), "# Package README\n\nContent.");

        const chunks = scanDocs(tempDir);

        expect(chunks).toHaveLength(0);
    });
});
