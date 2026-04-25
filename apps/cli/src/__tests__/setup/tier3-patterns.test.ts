import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { scanPatterns } from "../../lib/setup/tier3-patterns";

let tempDir: string;

function writePkg(
    deps: Record<string, string> = {},
    devDeps: Record<string, string> = {},
): void {
    writeFileSync(
        join(tempDir, "package.json"),
        JSON.stringify({
            name: "test-project",
            dependencies: deps,
            devDependencies: devDeps,
        }),
    );
}

beforeEach(() => {
    tempDir = mkdtempSync(join(tmpdir(), "fubbik-tier3-patterns-test-"));
});

afterEach(() => {
    rmSync(tempDir, { recursive: true, force: true });
});

describe("scanPatterns", () => {
    it("detects route structure when framework dep + routes/ dir exist", () => {
        writePkg({ elysia: "^1.0.0" });
        mkdirSync(join(tempDir, "src", "routes"), { recursive: true });
        writeFileSync(join(tempDir, "src", "routes", "users.ts"), "// users route");

        const { chunks, tips } = scanPatterns(tempDir);

        const routingChunk = chunks.find(c => c.tags.includes("routing"));
        expect(routingChunk).toBeDefined();
        expect(routingChunk!.tier).toBe(3);
        expect(routingChunk!.category).toBe("conventions");
        // Should not be a tip for routing
        const routingTip = tips.find(t => t.title.toLowerCase().includes("rout"));
        expect(routingTip).toBeUndefined();
    });

    it("detects test patterns when test runner + test files exist", () => {
        writePkg({}, { vitest: "^1.0.0" });
        mkdirSync(join(tempDir, "src", "__tests__"), { recursive: true });
        writeFileSync(join(tempDir, "src", "__tests__", "app.test.ts"), "// test");

        const { chunks } = scanPatterns(tempDir);

        const testChunk = chunks.find(c => c.tags.includes("testing"));
        expect(testChunk).toBeDefined();
        expect(testChunk!.content.toLowerCase()).toContain("vitest");
    });

    it("detects database when ORM dep + schema files exist", () => {
        writePkg({ "drizzle-orm": "^0.30.0" });
        mkdirSync(join(tempDir, "src", "db"), { recursive: true });
        writeFileSync(join(tempDir, "src", "db", "schema.ts"), "// schema");

        const { chunks } = scanPatterns(tempDir);

        const dbChunk = chunks.find(c => c.tags.includes("database"));
        expect(dbChunk).toBeDefined();
    });

    it("emits a tip (not chunk) when only dep exists without file structure", () => {
        writePkg({ "drizzle-orm": "^0.30.0" });
        // No schema files, no db/ directory

        const { chunks, tips } = scanPatterns(tempDir);

        const dbChunk = chunks.find(c => c.tags.includes("database"));
        expect(dbChunk).toBeUndefined();

        const dbTip = tips.find(t => t.title.toLowerCase().includes("database"));
        expect(dbTip).toBeDefined();
    });

    it("emits a tip when only file structure exists without dep", () => {
        writePkg(); // empty deps
        mkdirSync(join(tempDir, "src", "routes"), { recursive: true });
        writeFileSync(join(tempDir, "src", "routes", "index.ts"), "// routes");

        const { chunks, tips } = scanPatterns(tempDir);

        const routingChunk = chunks.find(c => c.tags.includes("routing"));
        expect(routingChunk).toBeUndefined();

        const routingTip = tips.find(t => t.title.toLowerCase().includes("rout"));
        expect(routingTip).toBeDefined();
    });

    it("returns empty for project with no detectable patterns", () => {
        writePkg(); // package.json with no relevant deps

        const { chunks, tips } = scanPatterns(tempDir);

        expect(chunks).toHaveLength(0);
        expect(tips).toHaveLength(0);
    });
});
