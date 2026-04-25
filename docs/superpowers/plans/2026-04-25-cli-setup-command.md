# CLI `fubbik setup` Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `fubbik setup` CLI command that scans a project across three tiers (docs, metadata, code patterns), previews findings, and imports to the server in one confirmed shot.

**Architecture:** A new `setup.ts` command orchestrates six phases (preflight → discover → preview → confirm → import → tips). Discovery logic lives in `apps/cli/src/lib/setup/` with one file per tier. The existing doc-scanning logic from `scanner.ts` is extracted to `tier1-docs.ts` and reused by both `init --scan` and `setup`.

**Tech Stack:** Commander.js (CLI), picocolors (output), existing fubbik API endpoints (no new server code)

**Spec:** `docs/superpowers/specs/2026-04-25-cli-setup-command-design.md`

---

## File Structure

```
apps/cli/src/
├── commands/
│   └── setup.ts                    # Command definition, phase orchestration
├── lib/
│   ├── scanner.ts                  # MODIFIED: import scanDocs from setup/tier1-docs
│   └── setup/
│       ├── index.ts                # Re-exports types + discover function
│       ├── types.ts                # DiscoveryResult, DiscoveredChunk, etc.
│       ├── tier1-docs.ts           # Extracted from scanner.ts (doc scanning)
│       ├── tier2-metadata.ts       # Config file readers (package.json, tsconfig, etc.)
│       ├── tier3-patterns.ts       # Code pattern detection (routes, tests, DB, etc.)
│       ├── connections.ts          # Connection inference between discovered chunks
│       ├── tips.ts                 # Lower-confidence suggestion generation
│       ├── discover.ts             # Orchestrates all tiers → DiscoveryResult
│       ├── preview.ts              # Summary table formatting
│       └── import-chunks.ts        # Server API calls for import
├── index.ts                        # MODIFIED: register setupCommand
└── __tests__/
    ├── commands.test.ts            # MODIFIED: add setup help test
    └── setup/
        ├── tier1-docs.test.ts      # Unit tests for doc scanning
        ├── tier2-metadata.test.ts  # Unit tests for metadata scanning
        ├── tier3-patterns.test.ts  # Unit tests for pattern detection
        ├── connections.test.ts     # Unit tests for connection inference
        ├── tips.test.ts            # Unit tests for tip generation
        ├── preview.test.ts         # Unit tests for preview table
        └── discover.test.ts        # Integration test for full discovery
```

---

## Task 1: Types and Shared Types File

**Files:**
- Create: `apps/cli/src/lib/setup/types.ts`

- [ ] **Step 1: Create the types file**

```typescript
// apps/cli/src/lib/setup/types.ts

export interface DiscoveredChunk {
    title: string;
    content: string;
    type: string;
    tags: string[];
    tier: 1 | 2 | 3;
    /** Category for preview grouping: documents, tech-stack, structure, conventions, config */
    category: "documents" | "tech-stack" | "structure" | "conventions" | "config";
    /** File path or detection name that produced this chunk */
    source: string;
    /** Glob patterns for file-area linking */
    appliesTo?: string[];
}

export interface DiscoveredConnection {
    /** Title of the source chunk (resolved to ID after import) */
    sourceTitle: string;
    /** Title of the target chunk (resolved to ID after import) */
    targetTitle: string;
    relation: string;
}

export interface Tip {
    title: string;
    detail: string;
}

export interface DiscoveryResult {
    codebase: { name: string; remoteUrl: string | null; localPath: string };
    chunks: DiscoveredChunk[];
    connections: DiscoveredConnection[];
    tags: string[];
    tips: Tip[];
}

```

- [ ] **Step 2: Create the index re-export file**

```typescript
// apps/cli/src/lib/setup/index.ts

export type {
    DiscoveredChunk,
    DiscoveredConnection,
    DiscoveryResult,
    Tip,
} from "./types";
export { discover } from "./discover";
export { formatPreview } from "./preview";
export { importToServer } from "./import-chunks";
```

Note: `discover`, `formatPreview`, and `importToServer` don't exist yet — they'll be created in subsequent tasks. This file will cause type errors until then, which is expected.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/lib/setup/types.ts apps/cli/src/lib/setup/index.ts
git commit -m "feat(cli): add setup command types and index"
```

---

## Task 2: Extract Tier 1 Doc Scanning from scanner.ts

**Files:**
- Create: `apps/cli/src/lib/setup/tier1-docs.ts`
- Create: `apps/cli/src/__tests__/setup/tier1-docs.test.ts`
- Modify: `apps/cli/src/lib/scanner.ts`

- [ ] **Step 1: Write the failing test for tier 1 scanning**

```typescript
// apps/cli/src/__tests__/setup/tier1-docs.test.ts

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { rmSync } from "node:fs";

import { scanDocs } from "../../lib/setup/tier1-docs";

const TMP_DIR = join(import.meta.dirname, "__tmp_tier1__");

beforeEach(() => {
    mkdirSync(TMP_DIR, { recursive: true });
});

afterEach(() => {
    rmSync(TMP_DIR, { recursive: true, force: true });
});

describe("scanDocs", () => {
    it("finds root README.md", () => {
        writeFileSync(join(TMP_DIR, "README.md"), "# My Project\n\nHello world");
        const chunks = scanDocs(TMP_DIR);
        expect(chunks.length).toBe(1);
        expect(chunks[0]!.title).toBe("Project README");
        expect(chunks[0]!.tier).toBe(1);
        expect(chunks[0]!.category).toBe("documents");
        expect(chunks[0]!.tags).toContain("documentation");
    });

    it("finds markdown files in docs/ directory", () => {
        mkdirSync(join(TMP_DIR, "docs"), { recursive: true });
        writeFileSync(join(TMP_DIR, "docs", "guide.md"), "# Guide\n\nSome guide content");
        const chunks = scanDocs(TMP_DIR);
        expect(chunks.length).toBe(1);
        expect(chunks[0]!.title).toBe("Guide");
        expect(chunks[0]!.tags).toContain("docs");
    });

    it("returns empty array for project with no docs", () => {
        writeFileSync(join(TMP_DIR, "index.ts"), "console.log('hi')");
        const chunks = scanDocs(TMP_DIR);
        expect(chunks).toEqual([]);
    });

    it("ignores node_modules", () => {
        mkdirSync(join(TMP_DIR, "node_modules", "pkg"), { recursive: true });
        writeFileSync(join(TMP_DIR, "node_modules", "pkg", "README.md"), "# Pkg");
        const chunks = scanDocs(TMP_DIR);
        expect(chunks).toEqual([]);
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tier1-docs.test.ts`
Expected: FAIL — `scanDocs` does not exist yet.

- [ ] **Step 3: Create tier1-docs.ts by extracting from scanner.ts**

Extract the doc-scanning logic from `scanner.ts` into `tier1-docs.ts`, converting the output to `DiscoveredChunk[]`:

```typescript
// apps/cli/src/lib/setup/tier1-docs.ts

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";

import { DEFAULT_THRESHOLDS } from "@fubbik/api/chunk-size";

import type { DiscoveredChunk } from "./types";

const IGNORE_DIRS = new Set([
    "node_modules", ".git", ".turbo", "dist", "build",
    ".next", ".output", ".cache", "coverage", ".fubbik",
]);

const DOC_FILES = ["README.md", "CLAUDE.md", "CONTRIBUTING.md", "Agents.md", "CHANGELOG.md"];

const DOC_FILE_TITLES: Record<string, string> = {
    "README.md": "Project README",
    "CLAUDE.md": "AI Assistant Instructions (CLAUDE.md)",
    "CONTRIBUTING.md": "Contributing Guide",
    "Agents.md": "AI Agents Documentation",
    "CHANGELOG.md": "Changelog",
};

/**
 * Scan a project directory for markdown documentation files.
 * Returns DiscoveredChunk[] with tier=1 and category="documents".
 */
export function scanDocs(dir: string): DiscoveredChunk[] {
    const chunks: DiscoveredChunk[] = [];

    // 1. Root documentation files
    for (const docFile of DOC_FILES) {
        const path = join(dir, docFile);
        if (existsSync(path)) {
            const content = readFileSync(path, "utf-8");
            if (content.trim()) {
                addChunkWithAutoSplit(chunks, {
                    title: DOC_FILE_TITLES[docFile] ?? docFile,
                    content,
                    type: "guide",
                    tags: ["documentation", "project"],
                    tier: 1,
                    category: "documents",
                    source: docFile,
                });
            }
        }
    }

    // 2. docs/ directory — each markdown file becomes a chunk
    const docsDir = join(dir, "docs");
    if (existsSync(docsDir) && statSync(docsDir).isDirectory()) {
        for (const mdPath of findFiles(docsDir, ".md")) {
            const content = readFileSync(mdPath, "utf-8");
            const rel = relative(dir, mdPath);
            const title = extractMarkdownTitle(content) ?? rel;
            addChunkWithAutoSplit(chunks, {
                title,
                content,
                type: "guide",
                tags: ["documentation", ...pathTags(rel)],
                tier: 1,
                category: "documents",
                source: rel,
            });
        }
    }

    // 3. Markdown files throughout the project (not in docs/ or root)
    for (const mdPath of findFiles(dir, ".md")) {
        const rel = relative(dir, mdPath);
        if (DOC_FILES.includes(basename(mdPath)) && rel === basename(mdPath)) continue;
        if (rel.startsWith("docs/") || rel.startsWith("docs\\")) continue;

        const content = readFileSync(mdPath, "utf-8");
        if (!content.trim()) continue;
        const title = extractMarkdownTitle(content) ?? rel;
        addChunkWithAutoSplit(chunks, {
            title,
            content,
            type: "guide",
            tags: ["documentation", ...pathTags(rel)],
            tier: 1,
            category: "documents",
            source: rel,
        });
    }

    return chunks;
}

// --- Auto-split ---

function exceedsWarning(content: string): boolean {
    return (
        content.split("\n").length > DEFAULT_THRESHOLDS.warningLines ||
        content.length > DEFAULT_THRESHOLDS.warningChars
    );
}

function splitByHeadings(content: string): { title: string; content: string }[] | null {
    const lines = content.split("\n");
    const sections: { title: string; content: string }[] = [];
    let currentTitle = "";
    let currentLines: string[] = [];

    for (const line of lines) {
        const match = line.match(/^(#{1,3})\s+(.+)$/);
        if (match) {
            const prev = currentLines.join("\n").trim();
            if (prev) sections.push({ title: currentTitle, content: prev });
            currentTitle = match[2]!;
            currentLines = [];
        } else {
            currentLines.push(line);
        }
    }
    const last = currentLines.join("\n").trim();
    if (last) sections.push({ title: currentTitle, content: last });

    return sections.length >= 2 ? sections : null;
}

function addChunkWithAutoSplit(chunks: DiscoveredChunk[], chunk: DiscoveredChunk): void {
    if (!exceedsWarning(chunk.content)) {
        chunks.push(chunk);
        return;
    }

    const sections = splitByHeadings(chunk.content);
    if (!sections) {
        chunks.push(chunk);
        return;
    }

    // Create index chunk
    const indexContent = sections.map(s => `- ${s.title || "(intro)"}`).join("\n");
    chunks.push({ ...chunk, content: `Sections:\n\n${indexContent}` });

    // Create sub-chunks
    for (const section of sections) {
        const sectionTitle = section.title || `${chunk.title} (intro)`;
        chunks.push({
            title: sectionTitle,
            content: section.content,
            type: chunk.type,
            tags: chunk.tags,
            tier: 1,
            category: "documents",
            source: chunk.source,
        });
    }
}

// --- Helpers ---

function extractMarkdownTitle(content: string): string | null {
    const match = content.match(/^#\s+(.+)$/m);
    return match?.[1]?.trim() ?? null;
}

function pathTags(relPath: string): string[] {
    const parts = relPath.split("/").filter(Boolean);
    return parts
        .filter(p => !["src", "lib", "index.ts", "package.json"].includes(p))
        .slice(0, 3);
}

function findFiles(dir: string, ext: string, maxDepth = 5, depth = 0): string[] {
    if (depth >= maxDepth) return [];
    const results: string[] = [];
    try {
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            if (entry.name.startsWith(".") || IGNORE_DIRS.has(entry.name)) continue;
            const full = join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...findFiles(full, ext, maxDepth, depth + 1));
            } else if (entry.name.endsWith(ext)) {
                results.push(full);
            }
        }
    } catch {
        // permission errors etc
    }
    return results;
}
```

- [ ] **Step 4: Update scanner.ts to delegate to tier1-docs.ts**

Replace the duplicated logic in `scanner.ts` with an import from tier1-docs. The `ScannedChunk` type and `scanProject` function signature remain unchanged for backward compatibility:

```typescript
// apps/cli/src/lib/scanner.ts
// Replace the ENTIRE file content with:

import { scanDocs } from "./setup/tier1-docs";

export interface ScannedChunk {
    title: string;
    content: string;
    type: string;
    tags: string[];
    folder: string;
    isIndex?: boolean;
    parentTitle?: string;
}

interface ScanOptions {
    dir: string;
    verbose?: boolean;
}

export function scanProject(opts: ScanOptions): ScannedChunk[] {
    const discovered = scanDocs(opts.dir);

    // Convert DiscoveredChunk to ScannedChunk for backward compat
    return discovered.map(d => ({
        title: d.title,
        content: d.content,
        type: d.type,
        tags: d.tags,
        folder: ".",
        isIndex: false,
    }));
}
```

- [ ] **Step 5: Run the tier1-docs test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tier1-docs.test.ts`
Expected: PASS — all 4 tests green.

- [ ] **Step 6: Run existing tests to verify no regressions**

Run: `cd apps/cli && pnpm vitest run`
Expected: All existing tests pass (including the init/scan behavior via `scanner.ts`).

- [ ] **Step 7: Commit**

```bash
git add apps/cli/src/lib/setup/tier1-docs.ts apps/cli/src/__tests__/setup/tier1-docs.test.ts apps/cli/src/lib/scanner.ts
git commit -m "refactor(cli): extract doc scanning to tier1-docs.ts for reuse"
```

---

## Task 3: Tier 2 — Project Metadata Scanner

**Files:**
- Create: `apps/cli/src/lib/setup/tier2-metadata.ts`
- Create: `apps/cli/src/__tests__/setup/tier2-metadata.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// apps/cli/src/__tests__/setup/tier2-metadata.test.ts

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { rmSync } from "node:fs";

import { scanMetadata } from "../../lib/setup/tier2-metadata";

const TMP_DIR = join(import.meta.dirname, "__tmp_tier2__");

beforeEach(() => {
    mkdirSync(TMP_DIR, { recursive: true });
});

afterEach(() => {
    rmSync(TMP_DIR, { recursive: true, force: true });
});

describe("scanMetadata", () => {
    it("extracts tech stack from package.json", () => {
        writeFileSync(
            join(TMP_DIR, "package.json"),
            JSON.stringify({
                name: "my-app",
                dependencies: { react: "^18.0.0", "next": "^14.0.0" },
                devDependencies: { vitest: "^1.0.0", typescript: "^5.0.0" },
            })
        );
        const { chunks, tips } = scanMetadata(TMP_DIR);
        const techChunk = chunks.find(c => c.title.includes("Tech Stack"));
        expect(techChunk).toBeDefined();
        expect(techChunk!.tier).toBe(2);
        expect(techChunk!.category).toBe("tech-stack");
        expect(techChunk!.content).toContain("react");
        expect(techChunk!.content).toContain("next");
        expect(techChunk!.tags).toContain("framework");
    });

    it("detects monorepo from workspaces field", () => {
        writeFileSync(
            join(TMP_DIR, "package.json"),
            JSON.stringify({
                name: "my-monorepo",
                workspaces: ["apps/*", "packages/*"],
            })
        );
        mkdirSync(join(TMP_DIR, "apps", "web"), { recursive: true });
        writeFileSync(join(TMP_DIR, "apps", "web", "package.json"), JSON.stringify({ name: "@mono/web" }));
        mkdirSync(join(TMP_DIR, "packages", "shared"), { recursive: true });
        writeFileSync(join(TMP_DIR, "packages", "shared", "package.json"), JSON.stringify({ name: "@mono/shared" }));

        const { chunks } = scanMetadata(TMP_DIR);
        const structureChunk = chunks.find(c => c.category === "structure");
        expect(structureChunk).toBeDefined();
        expect(structureChunk!.content).toContain("monorepo");
    });

    it("extracts tsconfig info", () => {
        writeFileSync(join(TMP_DIR, "package.json"), JSON.stringify({ name: "app" }));
        writeFileSync(
            join(TMP_DIR, "tsconfig.json"),
            JSON.stringify({ compilerOptions: { strict: true, target: "ES2022", paths: { "@/*": ["./src/*"] } } })
        );
        const { chunks } = scanMetadata(TMP_DIR);
        const tsChunk = chunks.find(c => c.title.includes("TypeScript"));
        expect(tsChunk).toBeDefined();
        expect(tsChunk!.content).toContain("strict");
        expect(tsChunk!.category).toBe("config");
    });

    it("extracts env vars from .env.example (never .env)", () => {
        writeFileSync(join(TMP_DIR, "package.json"), JSON.stringify({ name: "app" }));
        writeFileSync(join(TMP_DIR, ".env.example"), "DATABASE_URL=\nAPI_KEY=\nPORT=3000\n");
        writeFileSync(join(TMP_DIR, ".env"), "DATABASE_URL=postgres://secret\nAPI_KEY=sk-secret\n");
        const { chunks } = scanMetadata(TMP_DIR);
        const envChunk = chunks.find(c => c.title.includes("Environment"));
        expect(envChunk).toBeDefined();
        expect(envChunk!.content).toContain("DATABASE_URL");
        expect(envChunk!.content).not.toContain("secret");
        expect(envChunk!.type).toBe("schema");
    });

    it("returns empty for project with no package.json", () => {
        const { chunks } = scanMetadata(TMP_DIR);
        expect(chunks).toEqual([]);
    });

    it("detects CI from github workflows", () => {
        writeFileSync(join(TMP_DIR, "package.json"), JSON.stringify({ name: "app" }));
        mkdirSync(join(TMP_DIR, ".github", "workflows"), { recursive: true });
        writeFileSync(
            join(TMP_DIR, ".github", "workflows", "ci.yml"),
            "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: npm test\n"
        );
        const { chunks } = scanMetadata(TMP_DIR);
        const ciChunk = chunks.find(c => c.tags.includes("ci"));
        expect(ciChunk).toBeDefined();
        expect(ciChunk!.category).toBe("config");
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tier2-metadata.test.ts`
Expected: FAIL — `scanMetadata` does not exist.

- [ ] **Step 3: Implement tier2-metadata.ts**

```typescript
// apps/cli/src/lib/setup/tier2-metadata.ts

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";

import type { DiscoveredChunk, Tip } from "./types";

interface MetadataResult {
    chunks: DiscoveredChunk[];
    tips: Tip[];
}

/**
 * Scan project config files to produce metadata chunks.
 * Reads package.json, tsconfig, env examples, Docker, CI, and build config.
 */
export function scanMetadata(dir: string): MetadataResult {
    const chunks: DiscoveredChunk[] = [];
    const tips: Tip[] = [];

    const pkg = readJson<PackageJson>(join(dir, "package.json"));
    if (!pkg) return { chunks, tips };

    // Tech stack from package.json
    const techChunk = buildTechStackChunk(pkg, dir);
    if (techChunk) chunks.push(techChunk);

    // Monorepo structure
    const structureChunk = buildStructureChunk(pkg, dir);
    if (structureChunk) chunks.push(structureChunk);

    // TypeScript config
    const tsChunk = buildTsConfigChunk(dir);
    if (tsChunk) chunks.push(tsChunk);

    // Environment variables
    const envChunk = buildEnvChunk(dir);
    if (envChunk) chunks.push(envChunk);

    // Docker / docker-compose
    const dockerChunk = buildDockerChunk(dir);
    if (dockerChunk) chunks.push(dockerChunk);
    else if (existsSync(join(dir, "Dockerfile")) || existsSync(join(dir, "docker-compose.yml"))) {
        tips.push({ title: "Docker", detail: "Dockerfile or docker-compose.yml found but could not be parsed" });
    }

    // Build pipeline (turbo / nx)
    const buildChunk = buildPipelineChunk(dir);
    if (buildChunk) chunks.push(buildChunk);

    // CI/CD
    const ciChunk = buildCiChunk(dir);
    if (ciChunk) chunks.push(ciChunk);

    return { chunks, tips };
}

// --- Package.json types ---

interface PackageJson {
    name?: string;
    workspaces?: string[] | { packages: string[] };
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
    scripts?: Record<string, string>;
}

// --- Chunk builders ---

function buildTechStackChunk(pkg: PackageJson, dir: string): DiscoveredChunk | null {
    const deps = { ...pkg.dependencies };
    const devDeps = { ...pkg.devDependencies };
    const allDeps = { ...deps, ...devDeps };
    if (Object.keys(allDeps).length === 0) return null;

    const lines: string[] = [];
    const tags: string[] = ["tech-stack"];

    // Detect frameworks
    const frameworks = detectFrameworks(allDeps);
    if (frameworks.length > 0) {
        lines.push(`## Frameworks\n`);
        for (const f of frameworks) lines.push(`- **${f.name}** ${f.version}`);
        lines.push("");
        tags.push("framework");
    }

    // Detect key libraries (DB, auth, etc.)
    const libraries = detectKeyLibraries(allDeps);
    if (libraries.length > 0) {
        lines.push(`## Key Libraries\n`);
        for (const l of libraries) lines.push(`- **${l.name}** (${l.category}) ${l.version}`);
        lines.push("");
    }

    // Dev tooling
    const devTools = detectDevTools(devDeps);
    if (devTools.length > 0) {
        lines.push(`## Dev Tooling\n`);
        for (const t of devTools) lines.push(`- **${t.name}** ${t.version}`);
        lines.push("");
        tags.push("tooling");
    }

    if (lines.length === 0) return null;

    return {
        title: `Tech Stack — ${pkg.name ?? basename(dir)}`,
        content: lines.join("\n").trim(),
        type: "reference",
        tags,
        tier: 2,
        category: "tech-stack",
        source: "package.json",
    };
}

function buildStructureChunk(pkg: PackageJson, dir: string): DiscoveredChunk | null {
    const workspaceGlobs = Array.isArray(pkg.workspaces)
        ? pkg.workspaces
        : pkg.workspaces?.packages;
    if (!workspaceGlobs) return null;

    // Find actual workspace packages
    const packages: { name: string; path: string }[] = [];
    for (const glob of workspaceGlobs) {
        const baseDir = glob.replace(/\/?\*$/, "");
        const fullBase = join(dir, baseDir);
        if (!existsSync(fullBase) || !statSync(fullBase).isDirectory()) continue;
        try {
            for (const entry of readdirSync(fullBase, { withFileTypes: true })) {
                if (!entry.isDirectory()) continue;
                const pkgPath = join(fullBase, entry.name, "package.json");
                if (existsSync(pkgPath)) {
                    const sub = readJson<{ name?: string }>(pkgPath);
                    packages.push({
                        name: sub?.name ?? entry.name,
                        path: relative(dir, join(fullBase, entry.name)),
                    });
                }
            }
        } catch {
            // permission errors
        }
    }

    if (packages.length === 0) return null;

    const lines = [
        `This is a monorepo with ${packages.length} packages.\n`,
        `## Workspace Globs\n`,
        ...workspaceGlobs.map(g => `- \`${g}\``),
        "",
        `## Packages\n`,
        ...packages.map(p => `- **${p.name}** — \`${p.path}\``),
    ];

    return {
        title: "Project Structure (Monorepo)",
        content: lines.join("\n").trim(),
        type: "reference",
        tags: ["structure", "monorepo"],
        tier: 2,
        category: "structure",
        source: "package.json",
    };
}

function buildTsConfigChunk(dir: string): DiscoveredChunk | null {
    const tsconfig = readJson<TsConfig>(join(dir, "tsconfig.json"))
        ?? readJson<TsConfig>(join(dir, "jsconfig.json"));
    if (!tsconfig) return null;

    const opts = tsconfig.compilerOptions ?? {};
    const lines: string[] = [];

    if (opts.strict !== undefined) lines.push(`- **Strict mode:** ${opts.strict ? "enabled" : "disabled"}`);
    if (opts.target) lines.push(`- **Target:** ${opts.target}`);
    if (opts.module) lines.push(`- **Module:** ${opts.module}`);
    if (opts.moduleResolution) lines.push(`- **Module resolution:** ${opts.moduleResolution}`);
    if (opts.paths) {
        const aliases = Object.entries(opts.paths).map(([k, v]) => `\`${k}\` → \`${(v as string[])[0]}\``);
        lines.push(`- **Path aliases:** ${aliases.join(", ")}`);
    }
    if (opts.jsx) lines.push(`- **JSX:** ${opts.jsx}`);

    if (lines.length === 0) return null;

    return {
        title: "TypeScript Configuration",
        content: lines.join("\n"),
        type: "reference",
        tags: ["config", "typescript"],
        tier: 2,
        category: "config",
        source: existsSync(join(dir, "tsconfig.json")) ? "tsconfig.json" : "jsconfig.json",
    };
}

function buildEnvChunk(dir: string): DiscoveredChunk | null {
    // Only read example files — NEVER .env
    const envFile = [".env.example", ".env.local.example", ".env.sample"]
        .find(f => existsSync(join(dir, f)));
    if (!envFile) return null;

    const content = readFileSync(join(dir, envFile), "utf-8");
    const vars = content
        .split("\n")
        .filter(line => line.trim() && !line.startsWith("#"))
        .map(line => {
            const [key] = line.split("=");
            const comment = line.includes("#") ? line.split("#").slice(1).join("#").trim() : undefined;
            return { key: key!.trim(), comment };
        })
        .filter(v => v.key);

    if (vars.length === 0) return null;

    const lines = [
        `Environment variables required by this project (from \`${envFile}\`):\n`,
        ...vars.map(v => v.comment ? `- \`${v.key}\` — ${v.comment}` : `- \`${v.key}\``),
    ];

    return {
        title: "Environment Variables",
        content: lines.join("\n"),
        type: "schema",
        tags: ["config", "environment"],
        tier: 2,
        category: "config",
        source: envFile,
    };
}

function buildDockerChunk(dir: string): DiscoveredChunk | null {
    const lines: string[] = [];

    // docker-compose.yml
    const composePath = ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"]
        .find(f => existsSync(join(dir, f)));
    if (composePath) {
        const content = readFileSync(join(dir, composePath), "utf-8");
        // Extract service names from YAML (simple regex, no full parser)
        const services = [...content.matchAll(/^\s{2}(\w[\w-]*):\s*$/gm)].map(m => m[1]);
        if (services.length > 0) {
            lines.push(`## Docker Compose Services\n`);
            for (const svc of services) lines.push(`- \`${svc}\``);
            lines.push("");
        }
    }

    // Dockerfile
    if (existsSync(join(dir, "Dockerfile"))) {
        const content = readFileSync(join(dir, "Dockerfile"), "utf-8");
        const stages = [...content.matchAll(/^FROM\s+\S+(?:\s+AS\s+(\S+))?/gmi)]
            .map(m => m[1] ?? "base");
        if (stages.length > 0) {
            lines.push(`## Dockerfile Stages\n`);
            for (const stage of stages) lines.push(`- \`${stage}\``);
        }
    }

    if (lines.length === 0) return null;

    return {
        title: "Infrastructure (Docker)",
        content: lines.join("\n").trim(),
        type: "reference",
        tags: ["infrastructure", "docker"],
        tier: 2,
        category: "config",
        source: composePath ?? "Dockerfile",
    };
}

function buildPipelineChunk(dir: string): DiscoveredChunk | null {
    // Turbo
    const turboConfig = readJson<TurboConfig>(join(dir, "turbo.json"));
    if (turboConfig) {
        const tasks = Object.keys(turboConfig.tasks ?? turboConfig.pipeline ?? {});
        if (tasks.length === 0) return null;
        const lines = [
            `Build pipeline managed by **Turborepo**.\n`,
            `## Tasks\n`,
            ...tasks.map(t => `- \`${t}\``),
        ];
        return {
            title: "Build Pipeline (Turborepo)",
            content: lines.join("\n"),
            type: "reference",
            tags: ["config", "build", "turborepo"],
            tier: 2,
            category: "config",
            source: "turbo.json",
        };
    }

    // Nx
    const nxConfig = readJson<Record<string, unknown>>(join(dir, "nx.json"));
    if (nxConfig) {
        return {
            title: "Build Pipeline (Nx)",
            content: "Build pipeline managed by **Nx**.",
            type: "reference",
            tags: ["config", "build", "nx"],
            tier: 2,
            category: "config",
            source: "nx.json",
        };
    }

    return null;
}

function buildCiChunk(dir: string): DiscoveredChunk | null {
    const lines: string[] = [];
    let source = "";

    // GitHub Actions
    const ghDir = join(dir, ".github", "workflows");
    if (existsSync(ghDir) && statSync(ghDir).isDirectory()) {
        source = ".github/workflows/";
        const workflows = readdirSync(ghDir).filter(f => f.endsWith(".yml") || f.endsWith(".yaml"));
        if (workflows.length > 0) {
            lines.push(`CI/CD via **GitHub Actions**.\n`);
            lines.push(`## Workflows\n`);
            for (const w of workflows) {
                const content = readFileSync(join(ghDir, w), "utf-8");
                const nameMatch = content.match(/^name:\s*(.+)$/m);
                const name = nameMatch?.[1]?.trim() ?? w;
                lines.push(`- **${name}** (\`${w}\`)`);
            }
        }
    }

    // GitLab CI
    if (existsSync(join(dir, ".gitlab-ci.yml"))) {
        source = ".gitlab-ci.yml";
        lines.push("CI/CD via **GitLab CI**.");
    }

    if (lines.length === 0) return null;

    return {
        title: "CI/CD Configuration",
        content: lines.join("\n").trim(),
        type: "reference",
        tags: ["config", "ci"],
        tier: 2,
        category: "config",
        source,
    };
}

// --- Detection helpers ---

interface FrameworkInfo { name: string; version: string }
interface LibraryInfo { name: string; version: string; category: string }

const FRAMEWORK_PATTERNS: [string, string][] = [
    ["next", "Next.js"], ["react", "React"], ["vue", "Vue"], ["svelte", "Svelte"],
    ["@angular/core", "Angular"], ["elysia", "Elysia"], ["express", "Express"],
    ["fastify", "Fastify"], ["hono", "Hono"], ["nuxt", "Nuxt"],
    ["astro", "Astro"], ["remix", "Remix"], ["solid-js", "SolidJS"],
];

const LIBRARY_PATTERNS: [string, string, string][] = [
    ["drizzle-orm", "Drizzle ORM", "database"],
    ["prisma", "Prisma", "database"],
    ["@prisma/client", "Prisma", "database"],
    ["mongoose", "Mongoose", "database"],
    ["better-auth", "Better Auth", "auth"],
    ["next-auth", "NextAuth", "auth"],
    ["@auth/core", "Auth.js", "auth"],
    ["tailwindcss", "Tailwind CSS", "styling"],
    ["effect", "Effect", "fp"],
    ["zod", "Zod", "validation"],
    ["trpc", "tRPC", "api"],
    ["@trpc/server", "tRPC", "api"],
];

const DEV_TOOL_PATTERNS: [string, string][] = [
    ["vitest", "Vitest"], ["jest", "Jest"], ["mocha", "Mocha"],
    ["typescript", "TypeScript"], ["eslint", "ESLint"], ["prettier", "Prettier"],
    ["biome", "Biome"], ["tsup", "tsup"], ["esbuild", "esbuild"],
    ["vite", "Vite"], ["webpack", "Webpack"], ["rollup", "Rollup"],
];

function detectFrameworks(deps: Record<string, string>): FrameworkInfo[] {
    const found: FrameworkInfo[] = [];
    for (const [pkg, label] of FRAMEWORK_PATTERNS) {
        if (deps[pkg]) found.push({ name: label, version: deps[pkg]! });
    }
    return found;
}

function detectKeyLibraries(deps: Record<string, string>): LibraryInfo[] {
    const found: LibraryInfo[] = [];
    const seen = new Set<string>();
    for (const [pkg, label, cat] of LIBRARY_PATTERNS) {
        if (deps[pkg] && !seen.has(label)) {
            found.push({ name: label, version: deps[pkg]!, category: cat });
            seen.add(label);
        }
    }
    return found;
}

function detectDevTools(devDeps: Record<string, string>): FrameworkInfo[] {
    const found: FrameworkInfo[] = [];
    for (const [pkg, label] of DEV_TOOL_PATTERNS) {
        if (devDeps[pkg]) found.push({ name: label, version: devDeps[pkg]! });
    }
    return found;
}

// --- Utility ---

interface TsConfig {
    compilerOptions?: {
        strict?: boolean;
        target?: string;
        module?: string;
        moduleResolution?: string;
        paths?: Record<string, string[]>;
        jsx?: string;
    };
}

interface TurboConfig {
    tasks?: Record<string, unknown>;
    pipeline?: Record<string, unknown>;
}

function readJson<T>(path: string): T | null {
    try {
        if (!existsSync(path)) return null;
        const raw = readFileSync(path, "utf-8");
        return JSON.parse(raw) as T;
    } catch {
        return null;
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tier2-metadata.test.ts`
Expected: PASS — all 6 tests green.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/lib/setup/tier2-metadata.ts apps/cli/src/__tests__/setup/tier2-metadata.test.ts
git commit -m "feat(cli): add tier 2 project metadata scanner"
```

---

## Task 4: Tier 3 — Code Pattern Detection

**Files:**
- Create: `apps/cli/src/lib/setup/tier3-patterns.ts`
- Create: `apps/cli/src/__tests__/setup/tier3-patterns.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// apps/cli/src/__tests__/setup/tier3-patterns.test.ts

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { rmSync } from "node:fs";

import { scanPatterns } from "../../lib/setup/tier3-patterns";

const TMP_DIR = join(import.meta.dirname, "__tmp_tier3__");

beforeEach(() => {
    mkdirSync(TMP_DIR, { recursive: true });
});

afterEach(() => {
    rmSync(TMP_DIR, { recursive: true, force: true });
});

/** Helper: write a package.json with given deps */
function writePkg(deps: Record<string, string> = {}, devDeps: Record<string, string> = {}) {
    writeFileSync(
        join(TMP_DIR, "package.json"),
        JSON.stringify({ name: "test", dependencies: deps, devDependencies: devDeps })
    );
}

describe("scanPatterns", () => {
    it("detects route structure when framework dep + routes/ dir exist", () => {
        writePkg({ elysia: "^1.0.0" });
        mkdirSync(join(TMP_DIR, "src", "routes"), { recursive: true });
        writeFileSync(join(TMP_DIR, "src", "routes", "users.ts"), "export const usersRoute = new Elysia()");
        const { chunks } = scanPatterns(TMP_DIR);
        const routeChunk = chunks.find(c => c.tags.includes("routing"));
        expect(routeChunk).toBeDefined();
        expect(routeChunk!.tier).toBe(3);
        expect(routeChunk!.category).toBe("conventions");
    });

    it("detects test patterns when test runner + test files exist", () => {
        writePkg({}, { vitest: "^1.0.0" });
        mkdirSync(join(TMP_DIR, "src", "__tests__"), { recursive: true });
        writeFileSync(join(TMP_DIR, "src", "__tests__", "app.test.ts"), "test('works', () => {})");
        const { chunks } = scanPatterns(TMP_DIR);
        const testChunk = chunks.find(c => c.tags.includes("testing"));
        expect(testChunk).toBeDefined();
        expect(testChunk!.content).toContain("vitest");
    });

    it("detects database when ORM dep + schema files exist", () => {
        writePkg({ "drizzle-orm": "^0.30.0" });
        mkdirSync(join(TMP_DIR, "src", "db"), { recursive: true });
        writeFileSync(join(TMP_DIR, "src", "db", "schema.ts"), "export const users = pgTable('users', {})");
        const { chunks } = scanPatterns(TMP_DIR);
        const dbChunk = chunks.find(c => c.tags.includes("database"));
        expect(dbChunk).toBeDefined();
    });

    it("emits a tip (not chunk) when only dep exists without file structure", () => {
        writePkg({ "drizzle-orm": "^0.30.0" });
        // No schema files created
        const { chunks, tips } = scanPatterns(TMP_DIR);
        expect(chunks.find(c => c.tags.includes("database"))).toBeUndefined();
        expect(tips.find(t => t.title.toLowerCase().includes("database"))).toBeDefined();
    });

    it("emits a tip when only file structure exists without dep", () => {
        writePkg({});
        mkdirSync(join(TMP_DIR, "src", "routes"), { recursive: true });
        writeFileSync(join(TMP_DIR, "src", "routes", "users.ts"), "export default {}");
        const { chunks, tips } = scanPatterns(TMP_DIR);
        expect(chunks.find(c => c.tags.includes("routing"))).toBeUndefined();
        expect(tips.find(t => t.title.toLowerCase().includes("rout"))).toBeDefined();
    });

    it("returns empty for project with no detectable patterns", () => {
        writePkg({});
        const { chunks, tips } = scanPatterns(TMP_DIR);
        expect(chunks).toEqual([]);
        expect(tips).toEqual([]);
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tier3-patterns.test.ts`
Expected: FAIL — `scanPatterns` does not exist.

- [ ] **Step 3: Implement tier3-patterns.ts**

```typescript
// apps/cli/src/lib/setup/tier3-patterns.ts

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import type { DiscoveredChunk, Tip } from "./types";

interface PatternResult {
    chunks: DiscoveredChunk[];
    tips: Tip[];
}

/**
 * Detect code patterns by checking for both a dependency AND matching file structure.
 * If only one signal is found, it becomes a tip instead of a chunk.
 */
export function scanPatterns(dir: string): PatternResult {
    const chunks: DiscoveredChunk[] = [];
    const tips: Tip[] = [];

    const pkg = readJson<PackageJson>(join(dir, "package.json"));
    if (!pkg) return { chunks, tips };

    const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };

    for (const detector of DETECTORS) {
        const hasDep = detector.deps.some(d => d in allDeps);
        const fileResult = detector.findFiles(dir);
        const hasFiles = fileResult.found;

        if (hasDep && hasFiles) {
            const depName = detector.deps.find(d => d in allDeps)!;
            chunks.push(detector.buildChunk(dir, depName, allDeps[depName]!, fileResult));
        } else if (hasDep && !hasFiles) {
            tips.push({
                title: detector.tipTitle,
                detail: `${detector.depLabel} dependency found but no matching file structure detected`,
            });
        } else if (!hasDep && hasFiles) {
            tips.push({
                title: detector.tipTitle,
                detail: `${detector.fileLabel} found but no matching dependency in package.json`,
            });
        }
    }

    return { chunks, tips };
}

// --- Types ---

interface PackageJson {
    name?: string;
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
}

interface FileDetectionResult {
    found: boolean;
    paths: string[];
    details?: string;
}

interface PatternDetector {
    deps: string[];
    depLabel: string;
    fileLabel: string;
    tipTitle: string;
    findFiles: (dir: string) => FileDetectionResult;
    buildChunk: (dir: string, depName: string, version: string, files: FileDetectionResult) => DiscoveredChunk;
}

// --- Detectors ---

const DETECTORS: PatternDetector[] = [
    // Route structure
    {
        deps: ["elysia", "express", "fastify", "hono", "next", "@angular/core", "vue-router", "react-router", "react-router-dom"],
        depLabel: "Web framework",
        fileLabel: "Route directories (routes/, pages/, api/)",
        tipTitle: "Route Structure",
        findFiles(dir) {
            const patterns = ["routes", "pages", "api"];
            const found: string[] = [];
            searchDirs(dir, (relPath, name) => {
                if (patterns.includes(name) && !relPath.includes("node_modules")) {
                    found.push(relPath);
                }
            });
            return { found: found.length > 0, paths: found };
        },
        buildChunk(dir, depName, version, files) {
            const pathList = files.paths.map(p => `\`${p}\``).join(", ");
            return {
                title: "Route Structure",
                content: `Routes are defined using **${depName}** (${version}).\n\nRoute directories: ${pathList}`,
                type: "convention",
                tags: ["routing", "convention"],
                tier: 3,
                category: "conventions",
                source: `pattern:routes`,
                appliesTo: files.paths.map(p => `${p}/**/*`),
            };
        },
    },

    // Test patterns
    {
        deps: ["vitest", "jest", "mocha", "@jest/core"],
        depLabel: "Test runner",
        fileLabel: "Test files (*.test.ts, *.spec.ts, __tests__/)",
        tipTitle: "Testing Patterns",
        findFiles(dir) {
            const found: string[] = [];
            searchDirs(dir, (relPath, name) => {
                if (name === "__tests__" && !relPath.includes("node_modules")) {
                    found.push(relPath);
                }
            });
            // Also look for *.test.ts / *.spec.ts files
            const testFiles = findFilesByPattern(dir, /\.(test|spec)\.(ts|tsx|js|jsx)$/);
            return { found: found.length > 0 || testFiles.length > 0, paths: found, details: testFiles.length > 0 ? `${testFiles.length} test files` : undefined };
        },
        buildChunk(dir, depName, version, files) {
            const lines = [`Tests use **${depName}** (${version}).`];
            if (files.paths.length > 0) {
                lines.push(`\nTest directories: ${files.paths.map(p => `\`${p}\``).join(", ")}`);
            }
            if (files.details) {
                lines.push(`\n${files.details} found across the project.`);
            }
            return {
                title: "Testing Conventions",
                content: lines.join("\n"),
                type: "convention",
                tags: ["testing", "convention"],
                tier: 3,
                category: "conventions",
                source: "pattern:testing",
            };
        },
    },

    // Database/ORM
    {
        deps: ["drizzle-orm", "@prisma/client", "prisma", "mongoose", "typeorm", "sequelize", "knex", "kysely"],
        depLabel: "ORM/Database",
        fileLabel: "Schema/migration files",
        tipTitle: "Database Patterns",
        findFiles(dir) {
            const found: string[] = [];
            // Look for schema files
            const schemaFiles = findFilesByPattern(dir, /schema\.(ts|js)$/);
            found.push(...schemaFiles.map(f => relative(dir, f)));
            // Look for migrations directory
            searchDirs(dir, (relPath, name) => {
                if ((name === "migrations" || name === "drizzle") && !relPath.includes("node_modules")) {
                    found.push(relPath);
                }
            });
            return { found: found.length > 0, paths: found };
        },
        buildChunk(dir, depName, version, files) {
            const pathList = files.paths.map(p => `\`${p}\``).join(", ");
            return {
                title: "Database Schema",
                content: `Database managed by **${depName}** (${version}).\n\nSchema/migration locations: ${pathList}`,
                type: "convention",
                tags: ["database", "convention"],
                tier: 3,
                category: "conventions",
                source: "pattern:database",
                appliesTo: files.paths.filter(p => !p.endsWith(".ts") && !p.endsWith(".js")).map(p => `${p}/**/*`),
            };
        },
    },

    // Component structure
    {
        deps: ["react", "vue", "svelte", "solid-js", "@angular/core", "preact"],
        depLabel: "UI framework",
        fileLabel: "Component directories (components/, features/)",
        tipTitle: "Component Structure",
        findFiles(dir) {
            const patterns = ["components", "features"];
            const found: string[] = [];
            searchDirs(dir, (relPath, name) => {
                if (patterns.includes(name) && !relPath.includes("node_modules")) {
                    found.push(relPath);
                }
            });
            return { found: found.length > 0, paths: found };
        },
        buildChunk(dir, depName, version, files) {
            const pathList = files.paths.map(p => `\`${p}\``).join(", ");
            return {
                title: "Component Structure",
                content: `UI built with **${depName}** (${version}).\n\nComponent directories: ${pathList}`,
                type: "reference",
                tags: ["components", "ui"],
                tier: 3,
                category: "structure",
                source: "pattern:components",
                appliesTo: files.paths.map(p => `${p}/**/*`),
            };
        },
    },

    // Auth
    {
        deps: ["better-auth", "next-auth", "@auth/core", "passport", "lucia"],
        depLabel: "Auth library",
        fileLabel: "Auth config files (auth.ts, auth/)",
        tipTitle: "Authentication",
        findFiles(dir) {
            const found: string[] = [];
            const authFiles = findFilesByPattern(dir, /auth\.(ts|js|config\.(ts|js))$/);
            found.push(...authFiles.map(f => relative(dir, f)));
            searchDirs(dir, (relPath, name) => {
                if (name === "auth" && !relPath.includes("node_modules")) {
                    found.push(relPath);
                }
            });
            return { found: found.length > 0, paths: found };
        },
        buildChunk(dir, depName, version, files) {
            const pathList = files.paths.map(p => `\`${p}\``).join(", ");
            return {
                title: "Authentication",
                content: `Authentication via **${depName}** (${version}).\n\nAuth files: ${pathList}`,
                type: "convention",
                tags: ["auth", "convention"],
                tier: 3,
                category: "conventions",
                source: "pattern:auth",
                appliesTo: files.paths.filter(p => !p.endsWith(".ts") && !p.endsWith(".js")).map(p => `${p}/**/*`),
            };
        },
    },
];

// --- File system helpers ---

/** Walk directories up to depth 4 and call visitor with (relativePath, dirName) */
function searchDirs(dir: string, visitor: (relPath: string, name: string) => void, maxDepth = 4, depth = 0, base?: string): void {
    if (depth >= maxDepth) return;
    const root = base ?? dir;
    try {
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            if (!entry.isDirectory()) continue;
            if (entry.name.startsWith(".") || IGNORE_DIRS.has(entry.name)) continue;
            const full = join(dir, entry.name);
            const rel = relative(root, full);
            visitor(rel, entry.name);
            searchDirs(full, visitor, maxDepth, depth + 1, root);
        }
    } catch {
        // permission errors
    }
}

/** Find files matching a regex pattern, max depth 5 */
function findFilesByPattern(dir: string, pattern: RegExp, maxDepth = 5, depth = 0): string[] {
    if (depth >= maxDepth) return [];
    const results: string[] = [];
    try {
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            if (entry.name.startsWith(".") || IGNORE_DIRS.has(entry.name)) continue;
            const full = join(dir, entry.name);
            if (entry.isDirectory()) {
                results.push(...findFilesByPattern(full, pattern, maxDepth, depth + 1));
            } else if (pattern.test(entry.name)) {
                results.push(full);
            }
        }
    } catch {
        // permission errors
    }
    return results;
}

const IGNORE_DIRS = new Set([
    "node_modules", ".git", ".turbo", "dist", "build",
    ".next", ".output", ".cache", "coverage", ".fubbik",
]);

function readJson<T>(path: string): T | null {
    try {
        if (!existsSync(path)) return null;
        return JSON.parse(readFileSync(path, "utf-8")) as T;
    } catch {
        return null;
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tier3-patterns.test.ts`
Expected: PASS — all 6 tests green.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/lib/setup/tier3-patterns.ts apps/cli/src/__tests__/setup/tier3-patterns.test.ts
git commit -m "feat(cli): add tier 3 code pattern scanner"
```

---

## Task 5: Connection Inference

**Files:**
- Create: `apps/cli/src/lib/setup/connections.ts`
- Create: `apps/cli/src/__tests__/setup/connections.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// apps/cli/src/__tests__/setup/connections.test.ts

import { describe, expect, it } from "vitest";

import { inferConnections } from "../../lib/setup/connections";
import type { DiscoveredChunk } from "../../lib/setup/types";

function makeChunk(overrides: Partial<DiscoveredChunk> & { title: string }): DiscoveredChunk {
    return {
        content: "",
        type: "reference",
        tags: [],
        tier: 1,
        category: "documents",
        source: "test",
        ...overrides,
    };
}

describe("inferConnections", () => {
    it("creates references connections for markdown links between tier-1 docs", () => {
        const chunks = [
            makeChunk({ title: "README", content: "See [Guide](./docs/guide.md) for details" }),
            makeChunk({ title: "Guide", source: "docs/guide.md", content: "The guide" }),
        ];
        const connections = inferConnections(chunks);
        expect(connections).toContainEqual({
            sourceTitle: "README",
            targetTitle: "Guide",
            relation: "references",
        });
    });

    it("creates part_of connections for monorepo packages", () => {
        const chunks = [
            makeChunk({ title: "Project Structure (Monorepo)", tier: 2, category: "structure", content: "monorepo with 2 packages" }),
            makeChunk({ title: "Tech Stack — @mono/web", tier: 2, category: "tech-stack", content: "web app" }),
        ];
        const connections = inferConnections(chunks);
        expect(connections).toContainEqual({
            sourceTitle: "Tech Stack — @mono/web",
            targetTitle: "Project Structure (Monorepo)",
            relation: "part_of",
        });
    });

    it("creates depends_on between routes and database", () => {
        const chunks = [
            makeChunk({ title: "Route Structure", tier: 3, category: "conventions", tags: ["routing"] }),
            makeChunk({ title: "Database Schema", tier: 3, category: "conventions", tags: ["database"] }),
        ];
        const connections = inferConnections(chunks);
        expect(connections).toContainEqual({
            sourceTitle: "Route Structure",
            targetTitle: "Database Schema",
            relation: "depends_on",
        });
    });

    it("creates supports connections when tier2/3 keywords appear in tier1 content", () => {
        const chunks = [
            makeChunk({ title: "README", tier: 1, content: "This project uses Drizzle ORM for database access" }),
            makeChunk({ title: "Database Schema", tier: 3, category: "conventions", tags: ["database"], content: "drizzle-orm" }),
        ];
        const connections = inferConnections(chunks);
        expect(connections).toContainEqual({
            sourceTitle: "Database Schema",
            targetTitle: "README",
            relation: "supports",
        });
    });

    it("returns empty for unrelated chunks", () => {
        const chunks = [
            makeChunk({ title: "README", content: "A simple hello world project" }),
            makeChunk({ title: "CI/CD Configuration", tier: 2, category: "config", content: "GitHub Actions" }),
        ];
        const connections = inferConnections(chunks);
        // CI/CD isn't mentioned in README content, so no supports connection
        expect(connections.filter(c => c.relation === "supports")).toEqual([]);
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/connections.test.ts`
Expected: FAIL — `inferConnections` does not exist.

- [ ] **Step 3: Implement connections.ts**

```typescript
// apps/cli/src/lib/setup/connections.ts

import type { DiscoveredChunk, DiscoveredConnection } from "./types";

/**
 * Infer connections between discovered chunks based on:
 * 1. Markdown links between tier-1 docs
 * 2. Workspace packages → monorepo structure chunk (part_of)
 * 3. Known dependency patterns between tier-3 detections (depends_on)
 * 4. Keyword matching: tier-2/3 chunk title keywords in tier-1 content (supports)
 */
export function inferConnections(chunks: DiscoveredChunk[]): DiscoveredConnection[] {
    const connections: DiscoveredConnection[] = [];
    const seen = new Set<string>();

    const addUnique = (conn: DiscoveredConnection) => {
        const key = `${conn.sourceTitle}→${conn.targetTitle}→${conn.relation}`;
        if (seen.has(key)) return;
        // Don't create self-references
        if (conn.sourceTitle === conn.targetTitle) return;
        seen.add(key);
        connections.push(conn);
    };

    // 1. Markdown links between tier-1 docs
    const tier1 = chunks.filter(c => c.tier === 1);
    for (const chunk of tier1) {
        const links = extractMarkdownLinks(chunk.content);
        for (const link of links) {
            // Match link target to another chunk's source path
            const target = tier1.find(c => c.source === normalizeLink(link) && c.title !== chunk.title);
            if (target) {
                addUnique({ sourceTitle: chunk.title, targetTitle: target.title, relation: "references" });
            }
        }
    }

    // 2. Workspace packages part_of monorepo structure
    const structureChunk = chunks.find(c => c.title === "Project Structure (Monorepo)");
    if (structureChunk) {
        const tier2TechChunks = chunks.filter(c => c.tier === 2 && c.category === "tech-stack" && c.title !== structureChunk.title);
        for (const tech of tier2TechChunks) {
            addUnique({ sourceTitle: tech.title, targetTitle: structureChunk.title, relation: "part_of" });
        }
    }

    // 3. Known dependency patterns between tier-3 detections
    const tier3ByTag = new Map<string, DiscoveredChunk>();
    for (const chunk of chunks.filter(c => c.tier === 3)) {
        for (const tag of chunk.tags) {
            tier3ByTag.set(tag, chunk);
        }
    }

    // Routes depend on database
    const routeChunk = tier3ByTag.get("routing");
    const dbChunk = tier3ByTag.get("database");
    if (routeChunk && dbChunk) {
        addUnique({ sourceTitle: routeChunk.title, targetTitle: dbChunk.title, relation: "depends_on" });
    }

    // Routes depend on auth
    const authChunk = tier3ByTag.get("auth");
    if (routeChunk && authChunk) {
        addUnique({ sourceTitle: routeChunk.title, targetTitle: authChunk.title, relation: "depends_on" });
    }

    // 4. Keyword matching: tier-2/3 chunk titles in tier-1 content
    const higherTier = chunks.filter(c => c.tier === 2 || c.tier === 3);
    for (const higher of higherTier) {
        const keywords = extractKeywords(higher);
        if (keywords.length === 0) continue;

        for (const doc of tier1) {
            const contentLower = doc.content.toLowerCase();
            const matched = keywords.some(kw => contentLower.includes(kw.toLowerCase()));
            if (matched) {
                addUnique({ sourceTitle: higher.title, targetTitle: doc.title, relation: "supports" });
            }
        }
    }

    return connections;
}

/** Extract markdown link paths: [text](path) */
function extractMarkdownLinks(content: string): string[] {
    const matches = [...content.matchAll(/\[.*?\]\(([^)]+)\)/g)];
    return matches.map(m => m[1]!).filter(link => !link.startsWith("http"));
}

/** Normalize a relative link path: ./docs/guide.md → docs/guide.md */
function normalizeLink(link: string): string {
    return link.replace(/^\.\//, "").replace(/#.*$/, "");
}

/** Extract meaningful keywords from a chunk for matching against doc content */
function extractKeywords(chunk: DiscoveredChunk): string[] {
    const keywords: string[] = [];

    // Extract dependency/framework names from content (bold text like **Drizzle ORM**)
    const boldMatches = [...chunk.content.matchAll(/\*\*([^*]+)\*\*/g)];
    for (const m of boldMatches) {
        const text = m[1]!.trim();
        if (text.length >= 3 && text.length <= 40) {
            keywords.push(text);
        }
    }

    // Use specific tags as keywords (skip generic ones)
    const genericTags = new Set(["convention", "config", "tech-stack", "structure", "reference"]);
    for (const tag of chunk.tags) {
        if (!genericTags.has(tag) && tag.length >= 3) {
            keywords.push(tag);
        }
    }

    return keywords;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/connections.test.ts`
Expected: PASS — all 5 tests green.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/lib/setup/connections.ts apps/cli/src/__tests__/setup/connections.test.ts
git commit -m "feat(cli): add connection inference for setup discovery"
```

---

## Task 6: Tips Generation

**Files:**
- Create: `apps/cli/src/lib/setup/tips.ts`
- Create: `apps/cli/src/__tests__/setup/tips.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// apps/cli/src/__tests__/setup/tips.test.ts

import { describe, expect, it } from "vitest";

import { mergeTips } from "../../lib/setup/tips";
import type { Tip } from "../../lib/setup/types";

describe("mergeTips", () => {
    it("deduplicates tips with the same title", () => {
        const tips: Tip[] = [
            { title: "Testing Patterns", detail: "vitest dep found" },
            { title: "Testing Patterns", detail: "test files found" },
        ];
        const merged = mergeTips(tips);
        expect(merged.length).toBe(1);
        expect(merged[0]!.title).toBe("Testing Patterns");
    });

    it("keeps tips with different titles", () => {
        const tips: Tip[] = [
            { title: "Testing Patterns", detail: "vitest found" },
            { title: "Database Patterns", detail: "drizzle found" },
        ];
        const merged = mergeTips(tips);
        expect(merged.length).toBe(2);
    });

    it("returns empty for empty input", () => {
        expect(mergeTips([])).toEqual([]);
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tips.test.ts`
Expected: FAIL — `mergeTips` does not exist.

- [ ] **Step 3: Implement tips.ts**

```typescript
// apps/cli/src/lib/setup/tips.ts

import type { Tip } from "./types";

/**
 * Merge and deduplicate tips from tier 2 and tier 3 scanners.
 * Tips with the same title are merged — the first detail is kept.
 */
export function mergeTips(tips: Tip[]): Tip[] {
    const seen = new Map<string, Tip>();
    for (const tip of tips) {
        if (!seen.has(tip.title)) {
            seen.set(tip.title, tip);
        }
    }
    return [...seen.values()];
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/tips.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/lib/setup/tips.ts apps/cli/src/__tests__/setup/tips.test.ts
git commit -m "feat(cli): add tip deduplication for setup command"
```

---

## Task 7: Discovery Orchestrator

**Files:**
- Create: `apps/cli/src/lib/setup/discover.ts`
- Create: `apps/cli/src/__tests__/setup/discover.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// apps/cli/src/__tests__/setup/discover.test.ts

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { rmSync } from "node:fs";

import { discover } from "../../lib/setup/discover";

const TMP_DIR = join(import.meta.dirname, "__tmp_discover__");

beforeEach(() => {
    mkdirSync(TMP_DIR, { recursive: true });
});

afterEach(() => {
    rmSync(TMP_DIR, { recursive: true, force: true });
});

describe("discover", () => {
    it("combines all tiers into a single result", () => {
        // Tier 1: a README
        writeFileSync(join(TMP_DIR, "README.md"), "# My App\n\nHello world");
        // Tier 2: package.json
        writeFileSync(
            join(TMP_DIR, "package.json"),
            JSON.stringify({
                name: "my-app",
                dependencies: { react: "^18.0.0" },
                devDependencies: { vitest: "^1.0.0" },
            })
        );
        // Tier 3: test files + dep to trigger full detection
        mkdirSync(join(TMP_DIR, "src", "__tests__"), { recursive: true });
        writeFileSync(join(TMP_DIR, "src", "__tests__", "app.test.ts"), "test('works', () => {})");

        const result = discover(TMP_DIR, { name: "my-app", remoteUrl: null, localPath: TMP_DIR });

        expect(result.chunks.length).toBeGreaterThanOrEqual(3); // README + tech stack + testing
        expect(result.chunks.some(c => c.tier === 1)).toBe(true);
        expect(result.chunks.some(c => c.tier === 2)).toBe(true);
        expect(result.chunks.some(c => c.tier === 3)).toBe(true);
        expect(result.tags.length).toBeGreaterThan(0);
        expect(result.codebase.name).toBe("my-app");
    });

    it("returns empty result for empty directory", () => {
        const result = discover(TMP_DIR, { name: "empty", remoteUrl: null, localPath: TMP_DIR });
        expect(result.chunks).toEqual([]);
        expect(result.connections).toEqual([]);
        expect(result.tips).toEqual([]);
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/discover.test.ts`
Expected: FAIL — `discover` does not exist.

- [ ] **Step 3: Implement discover.ts**

```typescript
// apps/cli/src/lib/setup/discover.ts

import type { DiscoveryResult } from "./types";
import { inferConnections } from "./connections";
import { scanDocs } from "./tier1-docs";
import { scanMetadata } from "./tier2-metadata";
import { scanPatterns } from "./tier3-patterns";
import { mergeTips } from "./tips";

/**
 * Run all three discovery tiers and combine into a single result.
 */
export function discover(
    dir: string,
    codebase: { name: string; remoteUrl: string | null; localPath: string },
): DiscoveryResult {
    // Tier 1: Documents
    const tier1Chunks = scanDocs(dir);

    // Tier 2: Project metadata
    const { chunks: tier2Chunks, tips: tier2Tips } = scanMetadata(dir);

    // Tier 3: Code patterns
    const { chunks: tier3Chunks, tips: tier3Tips } = scanPatterns(dir);

    const allChunks = [...tier1Chunks, ...tier2Chunks, ...tier3Chunks];

    // Infer connections
    const connections = inferConnections(allChunks);

    // Merge tips
    const tips = mergeTips([...tier2Tips, ...tier3Tips]);

    // Collect all unique tags
    const tags = [...new Set(allChunks.flatMap(c => c.tags))].sort();

    return {
        codebase,
        chunks: allChunks,
        connections,
        tags,
        tips,
    };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/discover.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/lib/setup/discover.ts apps/cli/src/__tests__/setup/discover.test.ts
git commit -m "feat(cli): add discovery orchestrator combining all tiers"
```

---

## Task 8: Preview Table Formatter

**Files:**
- Create: `apps/cli/src/lib/setup/preview.ts`
- Create: `apps/cli/src/__tests__/setup/preview.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// apps/cli/src/__tests__/setup/preview.test.ts

import { describe, expect, it } from "vitest";

import { formatPreview } from "../../lib/setup/preview";
import type { DiscoveredChunk, DiscoveredConnection } from "../../lib/setup/types";

function makeChunk(overrides: Partial<DiscoveredChunk> & { title: string; category: DiscoveredChunk["category"] }): DiscoveredChunk {
    return {
        content: "",
        type: "reference",
        tags: [],
        tier: 1,
        source: "test",
        ...overrides,
    };
}

describe("formatPreview", () => {
    it("groups chunks by category with counts", () => {
        const chunks: DiscoveredChunk[] = [
            makeChunk({ title: "README", category: "documents" }),
            makeChunk({ title: "Guide", category: "documents" }),
            makeChunk({ title: "Tech Stack", category: "tech-stack" }),
        ];
        const output = formatPreview(chunks, [], []);
        expect(output).toContain("Documents");
        expect(output).toContain("2");
        expect(output).toContain("Tech stack");
        expect(output).toContain("1");
        expect(output).toContain("3 chunks");
    });

    it("shows connection count", () => {
        const chunks: DiscoveredChunk[] = [
            makeChunk({ title: "A", category: "documents" }),
        ];
        const connections: DiscoveredConnection[] = [
            { sourceTitle: "A", targetTitle: "B", relation: "references" },
            { sourceTitle: "C", targetTitle: "D", relation: "depends_on" },
        ];
        const output = formatPreview(chunks, connections, []);
        expect(output).toContain("2 connections");
    });

    it("shows example titles per category", () => {
        const chunks: DiscoveredChunk[] = [
            makeChunk({ title: "README", category: "documents" }),
            makeChunk({ title: "Contributing Guide", category: "documents" }),
        ];
        const output = formatPreview(chunks, [], []);
        expect(output).toContain("README");
    });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/preview.test.ts`
Expected: FAIL — `formatPreview` does not exist.

- [ ] **Step 3: Implement preview.ts**

```typescript
// apps/cli/src/lib/setup/preview.ts

import pc from "picocolors";

import type { DiscoveredChunk, DiscoveredConnection } from "./types";

const CATEGORY_LABELS: Record<DiscoveredChunk["category"], string> = {
    "documents": "Documents",
    "tech-stack": "Tech stack",
    "structure": "Structure",
    "conventions": "Conventions",
    "config": "Config",
};

const CATEGORY_ORDER: DiscoveredChunk["category"][] = [
    "documents", "tech-stack", "structure", "conventions", "config",
];

/**
 * Format the discovery preview as a human-readable summary table.
 */
export function formatPreview(
    chunks: DiscoveredChunk[],
    connections: DiscoveredConnection[],
    tags: string[],
): string {
    const total = chunks.length;

    // Group by category
    const groups = new Map<DiscoveredChunk["category"], DiscoveredChunk[]>();
    for (const chunk of chunks) {
        const list = groups.get(chunk.category) ?? [];
        list.push(chunk);
        groups.set(chunk.category, list);
    }

    const lines: string[] = [];
    lines.push("");
    lines.push(pc.bold(`  Ready to import ${total} chunks`));
    lines.push("");

    // Table rows
    const rows: [string, number, string][] = [];
    for (const cat of CATEGORY_ORDER) {
        const catChunks = groups.get(cat);
        if (!catChunks || catChunks.length === 0) continue;
        const examples = catChunks
            .slice(0, 3)
            .map(c => c.title.length > 25 ? c.title.slice(0, 23) + ".." : c.title)
            .join(", ");
        rows.push([CATEGORY_LABELS[cat], catChunks.length, examples]);
    }

    // Calculate column widths
    const col1Width = Math.max(...rows.map(r => r[0].length), 10);
    const col2Width = 5;

    for (const [label, count, examples] of rows) {
        const countStr = String(count).padStart(col2Width);
        lines.push(`  ${pc.cyan(label.padEnd(col1Width))} ${countStr}   ${pc.dim(examples)}`);
    }

    lines.push("");

    // Footer
    const footerParts: string[] = [];
    if (connections.length > 0) {
        footerParts.push(`${connections.length} connections`);
    }
    if (tags.length > 0) {
        footerParts.push(`${tags.length} tags`);
    }
    if (footerParts.length > 0) {
        lines.push(`  ${pc.dim("+ " + footerParts.join(", "))}`);
        lines.push("");
    }

    return lines.join("\n");
}

/**
 * Format the discovery result as a JSON-serializable object (for --json mode).
 */
export function formatPreviewJson(
    chunks: DiscoveredChunk[],
    connections: DiscoveredConnection[],
    tags: string[],
): Record<string, unknown> {
    const groups: Record<string, { count: number; titles: string[] }> = {};
    for (const cat of CATEGORY_ORDER) {
        const catChunks = chunks.filter(c => c.category === cat);
        if (catChunks.length > 0) {
            groups[cat] = {
                count: catChunks.length,
                titles: catChunks.map(c => c.title),
            };
        }
    }
    return {
        totalChunks: chunks.length,
        groups,
        connections: connections.length,
        tags,
    };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/setup/preview.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/lib/setup/preview.ts apps/cli/src/__tests__/setup/preview.test.ts
git commit -m "feat(cli): add preview table formatter for setup command"
```

---

## Task 9: Server Import Logic

**Files:**
- Create: `apps/cli/src/lib/setup/import-chunks.ts`

- [ ] **Step 1: Implement import-chunks.ts**

This module handles the server API calls to import discovered chunks and connections. No unit test for this file — it's a thin HTTP client that will be covered by the integration/e2e-style testing of the setup command itself.

```typescript
// apps/cli/src/lib/setup/import-chunks.ts

import { readFileSync } from "node:fs";
import { join, relative } from "node:path";

import type { DiscoveredChunk, DiscoveredConnection } from "./types";

interface ImportResult {
    chunksCreated: number;
    connectionsCreated: number;
    errors: { item: string; error: string }[];
}

/**
 * Import discovered chunks and connections to the fubbik server.
 *
 * Strategy:
 * - Tier 1 docs: POST /api/chunks/import-docs (batch)
 * - Tier 2/3 chunks: POST /api/chunks (parallel, concurrency 5)
 * - Connections: POST /api/connections (after chunks, needs title→ID resolution)
 */
export async function importToServer(
    serverUrl: string,
    codebaseId: string,
    chunks: DiscoveredChunk[],
    connections: DiscoveredConnection[],
    dir: string,
    onProgress?: (message: string) => void,
): Promise<ImportResult> {
    const errors: { item: string; error: string }[] = [];
    let chunksCreated = 0;
    let connectionsCreated = 0;

    // Title → server ID mapping for connection resolution
    const titleToId = new Map<string, string>();

    // --- Tier 1: Batch import via /api/chunks/import-docs ---
    const tier1 = chunks.filter(c => c.tier === 1);
    if (tier1.length > 0) {
        onProgress?.("Importing documents...");
        try {
            // Collect unique source files
            const sourceFiles = new Map<string, string>();
            for (const chunk of tier1) {
                if (chunk.source && !sourceFiles.has(chunk.source)) {
                    const fullPath = join(dir, chunk.source);
                    try {
                        sourceFiles.set(chunk.source, readFileSync(fullPath, "utf-8"));
                    } catch {
                        // File might not exist (e.g., split chunks share a source)
                        // Fall back to chunk content for these
                    }
                }
            }

            const files = [...sourceFiles.entries()].map(([path, content]) => ({ path, content }));

            if (files.length > 0) {
                const res = await fetch(`${serverUrl}/api/chunks/import-docs`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ files, codebaseId }),
                });

                if (res.ok) {
                    const data = (await res.json()) as { created: number; skipped: number; errors: { path: string; error: string }[] };
                    chunksCreated += data.created;
                    for (const err of data.errors) {
                        errors.push({ item: err.path, error: err.error });
                    }
                } else {
                    errors.push({ item: "import-docs", error: `Server returned ${res.status}: ${await res.text()}` });
                }
            }
        } catch (err) {
            errors.push({ item: "import-docs", error: String(err) });
        }
    }

    // --- Tier 2/3: Individual chunk creation ---
    const tier23 = chunks.filter(c => c.tier !== 1);
    if (tier23.length > 0) {
        onProgress?.("Importing metadata and patterns...");
        // Process in batches of 5
        for (let i = 0; i < tier23.length; i += 5) {
            const batch = tier23.slice(i, i + 5);
            const results = await Promise.allSettled(
                batch.map(async chunk => {
                    const body: Record<string, unknown> = {
                        title: chunk.title,
                        content: chunk.content,
                        type: chunk.type,
                        tags: chunk.tags,
                        codebaseIds: [codebaseId],
                        origin: "ai",
                    };

                    const res = await fetch(`${serverUrl}/api/chunks`, {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify(body),
                    });

                    if (!res.ok) {
                        throw new Error(`${res.status}: ${await res.text()}`);
                    }

                    const data = (await res.json()) as { id: string };
                    titleToId.set(chunk.title, data.id);
                    return data;
                }),
            );

            for (let j = 0; j < results.length; j++) {
                const result = results[j]!;
                if (result.status === "fulfilled") {
                    chunksCreated++;
                } else {
                    errors.push({ item: batch[j]!.title, error: String(result.reason) });
                }
            }
        }
    }

    // --- Look up tier-1 chunk IDs by title for connections ---
    // We need to search for them since import-docs doesn't return IDs per title
    if (connections.length > 0) {
        onProgress?.("Resolving chunk references...");
        try {
            const res = await fetch(`${serverUrl}/api/chunks?codebaseId=${codebaseId}&limit=200`);
            if (res.ok) {
                const data = (await res.json()) as { items: { id: string; title: string }[] };
                for (const item of data.items) {
                    if (!titleToId.has(item.title)) {
                        titleToId.set(item.title, item.id);
                    }
                }
            }
        } catch {
            // Non-fatal — connections will just fail to resolve
        }
    }

    // --- Connections ---
    if (connections.length > 0) {
        onProgress?.("Creating connections...");
        for (const conn of connections) {
            const sourceId = titleToId.get(conn.sourceTitle);
            const targetId = titleToId.get(conn.targetTitle);
            if (!sourceId || !targetId) continue; // Skip unresolvable connections

            try {
                const res = await fetch(`${serverUrl}/api/connections`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        sourceId,
                        targetId,
                        relation: conn.relation,
                        origin: "ai",
                    }),
                });

                if (res.ok) {
                    connectionsCreated++;
                } else {
                    // Non-fatal — duplicate connections, etc.
                }
            } catch {
                // Non-fatal
            }
        }
    }

    return { chunksCreated, connectionsCreated, errors };
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/cli/src/lib/setup/import-chunks.ts
git commit -m "feat(cli): add server import logic for setup command"
```

---

## Task 10: Update Index Re-exports

**Files:**
- Modify: `apps/cli/src/lib/setup/index.ts`

- [ ] **Step 1: Update index.ts with all exports**

Now that all modules exist, update the barrel file:

```typescript
// apps/cli/src/lib/setup/index.ts

export type {
    DiscoveredChunk,
    DiscoveredConnection,
    DiscoveryResult,
    Tip,
} from "./types";
export { discover } from "./discover";
export { formatPreview, formatPreviewJson } from "./preview";
export { importToServer } from "./import-chunks";
```

- [ ] **Step 2: Commit**

```bash
git add apps/cli/src/lib/setup/index.ts
git commit -m "chore(cli): update setup index exports"
```

---

## Task 11: The Setup Command

**Files:**
- Create: `apps/cli/src/commands/setup.ts`
- Modify: `apps/cli/src/index.ts`
- Modify: `apps/cli/src/__tests__/commands.test.ts`

- [ ] **Step 1: Implement setup.ts**

```typescript
// apps/cli/src/commands/setup.ts

import { Command } from "commander";

import { formatBold, formatDim, formatSuccess } from "../lib/colors";
import { loadConfig } from "../lib/config";
import { getGitRemoteUrl } from "../lib/detect-codebase";
import { isJson, output, outputError } from "../lib/output";
import { confirm, promptInput } from "../lib/prompt";
import { discover, formatPreview, formatPreviewJson, importToServer } from "../lib/setup";
import { getServerUrl } from "../lib/store";

export const setupCommand = new Command("setup")
    .description("Scan this project and populate your fubbik knowledge base")
    .option("--server <url>", "server URL (overrides config)")
    .option("--dry-run", "show preview without importing")
    .option("--yes", "skip confirmation prompt")
    .option("--force", "re-import even if chunks exist for this codebase")
    .action(async (opts: { server?: string; dryRun?: boolean; yes?: boolean; force?: boolean }, cmd: Command) => {
        const jsonMode = isJson(cmd);

        // --- Phase 0: Preflight ---
        const serverUrl = opts.server ?? loadConfig().serverUrl ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Set it in fubbik.config.json or pass --server <url>.");
            outputError("Is the fubbik server running? Start it with: pnpm dev");
            process.exit(1);
        }

        // Test connectivity
        try {
            const res = await fetch(`${serverUrl}/api/health`);
            if (!res.ok) {
                outputError(`Server at ${serverUrl} returned ${res.status}. Is it running?`);
                process.exit(1);
            }
        } catch {
            outputError(`Cannot connect to server at ${serverUrl}. Is it running?`);
            process.exit(1);
        }

        // Detect or create codebase
        const remoteUrl = getGitRemoteUrl();
        const localPath = process.cwd();
        let codebaseId: string | null = null;
        let codebaseName: string;

        // Try to detect existing codebase
        const detectParams = new URLSearchParams();
        if (remoteUrl) detectParams.set("remoteUrl", remoteUrl);
        else detectParams.set("localPath", localPath);

        try {
            const res = await fetch(`${serverUrl}/api/codebases/detect?${detectParams}`);
            if (res.ok) {
                const data = (await res.json()) as { id?: string; name?: string };
                if (data?.id) {
                    codebaseId = data.id;
                    codebaseName = data.name!;
                }
            }
        } catch {
            // Will create below
        }

        if (!codebaseId) {
            // Determine name for new codebase
            if (!remoteUrl) {
                codebaseName = await promptInput("No git remote detected. Codebase name", localPath.split("/").pop() ?? "my-project");
            } else {
                // Extract name from remote URL
                const match = remoteUrl.match(/\/([^/]+?)(?:\.git)?$/);
                codebaseName = match?.[1] ?? "my-project";
            }

            // Create the codebase
            try {
                const res = await fetch(`${serverUrl}/api/codebases`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        name: codebaseName!,
                        remoteUrl: remoteUrl ?? undefined,
                        localPaths: [localPath],
                    }),
                });
                if (res.ok) {
                    const data = (await res.json()) as { id: string };
                    codebaseId = data.id;
                } else {
                    outputError(`Failed to create codebase: ${res.status} ${await res.text()}`);
                    process.exit(1);
                }
            } catch (err) {
                outputError(`Failed to create codebase: ${err}`);
                process.exit(1);
            }
        }

        if (!jsonMode) {
            console.log(`\n${formatBold("📍")} Detected codebase: ${formatBold(codebaseName!)}${remoteUrl ? ` (${formatDim(remoteUrl)})` : ""}\n`);
        }

        // Check if codebase already has chunks
        if (!opts.force) {
            try {
                const res = await fetch(`${serverUrl}/api/chunks?codebaseId=${codebaseId}&limit=1`);
                if (res.ok) {
                    const data = (await res.json()) as { total: number };
                    if (data.total > 0) {
                        if (!jsonMode) {
                            console.log(`This codebase already has ${data.total} chunks.`);
                        }
                        if (!opts.yes) {
                            const proceed = await confirm("Continue and add more?");
                            if (!proceed) {
                                console.log("Aborted. Use --force to re-import.");
                                process.exit(0);
                            }
                        }
                    }
                }
            } catch {
                // Non-fatal
            }
        }

        // --- Phase 1: Discover ---
        if (!jsonMode) console.log("Scanning project...");

        const result = discover(localPath, {
            name: codebaseName!,
            remoteUrl,
            localPath,
        });

        if (result.chunks.length === 0) {
            if (jsonMode) {
                output(cmd, { chunks: 0, message: "No knowledge sources found" }, "");
            } else {
                console.log("\nNo knowledge sources found in this project.");
            }
            process.exit(0);
        }

        if (!jsonMode) {
            const tierCounts = [1, 2, 3].map(t => result.chunks.filter(c => c.tier === t).length);
            console.log(formatSuccess(`${tierCounts[0]} markdown docs found`));
            console.log(formatSuccess(`${tierCounts[1]} config files analyzed`));
            console.log(formatSuccess(`${tierCounts[2]} code patterns detected`));
        }

        // --- Phase 2: Preview ---
        if (jsonMode) {
            const previewData = formatPreviewJson(result.chunks, result.connections, result.tags);
            if (opts.dryRun) {
                output(cmd, previewData, "");
                process.exit(0);
            }
        } else {
            console.log(formatPreview(result.chunks, result.connections, result.tags));
            if (opts.dryRun) {
                console.log("Dry run complete. No changes made.");
                process.exit(0);
            }
        }

        // --- Phase 3: Confirm ---
        if (!opts.yes && !jsonMode) {
            const proceed = await confirm(`Import ${result.chunks.length} chunks to fubbik?`);
            if (!proceed) {
                console.log("Aborted.");
                process.exit(0);
            }
        }

        // --- Phase 4: Import ---
        if (!jsonMode) console.log("");

        const importResult = await importToServer(
            serverUrl,
            codebaseId!,
            result.chunks,
            result.connections,
            localPath,
            jsonMode ? undefined : (msg) => console.log(`  ${formatDim(msg)}`),
        );

        if (!jsonMode) {
            console.log("");
            console.log(formatSuccess(`${importResult.chunksCreated} chunks created`));
            if (importResult.connectionsCreated > 0) {
                console.log(formatSuccess(`${importResult.connectionsCreated} connections established`));
            }
            if (importResult.errors.length > 0) {
                console.log(`\n  ${importResult.errors.length} errors:`);
                for (const err of importResult.errors.slice(0, 5)) {
                    console.log(`    ${err.item}: ${err.error}`);
                }
                if (importResult.errors.length > 5) {
                    console.log(`    ... and ${importResult.errors.length - 5} more`);
                }
            }
        }

        // --- Phase 5: Tips ---
        if (result.tips.length > 0 && !jsonMode) {
            console.log(`\n${formatBold("You might also want to add:")}`);
            for (const tip of result.tips) {
                console.log(`  ${formatDim("•")} ${tip.title} — ${formatDim(tip.detail)}`);
            }
        }

        // Link to graph
        const webUrl = serverUrl.replace(":3000", ":3001");
        if (!jsonMode) {
            console.log(`\n  View your knowledge graph: ${formatDim(webUrl + "/graph")}\n`);
        }

        // JSON output
        if (jsonMode) {
            output(cmd, {
                chunksCreated: importResult.chunksCreated,
                connectionsCreated: importResult.connectionsCreated,
                errors: importResult.errors,
                tips: result.tips,
            }, "");
        }
    });
```

- [ ] **Step 2: Register the command in index.ts**

Add the import and registration to `apps/cli/src/index.ts`:

Add after the existing imports (around line 30):
```typescript
import { setupCommand } from "./commands/setup";
```

Add after the `initCommand` registration (around line 70):
```typescript
program.addCommand(setupCommand);
```

- [ ] **Step 3: Add help test**

Add to the existing `describe("CLI help output", ...)` block in `apps/cli/src/__tests__/commands.test.ts`:

```typescript
    it("setup --help shows options", () => {
        const { stdout } = runCli("setup --help");
        expect(stdout).toContain("--server");
        expect(stdout).toContain("--dry-run");
        expect(stdout).toContain("--yes");
        expect(stdout).toContain("--force");
    });
```

Also update the root help test to include `setup`:

In the `"root --help lists all commands"` test, add:
```typescript
        expect(stdout).toContain("setup");
```

- [ ] **Step 4: Run all tests**

Run: `cd apps/cli && pnpm vitest run`
Expected: All tests pass, including the new setup help test.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/commands/setup.ts apps/cli/src/index.ts apps/cli/src/__tests__/commands.test.ts
git commit -m "feat(cli): add fubbik setup command with full phase orchestration"
```

---

## Task 12: Type Check and Final Validation

**Files:** None new — this is a validation step.

- [ ] **Step 1: Run type checking**

Run: `pnpm run check-types`
Expected: No type errors. If there are errors, fix them.

- [ ] **Step 2: Run linting**

Run: `pnpm lint`
Expected: No lint errors in the new files.

- [ ] **Step 3: Run all CLI tests**

Run: `cd apps/cli && pnpm vitest run`
Expected: All tests pass.

- [ ] **Step 4: Test dry-run manually (if server running)**

Run: `cd /tmp && mkdir test-project && cd test-project && git init && echo '# Test' > README.md && echo '{"name":"test","dependencies":{"react":"^18.0.0"},"devDependencies":{"vitest":"^1.0.0"}}' > package.json && mkdir -p src/__tests__ && echo 'test("ok", () => {})' > src/__tests__/app.test.ts && fubbik setup --server http://localhost:3000 --dry-run`

Expected: Shows the preview table with at least a README doc chunk, a tech stack chunk (react), and a testing convention chunk. Does not import anything.

- [ ] **Step 5: Clean up and final commit if any fixes were needed**

```bash
git add -A && git commit -m "fix(cli): address type/lint issues in setup command"
```

Only run this if Step 1 or 2 required fixes. Skip if everything was clean.
