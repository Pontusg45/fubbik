import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { scanMetadata } from "../../lib/setup/tier2-metadata";

let tempDir: string;

beforeEach(() => {
    tempDir = mkdtempSync(join(tmpdir(), "fubbik-tier2-metadata-test-"));
});

afterEach(() => {
    rmSync(tempDir, { recursive: true, force: true });
});

describe("scanMetadata", () => {
    it("extracts tech stack from package.json", () => {
        // Given
        writeFileSync(
            join(tempDir, "package.json"),
            JSON.stringify({
                name: "my-app",
                dependencies: {
                    react: "^18.0.0",
                    next: "^14.0.0"
                },
                devDependencies: {
                    vitest: "^1.0.0",
                    typescript: "^5.0.0"
                }
            })
        );

        const { chunks } = scanMetadata(tempDir);
        // When
        const techChunk = chunks.find(c => c.title.includes("Tech Stack"));

        // Then
        expect(techChunk).toBeDefined();
        expect(techChunk!.tier).toBe(2);
        expect(techChunk!.category).toBe("tech-stack");
        expect(techChunk!.content.toLowerCase()).toContain("react");
        expect(techChunk!.content.toLowerCase()).toContain("next");
        expect(techChunk!.tags).toContain("framework");
    });

    it("detects monorepo from workspaces field", () => {
        // Given
        // Create workspace package directories
        mkdirSync(join(tempDir, "apps", "web"), { recursive: true });
        mkdirSync(join(tempDir, "packages", "shared"), { recursive: true });
        writeFileSync(join(tempDir, "apps", "web", "package.json"), JSON.stringify({ name: "@acme/web" }));
        writeFileSync(join(tempDir, "packages", "shared", "package.json"), JSON.stringify({ name: "@acme/shared" }));
        writeFileSync(
            join(tempDir, "package.json"),
            JSON.stringify({
                name: "my-monorepo",
                workspaces: ["apps/*", "packages/*"]
            })
        );

        const { chunks } = scanMetadata(tempDir);
        // When
        const structureChunk = chunks.find(c => c.category === "structure");

        // Then
        expect(structureChunk).toBeDefined();
        expect(structureChunk!.content.toLowerCase()).toContain("monorepo");
    });

    it("extracts tsconfig info", () => {
        // Given
        writeFileSync(
            join(tempDir, "tsconfig.json"),
            JSON.stringify({
                compilerOptions: {
                    strict: true,
                    target: "ES2022",
                    paths: {
                        "@/*": ["./src/*"]
                    }
                }
            })
        );

        const { chunks } = scanMetadata(tempDir);
        // When
        const tsChunk = chunks.find(c => c.title.includes("TypeScript"));

        // Then
        expect(tsChunk).toBeDefined();
        expect(tsChunk!.content.toLowerCase()).toContain("strict");
        expect(tsChunk!.category).toBe("config");
    });

    it("extracts env vars from .env.example (never .env)", () => {
        // Given
        writeFileSync(
            join(tempDir, ".env.example"),
            ["DATABASE_URL=postgres://localhost/mydb", "API_KEY=your-api-key-here", "PORT=3000"].join("\n")
        );
        // Write .env with secrets — must NOT be read
        writeFileSync(
            join(tempDir, ".env"),
            ["DATABASE_URL=postgres://user:secret@prod-host/db", "API_KEY=actual-secret-key-12345"].join("\n")
        );

        const { chunks } = scanMetadata(tempDir);
        // When
        const envChunk = chunks.find(c => c.type === "schema");

        // Then
        expect(envChunk).toBeDefined();
        expect(envChunk!.content).toContain("DATABASE_URL");
        expect(envChunk!.content).not.toContain("secret");
        expect(envChunk!.content).not.toContain("actual-secret-key-12345");
    });

    it("returns empty for project with no package.json", () => {
        // Given the inline inputs and test fixtures.
        // When
        const { chunks } = scanMetadata(tempDir);
        // Then
        expect(chunks).toHaveLength(0);
    });

    it("detects CI from github workflows", () => {
        // Given
        const workflowsDir = join(tempDir, ".github", "workflows");
        mkdirSync(workflowsDir, { recursive: true });
        writeFileSync(
            join(workflowsDir, "ci.yml"),
            [
                "name: CI",
                "on: [push]",
                "jobs:",
                "  test:",
                "    runs-on: ubuntu-latest",
                "    steps:",
                "      - uses: actions/checkout@v3"
            ].join("\n")
        );

        const { chunks } = scanMetadata(tempDir);
        // When
        const ciChunk = chunks.find(c => c.tags.includes("ci"));

        // Then
        expect(ciChunk).toBeDefined();
        expect(ciChunk!.category).toBe("config");
    });
});
