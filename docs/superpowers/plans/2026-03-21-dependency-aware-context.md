# Dependency-Aware Context Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When generating context for a file, also include chunks from codebases that the current project depends on (detected via package.json, go.mod, etc.).

**Architecture:** New service function that detects project dependencies, maps them to known codebases (by package name matching against codebase names or remote URLs), and includes their chunks in context results. Extends the existing `/context/for-file` endpoint with a `includeDeps` flag.

**Tech Stack:** Effect, Elysia, Node.js fs (for reading manifest files)

---

## File Structure

### New files:
- `packages/api/src/context-for-file/detect-deps.ts` — Detect project dependencies from manifest files
- `packages/api/src/context-for-file/detect-deps.test.ts` — Tests

### Files to modify:
- `packages/api/src/context-for-file/service.ts` — Add dependency-aware context
- `packages/api/src/context-for-file/routes.ts` — Add `deps` query param (comma-separated dependency names)
- `apps/cli/src/commands/context-for.ts` — Add `--include-deps` flag

---

## Task 1: Dependency Detection

**Files:**
- Create: `packages/api/src/context-for-file/detect-deps.ts`
- Create: `packages/api/src/context-for-file/detect-deps.test.ts`

- [ ] **Step 1: Write tests**

```ts
// packages/api/src/context-for-file/detect-deps.test.ts
import { describe, it, expect } from "vitest";
import { parseDependencies } from "./detect-deps";

describe("parseDependencies", () => {
    it("extracts npm dependencies from package.json content", () => {
        const content = JSON.stringify({
            dependencies: { "@acme/auth": "^1.0.0", "react": "^18.0.0" },
            devDependencies: { "vitest": "^3.0.0" },
        });
        const deps = parseDependencies("package.json", content);
        expect(deps).toContain("@acme/auth");
        expect(deps).toContain("react");
        expect(deps).not.toContain("vitest"); // devDeps excluded by default
    });

    it("extracts Go dependencies from go.mod content", () => {
        const content = `module github.com/acme/myapp\n\nrequire (\n\tgithub.com/acme/shared v1.2.0\n\tgithub.com/gin-gonic/gin v1.9.0\n)`;
        const deps = parseDependencies("go.mod", content);
        expect(deps).toContain("github.com/acme/shared");
        expect(deps).toContain("github.com/gin-gonic/gin");
    });

    it("returns empty array for unknown file types", () => {
        expect(parseDependencies("Makefile", "all: build")).toEqual([]);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && pnpm vitest run src/context-for-file/detect-deps.test.ts`

- [ ] **Step 3: Implement parseDependencies**

```ts
// packages/api/src/context-for-file/detect-deps.ts

export function parseDependencies(filename: string, content: string): string[] {
    if (filename === "package.json") {
        try {
            const pkg = JSON.parse(content);
            return Object.keys(pkg.dependencies ?? {});
        } catch {
            return [];
        }
    }

    if (filename === "go.mod") {
        const deps: string[] = [];
        const requireBlock = content.match(/require\s*\(([\s\S]*?)\)/);
        if (requireBlock) {
            const lines = requireBlock[1]!.split("\n");
            for (const line of lines) {
                const match = line.trim().match(/^(\S+)\s+/);
                if (match) deps.push(match[1]!);
            }
        }
        return deps;
    }

    // Extensible: add Cargo.toml, requirements.txt, etc. later
    return [];
}

// NOTE: Use ESM imports at the top of the file (NOT require()):
// import { dirname, join } from "node:path";
// import { existsSync } from "node:fs";

export function findManifestFile(filePath: string): string | null {
    // Walk up from filePath to find package.json or go.mod

    let dir = dirname(filePath);
    const manifests = ["package.json", "go.mod"];

    for (let i = 0; i < 10; i++) {
        for (const m of manifests) {
            const candidate = join(dir, m);
            if (existsSync(candidate)) {
                return candidate;
            }
        }
        const parent = dirname(dir);
        if (parent === dir) break;
        dir = parent;
    }
    return null;
}
```

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run src/context-for-file/detect-deps.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: add dependency detection from package.json and go.mod"
```

---

## Task 2: Dependency-Aware Context Service

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`
- Modify: `packages/api/src/context-for-file/routes.ts`

- [ ] **Step 1: Extend service to resolve dependencies to codebases**

In `service.ts`, add a function that:
1. Reads the manifest file near the requested file path
2. Parses dependencies
3. Matches dependency names against known codebase names (fuzzy: `@acme/auth` matches codebase named "auth" or "acme-auth")
4. Fetches chunks from matched codebases

```ts
import { parseDependencies, findManifestFile } from "./detect-deps";
// NOTE: Verify the actual export name in packages/db/src/repository/codebase.ts
// It may be `listCodebasesForUser` or similar — check before using.
import { /* listCodebases or actual name */ } from "@fubbik/db/repository";

export function getDepContext(userId: string, filePath: string) {
    return Effect.gen(function* () {
        // This runs server-side, so we need the actual file path on disk
        // The filePath is relative to the codebase root
        // We can't access the filesystem from the server
        // Instead, accept deps as a query param from the client
        // The CLI/client detects deps and sends them
    });
}
```

**Alternative approach (simpler):** The client (CLI) detects dependencies and sends them as a query param. The server matches dep names against codebase names and returns relevant chunks.

Add to the route:
```ts
query: t.Object({
    path: t.String(),
    codebaseId: t.Optional(t.String()),
    deps: t.Optional(t.String()), // comma-separated dependency names
})
```

In the service, when `deps` is provided:
1. Parse comma-separated dep names
2. Query codebases where name matches any dep name (fuzzy)
3. For each matched codebase, fetch top chunks (by relevance or recency)
4. Append to results with `matchReason: "dependency"`

- [ ] **Step 2: Add `deps` query param to route**

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: dependency-aware context with codebase name matching"
```

---

## Task 3: CLI Integration

**Files:**
- Modify: `apps/cli/src/commands/context-for.ts`

- [ ] **Step 1: Add `--include-deps` flag**

When set:
1. Find the nearest `package.json` or `go.mod` relative to the file path
2. Parse dependencies
3. Pass as `&deps=dep1,dep2,dep3` to the API call

```ts
.option("--include-deps", "include chunks from dependency codebases")
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(cli): add --include-deps flag to context-for command"
```
