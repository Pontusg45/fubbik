# Context Fetching & Organization Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bugs, consolidate two parallel context systems into one pipeline, improve retrieval quality with semantic search and scoring, and add organizational features (tag AND mode, scope registry, connection expansion).

**Architecture:** Four sequential phases (Fix → Consolidate → Enhance → Organize), each independently shippable. The `context/` module becomes the single owner of scoring, enrichment, formatting, and output. The older `context-for-file/` and `context-export/` modules become thin wrappers delegating to `context/`.

**Tech Stack:** TypeScript, Elysia, Effect, Drizzle ORM, pgvector, Ollama, Vitest

**Spec:** `docs/superpowers/specs/2026-05-05-context-fetching-improvements-design.md`

---

## Phase 1: Fix — Bugs, Performance & Correctness

### Task 1: Fix MCP Context Tool Registration + sync_claude_md Divergence

Spec items 1a and 1d. Both are in `packages/mcp/src/context-tools.ts`.

**Files:**
- Modify: `packages/mcp/src/context-tools.ts`

- [ ] **Step 1: Fix the function boundary — move orphaned tools inside registerContextTools**

The closing brace `}` at line 166 ends `registerContextTools` prematurely. The `create_context_snapshot` (lines 168-202) and `get_context_snapshot` (lines 204-236) tool registrations are outside the function, followed by a dangling `}` at line 237.

Fix: remove the premature `}` at line 166 and the dangling `}` at line 237, so all five `server.tool()` calls are inside `registerContextTools`.

In `packages/mcp/src/context-tools.ts`, replace:

```typescript
    });
}

    server.tool(
        "create_context_snapshot",
```

with:

```typescript
    });

    server.tool(
        "create_context_snapshot",
```

And remove the extra closing `}` at line 237 (after `get_context_snapshot` ends). The function's closing `}` should be the one right before `export const contextPlugin`.

- [ ] **Step 2: Replace sync_claude_md inline markdown with server endpoint call**

Replace the entire `sync_claude_md` tool handler (lines 7-66) to call `GET /chunks/export/claude-md` instead of building markdown inline:

```typescript
server.tool(
    "sync_claude_md",
    "Generate .claude/CLAUDE.md content from chunks tagged with a specific tag (default: claude-context). Returns the markdown content that can be written to CLAUDE.md.",
    {
        tag: z.string().optional().describe("Tag to filter by (default: claude-context)"),
        codebaseId: z.string().optional().describe("Codebase ID to scope chunks")
    },
    async ({ tag, codebaseId }) => {
        const params = new URLSearchParams();
        if (tag) params.set("tag", tag);
        if (codebaseId) params.set("codebaseId", codebaseId);

        const data = (await apiFetch(`/chunks/export/claude-md?${params}`)) as {
            content: string;
            chunks: number;
        };

        if (data.chunks === 0) {
            const filterTag = tag ?? "claude-context";
            return {
                content: [
                    {
                        type: "text" as const,
                        text: `No chunks tagged "${filterTag}" found. Tag some chunks with "${filterTag}" to generate CLAUDE.md content.`
                    }
                ]
            };
        }

        return {
            content: [
                {
                    type: "text" as const,
                    text: `Generated CLAUDE.md content (${data.chunks} chunks):\n\n${data.content}`
                }
            ]
        };
    }
);
```

- [ ] **Step 3: Verify the file compiles**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types`

Expected: No type errors in `packages/mcp/src/context-tools.ts`.

- [ ] **Step 4: Commit**

```bash
git add packages/mcp/src/context-tools.ts
git commit -m "fix: register orphaned MCP snapshot tools and use server endpoint for sync_claude_md"
```

---

### Task 2: Add Snapshot Auth Check

Spec item 1b. The `getSnapshot` function fetches by UUID without verifying user ownership.

**Files:**
- Modify: `packages/api/src/context/snapshot-service.ts`
- Modify: `packages/api/src/context/snapshot-routes.ts`

- [ ] **Step 1: Add userId parameter to getSnapshot**

In `packages/api/src/context/snapshot-service.ts`, change `getSnapshot`:

```typescript
export function getSnapshot(id: string, userId: string) {
    return Effect.gen(function* () {
        const snapshot = yield* getSnapshotById(id);
        if (!snapshot || snapshot.userId !== userId) {
            return yield* Effect.fail(new NotFoundError({ resource: "ContextSnapshot" }));
        }
        return snapshot;
    });
}
```

- [ ] **Step 2: Pass session userId in the route**

In `packages/api/src/context/snapshot-routes.ts`, update the GET handler to pass the session user ID:

```typescript
.get(
    "/context/snapshot/:id",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => getSnapshot(ctx.params.id, session.user.id)),
            ),
        ),
    {
        params: t.Object({ id: t.String() }),
    },
)
```

Also update the DELETE handler to pass userId for consistency (prevents deleting another user's snapshot):

```typescript
.delete(
    "/context/snapshot/:id",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    getSnapshot(ctx.params.id, session.user.id).pipe(
                        Effect.flatMap(() => deleteSnapshot(ctx.params.id)),
                    ),
                ),
            ),
        ),
    {
        params: t.Object({ id: t.String() }),
    },
)
```

- [ ] **Step 3: Verify types**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context/snapshot-service.ts packages/api/src/context/snapshot-routes.ts
git commit -m "fix: add userId check to snapshot retrieval and deletion"
```

---

### Task 3: Fix N+1 Query in context-for-file Strategy 2

Spec item 1c. Replace per-chunk `getAppliesToForChunk` with batch `getAppliesToForChunks`.

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`
- Test: `packages/api/src/context-for-file/service.test.ts` (new)

- [ ] **Step 1: Write the test**

Create `packages/api/src/context-for-file/service.test.ts`:

```typescript
import { Effect } from "effect";
import { describe, expect, it, vi } from "vitest";

vi.mock("@fubbik/db/repository", () => ({
    lookupChunksByFilePath: vi.fn(),
    getChunkById: vi.fn(),
    listChunks: vi.fn(),
    getAppliesToForChunks: vi.fn(),
    getRequirementsForChunks: vi.fn(),
    listCodebases: vi.fn(),
}));

import {
    getAppliesToForChunks,
    getChunkById,
    listChunks,
    lookupChunksByFilePath,
    getRequirementsForChunks,
    listCodebases,
} from "@fubbik/db/repository";
import { getContextForFile } from "./service";

function makeChunk(id: string, title: string) {
    return {
        id,
        title,
        content: `Content for ${title}`,
        type: "note",
        summary: null,
    };
}

describe("getContextForFile", () => {
    it("uses batch getAppliesToForChunks instead of per-chunk queries", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(
            Effect.succeed({
                chunks: [
                    makeChunk("c1", "Chunk 1"),
                    makeChunk("c2", "Chunk 2"),
                    makeChunk("c3", "Chunk 3"),
                ],
                total: 3,
            })
        );

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(
            Effect.succeed([
                { chunkId: "c1", pattern: "src/**/*.ts", note: null },
                { chunkId: "c3", pattern: "lib/**/*.ts", note: null },
            ])
        );

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/service.ts")
        );

        // Should have called batch function exactly once
        expect(batchMock).toHaveBeenCalledTimes(1);
        expect(batchMock).toHaveBeenCalledWith(["c1", "c2", "c3"]);

        // c1 matches src/**/*.ts, c3 does not match (lib/**/*.ts)
        expect(result.chunks).toHaveLength(1);
        expect(result.chunks[0]!.id).toBe("c1");
        expect(result.chunks[0]!.matchReason).toBe("applies-to");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/service.test.ts`

Expected: FAIL — the current service imports `getAppliesToForChunk` (singular), not `getAppliesToForChunks` (plural).

- [ ] **Step 3: Implement the batch fix**

In `packages/api/src/context-for-file/service.ts`, change the import:

```typescript
import { getAppliesToForChunks, getChunkById, getRequirementsForChunks, listChunks, listCodebases, lookupChunksByFilePath } from "@fubbik/db/repository";
```

Replace Strategy 2 (the `for (const c of chunks)` loop, lines 75-91) with:

```typescript
        // 2. Applies-to glob pattern matches (batch)
        const { chunks } = yield* listChunks({
            userId,
            codebaseId,
            limit: 1000,
            offset: 0
        });

        const uncheckedIds = chunks.filter(c => !results.has(c.id)).map(c => c.id);
        if (uncheckedIds.length > 0) {
            const allPatterns = yield* getAppliesToForChunks(uncheckedIds);

            // Group patterns by chunkId
            const patternsByChunk = new Map<string, Array<{ pattern: string }>>();
            for (const p of allPatterns) {
                const existing = patternsByChunk.get(p.chunkId) ?? [];
                existing.push(p);
                patternsByChunk.set(p.chunkId, existing);
            }

            for (const c of chunks) {
                if (results.has(c.id)) continue;
                const patterns = patternsByChunk.get(c.id);
                if (!patterns || patterns.length === 0) continue;

                const matches = patterns.some(p => globMatch(p.pattern, filePath));
                if (matches) {
                    results.set(c.id, {
                        id: c.id,
                        title: c.title,
                        type: c.type,
                        content: c.content,
                        summary: c.summary,
                        matchReason: "applies-to"
                    });
                }
            }
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/service.test.ts`

Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

Expected: All tests pass (existing `service.test.ts` in `context-export` should still pass since it mocks `getContextForFile`).

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/context-for-file/service.ts packages/api/src/context-for-file/service.test.ts
git commit -m "perf: batch applies-to queries in context-for-file (fix N+1)"
```

---

### Task 4: Batch CLI context dir Command

Spec item 1e. Replace per-file HTTP calls with a single `/context/for-files` call.

**Files:**
- Modify: `apps/cli/src/commands/context-dir.ts`

- [ ] **Step 1: Replace per-file loop with batch call**

In `apps/cli/src/commands/context-dir.ts`, replace the per-file fetch loop (lines 148-170) with a single batch call:

```typescript
        const relativePaths = files.map(f => relative(process.cwd(), f));
        const allChunks: ContextChunk[] = [];

        // Batch fetch — single HTTP call for all files
        const params = new URLSearchParams();
        params.set("paths", relativePaths.join(","));
        params.set("format", "structured-json");
        if (opts.codebase) params.set("codebaseId", opts.codebase);

        try {
            const res = await fetch(`${serverUrl}/api/context/for-files?${params.toString()}`);
            if (res.ok) {
                const data = (await res.json()) as {
                    sections?: Array<{
                        title: string;
                        chunks: Array<{
                            id: string;
                            title: string;
                            type: string;
                            content: string;
                            summary?: string | null;
                            rationale?: string | null;
                        }>;
                    }>;
                };
                if (data.sections) {
                    const seenIds = new Set<string>();
                    for (const section of data.sections) {
                        for (const chunk of section.chunks) {
                            if (!seenIds.has(chunk.id)) {
                                seenIds.add(chunk.id);
                                allChunks.push({
                                    id: chunk.id,
                                    title: chunk.title,
                                    type: chunk.type,
                                    content: chunk.content,
                                    summary: chunk.summary ?? null,
                                    matchReason: "file-ref",
                                });
                            }
                        }
                    }
                }
            }
        } catch {
            outputError("Failed to fetch context for directory");
            process.exit(1);
        }
```

Also remove the `seenChunkIds` variable that was declared before the old loop — it's no longer needed since dedup is handled inline above.

- [ ] **Step 2: Verify types**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types`

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/context-dir.ts
git commit -m "perf: batch context-dir CLI to single HTTP call"
```

---

## Phase 2: Consolidate — Unify the Two Context Systems

### Task 5: Single Scoring Function

Spec item 2a. Delete the duplicate `scoreChunk` from `context-export/service.ts`.

**Files:**
- Modify: `packages/api/src/context-export/service.ts`

- [ ] **Step 1: Remove duplicate scoreChunk and estimateTokens, import from context/utils**

In `packages/api/src/context-export/service.ts`:

1. Remove the `import { computeHealthScore } from "../chunks/health-score"` (no longer needed here)
2. Remove the local `scoreChunk` function (lines 21-42)
3. Remove the local `ScoredChunk` interface (lines 44-52)
4. Remove the local `estimateTokens` function (lines 54-56)
5. Remove the local `formatChunkText` function (lines 182-193)
6. Add imports from `context/utils`:

```typescript
import { scoreChunk, estimateTokens, formatChunkText, type ScoredChunk } from "../context/utils";
```

Also remove the `chunk as chunkTable` import and `type ChunkRow` since `scoreChunk` from utils already handles the typing.

The `exportContext` function body stays unchanged — it just now uses the imported functions.

- [ ] **Step 2: Run existing tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-export/service.test.ts`

Expected: All 10 tests pass (the scoring behavior is identical).

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/context-export/service.ts
git commit -m "refactor: deduplicate scoreChunk — single source in context/utils"
```

---

### Task 6: Route context-for-file Through the New Resolver

Spec item 2b. The old route returns raw chunks; rewire to use `resolveForFiles` + formatter.

**Files:**
- Modify: `packages/api/src/context-for-file/routes.ts`

- [ ] **Step 1: Rewrite the route to use resolvers**

Replace the contents of `packages/api/src/context-for-file/routes.ts`:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { formatStructured, formatStructuredMarkdown } from "../context/formatter";
import { enrichChunks, resolveForFiles } from "../context/resolvers";
import { budgetChunks } from "../context/utils";
import { getContextForFile } from "./service";

const DEFAULT_MAX_TOKENS = 4000;

export const contextForFileRoutes = new Elysia().get(
    "/context/for-file",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => {
                    const format = ctx.query.format ?? "structured-md";

                    // Legacy JSON format for backwards compatibility
                    if (format === "json-legacy") {
                        return getContextForFile(
                            session.user.id,
                            ctx.query.path,
                            ctx.query.codebaseId,
                            ctx.query.deps ? ctx.query.deps.split(",").filter(Boolean) : undefined
                        );
                    }

                    const maxTokens = ctx.query.maxTokens
                        ? Number(ctx.query.maxTokens)
                        : DEFAULT_MAX_TOKENS;

                    return resolveForFiles(
                        [ctx.query.path],
                        session.user.id,
                        ctx.query.codebaseId,
                    ).pipe(
                        Effect.flatMap(ids => enrichChunks(ids, session.user.id)),
                        Effect.map(chunks => {
                            const budgeted = budgetChunks(chunks, maxTokens);
                            const structured = formatStructured(budgeted);
                            if (format === "structured-json") {
                                return { format: "structured-json" as const, ...structured };
                            }
                            return {
                                format: "structured-md" as const,
                                content: formatStructuredMarkdown(structured),
                                totalChunks: structured.totalChunks,
                            };
                        }),
                    );
                }),
            ),
        ),
    {
        query: t.Object({
            path: t.String(),
            codebaseId: t.Optional(t.String()),
            deps: t.Optional(t.String()),
            format: t.Optional(
                t.Union([
                    t.Literal("structured-md"),
                    t.Literal("structured-json"),
                    t.Literal("json-legacy"),
                ]),
            ),
            maxTokens: t.Optional(t.String()),
        }),
    },
);
```

- [ ] **Step 2: Verify types**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types`

- [ ] **Step 3: Run all tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

Expected: All tests pass. The `context-export/service.test.ts` mocks `getContextForFile` so it's unaffected.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context-for-file/routes.ts
git commit -m "refactor: route context-for-file through unified resolver + formatter"
```

---

### Task 7: Route context-export Through the New Pipeline

Spec item 2c. Refactor `exportContext` to use shared enrichment.

**Files:**
- Modify: `packages/api/src/context-export/service.ts`

- [ ] **Step 1: Refactor exportContext to use enrichChunks pipeline**

Replace the contents of `packages/api/src/context-export/service.ts`:

```typescript
import {
    getTagsForChunks,
    listChunks as listChunksRepo,
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { getContextForFile } from "../context-for-file/service";
import { enrichChunks, resolveForFiles } from "../context/resolvers";
import { formatStructured, formatStructuredMarkdown } from "../context/formatter";
import { budgetChunks, scoreChunk, estimateTokens, formatChunkText, type ScoredChunk } from "../context/utils";

interface ContextExportQuery {
    codebaseId?: string;
    maxTokens?: number;
    format?: "markdown" | "json";
    forPath?: string;
}

export function exportContext(userId: string, query: ContextExportQuery) {
    const maxTokens = query.maxTokens ?? 4000;
    const format = query.format ?? "markdown";

    // Fetch approved chunks first, then others
    const fetchApproved = listChunksRepo({
        userId,
        reviewStatus: "approved",
        codebaseId: query.codebaseId,
        limit: 500,
        offset: 0,
    });

    const fetchOthers = listChunksRepo({
        userId,
        codebaseId: query.codebaseId,
        limit: 500,
        offset: 0,
    });

    return Effect.all({ approved: fetchApproved, all: fetchOthers }).pipe(
        Effect.flatMap(({ approved, all }) => {
            // Deduplicate: approved first, then non-approved from the "all" set
            const approvedIds = new Set(approved.chunks.map(c => c.id));
            const otherChunks = all.chunks.filter(c => !approvedIds.has(c.id));
            const combined = [...approved.chunks, ...otherChunks];
            const chunkIds = combined.map(c => c.id);

            // Use shared enrichment pipeline
            return enrichChunks(chunkIds, userId).pipe(
                Effect.flatMap(enriched =>
                    Effect.gen(function* () {
                        // File-path relevance boost
                        if (query.forPath) {
                            const fileIds = yield* resolveForFiles(
                                [query.forPath],
                                userId,
                                query.codebaseId,
                            );
                            const fileIdSet = new Set(fileIds);
                            for (const item of enriched) {
                                if (fileIdSet.has(item.id)) {
                                    item.score += 15;
                                }
                            }
                        }

                        const budgeted = budgetChunks(enriched, maxTokens);

                        if (format === "json") {
                            return {
                                format: "json" as const,
                                tokens: budgeted.reduce(
                                    (sum, c) => sum + estimateTokens(formatChunkText(c)),
                                    estimateTokens("# Project Context\n\n"),
                                ),
                                chunks: budgeted.map(c => ({
                                    title: c.title,
                                    content: c.content,
                                    type: c.type,
                                    tags: c.tags,
                                })),
                                content: undefined as string | undefined,
                            };
                        }

                        // Use structured formatter for markdown
                        const structured = formatStructured(budgeted);
                        const content = formatStructuredMarkdown(structured);

                        return {
                            format: "markdown" as const,
                            tokens: estimateTokens(content),
                            chunks: undefined as
                                | { title: string; content: string; type: string; tags: string[] }[]
                                | undefined,
                            content,
                        };
                    }),
                ),
            );
        }),
    );
}
```

- [ ] **Step 2: Run existing tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-export/service.test.ts`

Expected: Tests may need mock adjustments since `exportContext` now calls `enrichChunks` instead of directly fetching connections. Update mocks if needed — the test file already mocks `@fubbik/db/repository` and `../context-for-file/service`, but now also needs to mock `../context/resolvers` and `../context/formatter`.

If tests fail, update the mock setup in the test file to also mock:
```typescript
vi.mock("../context/resolvers", () => ({
    enrichChunks: vi.fn(),
    resolveForFiles: vi.fn(),
}));
vi.mock("../context/formatter", () => ({
    formatStructured: vi.fn(),
    formatStructuredMarkdown: vi.fn(),
}));
```

And wire up the mock returns appropriately.

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/context-export/service.ts packages/api/src/context-export/service.test.ts
git commit -m "refactor: route context-export through shared enrichment pipeline"
```

---

### Task 8: Unify CLAUDE.md Generation + Deprecation Checkpoint

Spec items 2d and 2e. Move `generateClaudeMd` to `context/` module.

**Files:**
- Create: `packages/api/src/context/claude-md.ts`
- Modify: `packages/api/src/context-export/routes.ts` (update import path)
- Delete content from: `packages/api/src/context-export/claude-md.ts` (re-export from new location)

- [ ] **Step 1: Move generateClaudeMd to context/ module**

Create `packages/api/src/context/claude-md.ts` with the exact contents of `packages/api/src/context-export/claude-md.ts` (no changes to the function body yet — Phase 3 will add token budgeting):

```typescript
import { listChunksByTag, listRequirements, listPlans, getChunksForRequirement } from "@fubbik/db/repository";
import { listTasks } from "@fubbik/db/repository/plan";
import { Effect } from "effect";

interface GenerateClaudeMdParams {
    userId: string;
    codebaseId?: string;
    tag?: string;
}

interface ChunkRow {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    summary: string | null;
}

const TYPE_SECTIONS: Record<string, string> = {
    note: "Conventions",
    document: "Architecture",
    reference: "References"
};

function sectionLabel(type: string): string {
    return TYPE_SECTIONS[type] ?? "Other";
}

function formatChunkEntry(c: ChunkRow): string {
    const parts = [`### ${c.title}`];
    if (c.content) parts.push(c.content);
    if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
    return parts.join("\n\n");
}

export function generateClaudeMd(params: GenerateClaudeMdParams) {
    const tagName = params.tag ?? "claude-context";

    return Effect.gen(function* () {
        const chunks = yield* listChunksByTag({
            userId: params.userId,
            tagName,
            codebaseId: params.codebaseId
        });

        const parts: string[] = ["# Project Context\n"];

        if (chunks.length === 0) {
            parts.push(`No chunks found with tag "${tagName}".\n`);
        } else {
            const sections = new Map<string, ChunkRow[]>();
            for (const c of chunks) {
                const label = sectionLabel(c.type);
                const group = sections.get(label) ?? [];
                group.push(c);
                sections.set(label, group);
            }

            const sectionOrder = ["Conventions", "Architecture", "References", "Other"];
            for (const sectionName of sectionOrder) {
                const group = sections.get(sectionName);
                if (!group || group.length === 0) continue;
                parts.push(`## ${sectionName}\n`);
                for (const c of group) {
                    parts.push(formatChunkEntry(c));
                }
            }
        }

        const { requirements } = yield* listRequirements({
            userId: params.userId,
            codebaseId: params.codebaseId,
            limit: 50,
            offset: 0
        });

        if (requirements.length > 0) {
            parts.push("## Requirements\n");

            const statusOrder: Record<string, number> = { failing: 0, untested: 1, passing: 2 };
            const sorted = [...requirements].sort(
                (a, b) => (statusOrder[a.status ?? ""] ?? 3) - (statusOrder[b.status ?? ""] ?? 3)
            );

            for (const req of sorted) {
                const marker =
                    req.status === "failing" || req.status === "untested"
                        ? " <!-- ACTION NEEDED -->"
                        : "";
                const priority = req.priority ? ` [${req.priority}]` : "";
                parts.push(`### ${req.title}${priority} — ${req.status}${marker}`);

                if (req.steps && Array.isArray(req.steps)) {
                    const stepsText = (req.steps as Array<{ keyword: string; text: string }>)
                        .map(s => `- **${s.keyword}** ${s.text}`)
                        .join("\n");
                    parts.push(stepsText);
                }

                const linkedChunks = yield* getChunksForRequirement(req.id);
                if (linkedChunks.length > 0) {
                    const chunkList = linkedChunks.map(c => c.title).join(", ");
                    parts.push(`**Linked chunks:** ${chunkList}`);
                }
            }
        }

        const plans = yield* listPlans({
            userId: params.userId,
            codebaseId: params.codebaseId,
            status: "in_progress",
        });

        if (plans.length > 0) {
            parts.push("## Active Plans\n");

            for (const plan of plans) {
                const tasks = yield* listTasks(plan.id);
                const done = tasks.filter(t => t.status === "done").length;
                const total = tasks.length;
                const pct = total > 0 ? Math.round((done / total) * 100) : 0;

                parts.push(`### ${plan.title} (${done}/${total} tasks — ${pct}%)`);

                const pending = tasks.filter(
                    t => t.status === "pending" || t.status === "in_progress"
                );
                if (pending.length > 0) {
                    const pendingText = pending.map(t => `- [ ] ${t.title}`).join("\n");
                    parts.push(pendingText);
                }
            }
        }

        return { content: parts.join("\n\n"), chunks: chunks.length };
    });
}
```

- [ ] **Step 2: Update the old file to re-export**

Replace `packages/api/src/context-export/claude-md.ts` with:

```typescript
// Re-export from canonical location
export { generateClaudeMd } from "../context/claude-md";
```

- [ ] **Step 3: Run existing CLAUDE.md tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-export/service.test.ts`

Expected: All `generateClaudeMd` tests pass (the re-export is transparent).

- [ ] **Step 4: Deprecation checkpoint — verify all routes go through context/**

Check that:
- `context-for-file/routes.ts` → calls `resolveForFiles` + formatter (done in Task 6)
- `context-export/routes.ts` → calls `exportContext` which uses `enrichChunks` (done in Task 7)
- `context-export/routes.ts` → calls `generateClaudeMd` which re-exports from `context/` (done above)
- `context/routes.ts` → already uses resolvers + formatter (unchanged)

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types && pnpm test`

Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/context/claude-md.ts packages/api/src/context-export/claude-md.ts
git commit -m "refactor: move generateClaudeMd to context/ module, completing consolidation"
```

---

## Phase 3: Enhance — Retrieval Quality & Missing Capabilities

### Task 9: Path Normalization for Glob Matching

Spec item 3f. Add `normalizePath` to prevent `./src/x.ts` vs `src/x.ts` mismatches.

**Files:**
- Modify: `packages/api/src/context-for-file/glob-match.ts`
- Test: `packages/api/src/context-for-file/glob-match.test.ts` (new)

- [ ] **Step 1: Write the test**

Create `packages/api/src/context-for-file/glob-match.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { globMatch, normalizePath } from "./glob-match";

describe("normalizePath", () => {
    it("strips leading ./", () => {
        expect(normalizePath("./src/auth/service.ts")).toBe("src/auth/service.ts");
    });

    it("strips leading /", () => {
        expect(normalizePath("/src/auth/service.ts")).toBe("src/auth/service.ts");
    });

    it("collapses consecutive slashes", () => {
        expect(normalizePath("src//auth///service.ts")).toBe("src/auth/service.ts");
    });

    it("strips trailing /", () => {
        expect(normalizePath("src/auth/")).toBe("src/auth");
    });

    it("handles combined edge cases", () => {
        expect(normalizePath("./src//auth/./service.ts")).toBe("src/auth/./service.ts");
    });

    it("returns empty string unchanged", () => {
        expect(normalizePath("")).toBe("");
    });

    it("handles already-clean paths", () => {
        expect(normalizePath("src/auth/service.ts")).toBe("src/auth/service.ts");
    });
});

describe("globMatch with normalization", () => {
    it("matches ./src/x.ts against src/**/*.ts pattern", () => {
        expect(globMatch("src/**/*.ts", "./src/auth/service.ts")).toBe(true);
    });

    it("matches /src/x.ts against src/**/*.ts pattern", () => {
        expect(globMatch("src/**/*.ts", "/src/auth/service.ts")).toBe(true);
    });

    it("normalizes the pattern too", () => {
        expect(globMatch("./src/**/*.ts", "src/auth/service.ts")).toBe(true);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/glob-match.test.ts`

Expected: FAIL — `normalizePath` is not exported and `globMatch` doesn't normalize.

- [ ] **Step 3: Implement normalizePath and integrate into globMatch**

Update `packages/api/src/context-for-file/glob-match.ts`:

```typescript
export function normalizePath(path: string): string {
    return path
        .replace(/^\.\//, "")   // strip leading ./
        .replace(/^\//, "")     // strip leading /
        .replace(/\/+/g, "/")   // collapse consecutive /
        .replace(/\/$/, "");    // strip trailing /
}

export function globMatch(pattern: string, path: string): boolean {
    const normalizedPattern = normalizePath(pattern);
    const normalizedPath = normalizePath(path);

    const regexStr = normalizedPattern
        .replace(/\./g, "\\.")
        .replace(/\*\*/g, "{{GLOBSTAR}}")
        .replace(/\*/g, "[^/]*")
        .replace(/\?/g, "[^/]")
        .replace(/\{\{GLOBSTAR\}\}/g, ".*");
    return new RegExp(`^${regexStr}$`).test(normalizedPath);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/glob-match.test.ts`

Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/context-for-file/glob-match.ts packages/api/src/context-for-file/glob-match.test.ts
git commit -m "feat: add path normalization to glob matching"
```

---

### Task 10: Score-Based Ranking in context-for-file

Spec item 3b. Add scoring + strategy bonuses so results are ranked by relevance.

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`
- Modify: `packages/api/src/context-for-file/service.test.ts`

- [ ] **Step 1: Add test for scoring**

Add to `packages/api/src/context-for-file/service.test.ts`:

```typescript
// Add to the vi.mock block at the top:
// getConnectionsForChunks: vi.fn(),

import { getConnectionsForChunks } from "@fubbik/db/repository";

describe("getContextForFile scoring", () => {
    it("returns chunks sorted by score descending", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(
            Effect.succeed([
                { chunkId: "c1", chunkTitle: "Thin Note", chunkType: "note", refId: "r1", path: "src/x.ts", anchor: null, relation: "documents" },
            ])
        );

        const getByIdMock = getChunkById as ReturnType<typeof vi.fn>;
        getByIdMock.mockReturnValue(Effect.succeed({
            id: "c1", title: "Thin Note", type: "note", content: "short",
            summary: null, rationale: null, alternatives: null, consequences: null,
            embedding: null, reviewStatus: null, updatedAt: new Date(), createdAt: new Date(),
            userId: "user-1", scope: null, aliases: null, notAbout: null,
            embeddingUpdatedAt: null, sourceUrl: null, sourceType: null,
        }));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(Effect.succeed({
            chunks: [
                {
                    id: "c2", title: "Rich Doc", type: "document", content: "detailed content about authentication",
                    summary: "auth summary", rationale: "because security", alternatives: null, consequences: null,
                    embedding: null, reviewStatus: "approved", updatedAt: new Date(), createdAt: new Date(),
                    userId: "user-1", scope: null, aliases: null, notAbout: null,
                    embeddingUpdatedAt: null, sourceUrl: null, sourceType: null,
                },
            ],
            total: 1,
        }));

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(Effect.succeed([
            { chunkId: "c2", pattern: "src/**/*.ts", note: null },
        ]));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/x.ts")
        );

        // c1 has file-ref bonus (+20) but low health (thin note)
        // c2 has applies-to bonus (+10) but higher health (document, approved, has rationale)
        // Both should be present; order depends on total score
        expect(result.chunks.length).toBe(2);
        // Results should have a score property
        expect(result.chunks[0]!.score).toBeDefined();
        expect(result.chunks[0]!.score).toBeGreaterThanOrEqual(result.chunks[1]!.score);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/service.test.ts`

Expected: FAIL — `score` property doesn't exist on ContextChunk.

- [ ] **Step 3: Implement scoring in the service**

In `packages/api/src/context-for-file/service.ts`:

1. Add imports:
```typescript
import { getAppliesToForChunks, getChunkById, getConnectionsForChunks, getRequirementsForChunks, listChunks, listCodebases, lookupChunksByFilePath } from "@fubbik/db/repository";
import { scoreChunk } from "../context/utils";
```

2. Add `score` to the `ContextChunk` interface:
```typescript
export interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to" | "dependency" | "semantic" | "connected";
    score: number;
}
```

3. Store the full chunk row alongside results so we can score them. Modify the strategy code to also store the raw chunk row for scoring. After all strategies run, add a scoring + sorting block before the requirements step:

```typescript
        // Score and sort results
        const matchedChunks = Array.from(results.values());

        // Fetch connection counts for scoring
        const chunkIdsForScoring = matchedChunks.map(c => c.id);
        const connections = chunkIdsForScoring.length > 0
            ? yield* getConnectionsForChunks(chunkIdsForScoring).pipe(
                  Effect.catchAll(() => Effect.succeed([] as Array<{ sourceId: string; targetId: string }>)),
              )
            : [];

        const connCountMap = new Map<string, number>();
        for (const conn of connections) {
            connCountMap.set(conn.sourceId, (connCountMap.get(conn.sourceId) ?? 0) + 1);
            connCountMap.set(conn.targetId, (connCountMap.get(conn.targetId) ?? 0) + 1);
        }

        // Strategy bonuses
        const STRATEGY_BONUS: Record<string, number> = {
            "file-ref": 20,
            "applies-to": 10,
            "dependency": 3,
            "semantic": 5,
            "connected": 2,
        };

        for (const chunk of matchedChunks) {
            const rawRow = chunkRows.get(chunk.id);
            const connectionCount = connCountMap.get(chunk.id) ?? 0;
            const baseScore = rawRow ? scoreChunk(rawRow, connectionCount) : 0;
            chunk.score = baseScore + (STRATEGY_BONUS[chunk.matchReason] ?? 0);
        }

        // Sort by score descending
        matchedChunks.sort((a, b) => b.score - a.score);
```

You'll need to add a `chunkRows` Map at the top of `getContextForFile`, alongside `results`:

```typescript
const chunkRows = new Map<string, ChunkRow>();
```

Where `ChunkRow` is `typeof chunkTable.$inferSelect` (import `chunk as chunkTable` from `@fubbik/db/schema/chunk`). In each strategy, after adding to `results`, also add the raw chunk row to `chunkRows`:
- Strategy 1 (file-ref): `chunkRows.set(full.id, full)` after `getChunkById`
- Strategy 2 (applies-to): `chunkRows.set(c.id, c)` after the `listChunks` iteration
- Strategy 3 (dependency): `chunkRows.set(c.id, c)` after the dependency `listChunks` iteration
- Strategy 4 (semantic): semantic results don't return full rows, so no entry (scoreChunk will use 0 for missing rows)

- [ ] **Step 4: Run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/service.test.ts`

Expected: PASS

- [ ] **Step 5: Run full suite**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/context-for-file/service.ts packages/api/src/context-for-file/service.test.ts
git commit -m "feat: add score-based ranking to context-for-file results"
```

---

### Task 11: Semantic Strategy in context-for-file

Spec item 3a. Add Strategy 4 using Ollama embeddings for semantic matching.

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`
- Modify: `packages/api/src/context-for-file/service.test.ts`

- [ ] **Step 1: Add a path-to-search-text utility**

In `packages/api/src/context-for-file/service.ts`, add:

```typescript
const IGNORED_SEGMENTS = new Set(["src", "lib", "dist", "build", "index", "node_modules", "packages", "apps"]);
const IGNORED_EXTENSIONS = new Set(["ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "md"]);

function pathToSearchText(filePath: string): string {
    return filePath
        .split(/[/.]/)
        .filter(seg => seg.length > 0 && !IGNORED_SEGMENTS.has(seg) && !IGNORED_EXTENSIONS.has(seg))
        .join(" ");
}
```

- [ ] **Step 2: Add Strategy 4 after Strategy 3 in getContextForFile**

```typescript
        // 4. Semantic similarity (requires Ollama; skip silently if unavailable)
        const searchText = pathToSearchText(filePath);
        if (searchText.length > 0) {
            const semanticChunks = yield* generateQueryEmbedding(searchText).pipe(
                Effect.flatMap(embedding =>
                    semanticSearchRepo({ embedding, userId, limit: 10 }),
                ),
                Effect.catchAll(() => Effect.succeed([] as Array<{ id: string; title: string; type: string; content: string; summary: string | null; similarity: number }>)),
            );

            for (const sc of semanticChunks) {
                if (results.has(sc.id)) continue;
                results.set(sc.id, {
                    id: sc.id,
                    title: sc.title,
                    type: sc.type,
                    content: sc.content,
                    summary: sc.summary,
                    matchReason: "semantic",
                    score: 0, // will be set in scoring step
                });
            }
        }
```

Add the import:
```typescript
import { generateQueryEmbedding } from "../ollama/client";
import { semanticSearch as semanticSearchRepo } from "@fubbik/db/repository";
```

Update the `STRATEGY_BONUS` for semantic to use similarity score: for semantic matches that come from pgvector, the `similarity` field is available. Since we lose that by the time we score, use a flat bonus of 5 (spec says `similarity × 10` but we can store similarity if needed — for v1, flat +5 is fine).

- [ ] **Step 3: Add test for semantic strategy**

Add to `packages/api/src/context-for-file/service.test.ts`:

```typescript
// Add to vi.mock: semanticSearch: vi.fn()
// Add import: import { semanticSearch } from "@fubbik/db/repository";

vi.mock("../ollama/client", () => ({
    generateQueryEmbedding: vi.fn(),
}));

import { generateQueryEmbedding } from "../ollama/client";

describe("getContextForFile semantic strategy", () => {
    it("adds semantic matches when Ollama is available", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(Effect.succeed({ chunks: [], total: 0 }));

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(Effect.succeed([]));

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.succeed([0.1, 0.2, 0.3]));

        const semanticMock = semanticSearch as ReturnType<typeof vi.fn>;
        semanticMock.mockReturnValue(Effect.succeed([
            { id: "s1", title: "Auth Middleware", type: "document", content: "auth content", summary: null, similarity: 0.85 },
        ]));

        const connMock = getConnectionsForChunks as ReturnType<typeof vi.fn>;
        connMock.mockReturnValue(Effect.succeed([]));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/middleware.ts")
        );

        expect(result.chunks).toHaveLength(1);
        expect(result.chunks[0]!.id).toBe("s1");
        expect(result.chunks[0]!.matchReason).toBe("semantic");
    });

    it("skips semantic search silently when Ollama is down", async () => {
        const lookupMock = lookupChunksByFilePath as ReturnType<typeof vi.fn>;
        lookupMock.mockReturnValue(Effect.succeed([]));

        const listMock = listChunks as ReturnType<typeof vi.fn>;
        listMock.mockReturnValue(Effect.succeed({ chunks: [], total: 0 }));

        const batchMock = getAppliesToForChunks as ReturnType<typeof vi.fn>;
        batchMock.mockReturnValue(Effect.succeed([]));

        const embeddingMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embeddingMock.mockReturnValue(Effect.fail(new Error("Ollama unavailable")));

        const reqMock = getRequirementsForChunks as ReturnType<typeof vi.fn>;
        reqMock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(
            getContextForFile("user-1", "src/auth/middleware.ts")
        );

        expect(result.chunks).toHaveLength(0);
    });
});
```

- [ ] **Step 4: Run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm vitest run packages/api/src/context-for-file/service.test.ts`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/context-for-file/service.ts packages/api/src/context-for-file/service.test.ts
git commit -m "feat: add semantic search strategy to context-for-file"
```

---

### Task 12: Feature Overlay Support in Context Retrieval

Spec item 3c. Apply active feature deltas in the enrichment pipeline.

**Files:**
- Modify: `packages/api/src/context/resolvers.ts`

- [ ] **Step 1: Add resolveFeatureOverlays function**

In `packages/api/src/context/resolvers.ts`, add import:

```typescript
import { getActiveFeatureIds, batchFetchDeltas } from "@fubbik/db/repository";
```

Note: `batchFetchDeltas` is exported from `packages/db/src/repository/chunk-feature-delta.ts`. Check that it's re-exported from the repository index. If not, add it to `packages/db/src/repository/index.ts`.

Add the function after `enrichChunks`:

```typescript
export function resolveFeatureOverlays(
    chunks: ChunkWithMetadata[],
    userId: string,
): Effect.Effect<ChunkWithMetadata[], never> {
    if (chunks.length === 0) return Effect.succeed(chunks);

    return Effect.gen(function* () {
        const activeRows = yield* getActiveFeatureIds(userId).pipe(
            Effect.catchAll(() => Effect.succeed([])),
        );
        const activeFeatureIds = activeRows.map((r: { featureId: string }) => r.featureId);

        if (activeFeatureIds.length === 0) return chunks;

        const chunkIds = chunks.map(c => c.id);
        const deltas = yield* batchFetchDeltas(chunkIds, activeFeatureIds).pipe(
            Effect.catchAll(() => Effect.succeed([])),
        );

        if (deltas.length === 0) return chunks;

        // Group deltas by chunkId, already sorted by priority from DB query
        const deltasByChunk = new Map<string, Array<{ delta: Record<string, unknown> }>>();
        for (const d of deltas) {
            const existing = deltasByChunk.get(d.chunkId) ?? [];
            existing.push(d);
            deltasByChunk.set(d.chunkId, existing);
        }

        // Apply deltas to chunks
        return chunks.map(chunk => {
            const chunkDeltas = deltasByChunk.get(chunk.id);
            if (!chunkDeltas || chunkDeltas.length === 0) return chunk;

            const overlay: Record<string, unknown> = {};
            for (const d of chunkDeltas) {
                Object.assign(overlay, d.delta);
            }

            return {
                ...chunk,
                ...(overlay.title != null && { title: overlay.title as string }),
                ...(overlay.content != null && { content: overlay.content as string }),
                ...(overlay.type != null && { type: overlay.type as string }),
                ...(overlay.rationale != null && { rationale: overlay.rationale as string | null }),
            };
        });
    });
}
```

- [ ] **Step 2: Wire into enrichChunks**

Add a call to `resolveFeatureOverlays` at the end of `enrichChunks`, right before the final return. Change the function to accept `userId` as required (it's already optional but always passed):

At the end of `enrichChunks`, after the `.pipe(Effect.map(...))` that filters nulls, chain:

```typescript
    ).pipe(
        Effect.map(results => results.filter((r): r is ChunkWithMetadata => r !== null)),
        Effect.flatMap(chunks => userId ? resolveFeatureOverlays(chunks, userId) : Effect.succeed(chunks)),
    );
```

- [ ] **Step 3: Verify types and run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types && pnpm test`

Expected: All pass. Check that `batchFetchDeltas` is properly exported from the repository index.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context/resolvers.ts
git commit -m "feat: apply active feature overlays in context enrichment pipeline"
```

---

### Task 13: Token Budget Guard for CLAUDE.md

Spec item 3d. Add `maxTokens` parameter to `generateClaudeMd`.

**Files:**
- Modify: `packages/api/src/context/claude-md.ts`
- Modify: `packages/api/src/context-export/routes.ts`

- [ ] **Step 1: Add maxTokens parameter to generateClaudeMd**

In `packages/api/src/context/claude-md.ts`:

1. Add to the `GenerateClaudeMdParams` interface:
```typescript
    maxTokens?: number;
```

2. Import `estimateTokens` from utils:
```typescript
import { estimateTokens } from "./utils";
```

3. After building all `parts`, add budget enforcement before the return:

```typescript
        const maxTokens = params.maxTokens ?? 32000;
        let content = parts.join("\n\n");
        const totalTokens = estimateTokens(content);

        if (totalTokens > maxTokens) {
            // Rebuild with budget — truncate chunk sections
            const headerParts: string[] = ["# Project Context\n"];
            let usedTokens = estimateTokens(headerParts[0]!);
            let includedChunks = 0;
            let omittedChunks = 0;

            if (chunks.length === 0) {
                headerParts.push(`No chunks found with tag "${tagName}".\n`);
            } else {
                const sections = new Map<string, ChunkRow[]>();
                for (const c of chunks) {
                    const label = sectionLabel(c.type);
                    const group = sections.get(label) ?? [];
                    group.push(c);
                    sections.set(label, group);
                }

                const sectionOrder = ["Conventions", "Architecture", "References", "Other"];
                for (const sectionName of sectionOrder) {
                    const group = sections.get(sectionName);
                    if (!group || group.length === 0) continue;
                    const sectionHeader = `## ${sectionName}\n`;
                    const sectionTokens = estimateTokens(sectionHeader);
                    if (usedTokens + sectionTokens > maxTokens) {
                        omittedChunks += group.length;
                        continue;
                    }
                    headerParts.push(sectionHeader);
                    usedTokens += sectionTokens;

                    for (const c of group) {
                        const entry = formatChunkEntry(c);
                        const entryTokens = estimateTokens(entry);
                        if (usedTokens + entryTokens > maxTokens) {
                            omittedChunks++;
                            continue;
                        }
                        headerParts.push(entry);
                        usedTokens += entryTokens;
                        includedChunks++;
                    }
                }
            }

            // Still include requirements and plans sections if budget allows
            // (they're already in parts — check remaining budget)

            if (omittedChunks > 0) {
                headerParts.push(
                    `<!-- Truncated: ${omittedChunks} chunks omitted due to token budget. Increase maxTokens or narrow the tag filter. -->`
                );
            }

            content = headerParts.join("\n\n");
        }

        return { content, chunks: chunks.length };
```

- [ ] **Step 2: Add maxTokens query param to route**

In `packages/api/src/context-export/routes.ts`, update the claude-md route:

```typescript
    .get(
        "/chunks/export/claude-md",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        generateClaudeMd({
                            userId: session.user.id,
                            codebaseId: ctx.query.codebaseId,
                            tag: ctx.query.tag,
                            maxTokens: ctx.query.maxTokens ? Number(ctx.query.maxTokens) : undefined,
                        })
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                tag: t.Optional(t.String()),
                maxTokens: t.Optional(t.String()),
            })
        }
    );
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context/claude-md.ts packages/api/src/context-export/routes.ts
git commit -m "feat: add token budget guard to CLAUDE.md generation"
```

---

### Task 14: Automatic Staleness Scanning

Spec item 3e. Run age-based staleness scan on server startup and recurring interval.

**Files:**
- Create: `packages/api/src/startup.ts`
- Modify: `packages/api/src/index.ts` (import startup)
- Modify: `packages/env/src/server.ts` (add env var)

- [ ] **Step 1: Add env var**

In `packages/env/src/server.ts`, add to the `server` object:

```typescript
STALENESS_SCAN_INTERVAL_HOURS: type("string | undefined"),
```

And to `runtimeEnv`:

```typescript
STALENESS_SCAN_INTERVAL_HOURS: process.env.STALENESS_SCAN_INTERVAL_HOURS,
```

- [ ] **Step 2: Create startup module**

Create `packages/api/src/startup.ts`:

```typescript
import { detectAgeStaleChunks } from "@fubbik/db/repository";
import { env } from "@fubbik/env/server";
import { Effect } from "effect";

import { logger } from "./logger";

const DEV_USER_ID = "dev-user";

async function runStaleScan() {
    const start = Date.now();
    try {
        const result = await Effect.runPromise(detectAgeStaleChunks(DEV_USER_ID));
        const duration = Date.now() - start;
        logger.info("Staleness scan completed", {
            flagged: result.flagged,
            durationMs: duration,
        });
    } catch (err) {
        logger.error("Staleness scan failed", { error: err });
    }
}

export function initStartupTasks() {
    const intervalHours = Number(env.STALENESS_SCAN_INTERVAL_HOURS ?? "24");
    if (intervalHours <= 0) {
        logger.info("Staleness scanning disabled (STALENESS_SCAN_INTERVAL_HOURS=0)");
        return;
    }

    // Run initial scan after a short delay (don't block startup)
    setTimeout(() => {
        runStaleScan();
    }, 5000);

    // Schedule recurring scans
    const intervalMs = intervalHours * 60 * 60 * 1000;
    setInterval(() => {
        runStaleScan();
    }, intervalMs);

    logger.info(`Staleness scanning enabled (every ${intervalHours}h)`);
}
```

- [ ] **Step 3: Call from server bootstrap**

In `packages/api/src/index.ts`, add at the bottom (after the `api` export):

```typescript
import { initStartupTasks } from "./startup";

// Fire-and-forget startup tasks (staleness scan, etc.)
initStartupTasks();
```

- [ ] **Step 4: Verify types**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/startup.ts packages/api/src/index.ts packages/env/src/server.ts
git commit -m "feat: add automatic staleness scanning on server startup"
```

---

## Phase 4: Organize — Query Semantics, Schema & Consistency

### Task 15: Tag AND Semantics

Spec item 4a. Add `tagMode=all` option to `listChunks` tag filtering.

**Files:**
- Modify: `packages/db/src/repository/chunk.ts`
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Add tagMode parameter to listChunks**

In `packages/db/src/repository/chunk.ts`, add `tagMode?: "any" | "all"` to `ListChunksParams`.

Update the tag filtering block (around line 63-70):

```typescript
if (params.tags && params.tags.length > 0) {
    if (params.tagMode === "all") {
        // AND semantics: chunk must have ALL specified tags
        const tagSubquery = db
            .select({ chunkId: chunkTag.chunkId })
            .from(chunkTag)
            .innerJoin(tag, eq(chunkTag.tagId, tag.id))
            .where(inArray(tag.name, params.tags))
            .groupBy(chunkTag.chunkId)
            .having(sql`COUNT(DISTINCT ${tag.name}) = ${params.tags.length}`);
        conditions.push(sql`${chunk.id} IN (${tagSubquery})`);
    } else {
        // OR semantics (default): chunk has at least one of the tags
        const tagSubquery = db
            .select({ chunkId: chunkTag.chunkId })
            .from(chunkTag)
            .innerJoin(tag, eq(chunkTag.tagId, tag.id))
            .where(inArray(tag.name, params.tags));
        conditions.push(sql`${chunk.id} IN (${tagSubquery})`);
    }
}
```

- [ ] **Step 2: Add tagMode query param to chunks route**

In `packages/api/src/chunks/routes.ts`, add to the list endpoint's query schema:

```typescript
tagMode: t.Optional(t.Union([t.Literal("any"), t.Literal("all")])),
```

And pass it to `listChunks`:

```typescript
tagMode: ctx.query.tagMode,
```

- [ ] **Step 3: Verify types and run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types && pnpm test`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/chunk.ts packages/api/src/chunks/routes.ts
git commit -m "feat: add tagMode=all for AND semantics in tag filtering"
```

---

### Task 16: Codebase Scoping Consistency

Spec item 4c. Add `codebaseId` filter to `lookupChunksByFilePath`.

**Files:**
- Modify: `packages/db/src/repository/file-ref.ts`
- Modify: `packages/api/src/context-for-file/service.ts`

- [ ] **Step 1: Add optional codebaseId to lookupChunksByFilePath**

In `packages/db/src/repository/file-ref.ts`, update `lookupChunksByFilePath`:

```typescript
export function lookupChunksByFilePath(path: string, userId: string, codebaseId?: string) {
    return dbEffect(() => {
        const conditions = [eq(chunkFileRef.path, path), eq(chunk.userId, userId)];

        if (codebaseId) {
            // Include chunks in this codebase OR global chunks (not in any codebase)
            const inCodebase = db
                .select({ chunkId: chunkCodebase.chunkId })
                .from(chunkCodebase)
                .where(eq(chunkCodebase.codebaseId, codebaseId));
            const inAnyCodebase = db
                .select({ chunkId: chunkCodebase.chunkId })
                .from(chunkCodebase);
            conditions.push(
                sql`(${chunk.id} IN (${inCodebase}) OR ${chunk.id} NOT IN (${inAnyCodebase}))`
            );
        }

        return db
            .select({
                chunkId: chunk.id,
                chunkTitle: chunk.title,
                chunkType: chunk.type,
                refId: chunkFileRef.id,
                path: chunkFileRef.path,
                anchor: chunkFileRef.anchor,
                relation: chunkFileRef.relation
            })
            .from(chunkFileRef)
            .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
            .where(and(...conditions));
    });
}
```

Add the necessary imports: `chunkCodebase` from schema, `sql` from drizzle-orm.

- [ ] **Step 2: Pass codebaseId in service**

In `packages/api/src/context-for-file/service.ts`, update the Strategy 1 call:

```typescript
const fileRefMatches = yield* lookupChunksByFilePath(filePath, userId, codebaseId);
```

- [ ] **Step 3: Verify types and run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types && pnpm test`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/file-ref.ts packages/api/src/context-for-file/service.ts
git commit -m "fix: apply codebase scoping to file-ref lookups for consistency"
```

---

### Task 17: Better Token Estimation

Spec item 4d. Replace `chars / 4` with `js-tiktoken`.

**Files:**
- Modify: `packages/api/src/context/utils.ts`
- Modify: `packages/api/package.json`

- [ ] **Step 1: Install js-tiktoken**

Run: `cd /Users/pontus/projects/fubbik && pnpm add js-tiktoken --filter @fubbik/api`

- [ ] **Step 2: Update estimateTokens in utils**

In `packages/api/src/context/utils.ts`, replace the `estimateTokens` function:

```typescript
import { encodingForModel } from "js-tiktoken";

let encoder: ReturnType<typeof encodingForModel> | null = null;

function getEncoder() {
    if (!encoder) {
        try {
            encoder = encodingForModel("gpt-4o");
        } catch {
            // Fall back to char-based estimation if tokenizer fails to load
            return null;
        }
    }
    return encoder;
}

export function estimateTokens(text: string): number {
    const enc = getEncoder();
    if (enc) {
        return enc.encode(text).length;
    }
    // Fallback: rough approximation
    return Math.ceil(text.length / 4);
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

Expected: Tests pass. Token counts may differ slightly from old estimates, but budget tests in `service.test.ts` use relative comparisons (≤ maxTokens) so they should be fine.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context/utils.ts packages/api/package.json pnpm-lock.yaml
git commit -m "feat: use js-tiktoken for accurate token estimation"
```

---

### Task 18: Connection-Aware Retrieval

Spec item 4e. Expand results to include one-hop connected chunks.

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`

- [ ] **Step 1: Add connection expansion after all strategies**

In `packages/api/src/context-for-file/service.ts`, after Strategy 4 (semantic) and before the scoring block, add:

```typescript
        // 5. Connection expansion — add tightly-coupled chunks
        const currentIds = Array.from(results.keys());
        if (currentIds.length > 0) {
            const allConnections = yield* getConnectionsForChunks(currentIds).pipe(
                Effect.catchAll(() => Effect.succeed([] as Array<{ id: string; sourceId: string; targetId: string; relation: string }>)),
            );

            // Prioritize by relation type
            const RELATION_PRIORITY: Record<string, number> = {
                part_of: 4,
                depends_on: 3,
                extends: 2,
                references: 1,
                related_to: 0,
            };

            // Collect candidate connected chunk IDs
            const candidates: Array<{ chunkId: string; priority: number }> = [];
            for (const conn of allConnections) {
                const connectedId = currentIds.includes(conn.sourceId) ? conn.targetId : conn.sourceId;
                if (!results.has(connectedId)) {
                    candidates.push({
                        chunkId: connectedId,
                        priority: RELATION_PRIORITY[conn.relation] ?? 0,
                    });
                }
            }

            // Sort by priority descending, take top 5
            candidates.sort((a, b) => b.priority - a.priority);
            const topCandidates = candidates.slice(0, 5);

            for (const candidate of topCandidates) {
                const full = yield* getChunkById(candidate.chunkId, userId).pipe(
                    Effect.catchAll(() => Effect.succeed(null)),
                );
                if (!full) continue;
                results.set(candidate.chunkId, {
                    id: full.id,
                    title: full.title,
                    type: full.type,
                    content: full.content,
                    summary: full.summary,
                    matchReason: "connected",
                    score: 0,
                });
                chunkRows.set(full.id, full);
            }
        }
```

Add the import:
```typescript
import { getConnectionsForChunks } from "@fubbik/db/repository";
```

Note: `getConnectionsForChunks` is already imported if you added it for the scoring step in Task 10. The function returns connection rows — check the exact return type from `packages/db/src/repository/connection.ts`.

- [ ] **Step 2: Run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm test`

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/context-for-file/service.ts
git commit -m "feat: add connection-aware retrieval to context-for-file"
```

---

### Task 19: Scope Schema Registry

Spec item 4b. New table, repository, and routes for scope key management.

**Files:**
- Create: `packages/db/src/schema/scope-key.ts`
- Create: `packages/db/src/repository/scope-key.ts`
- Create: `packages/api/src/scope-keys/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create schema**

Create `packages/db/src/schema/scope-key.ts`:

```typescript
import { pgTable, text, timestamp, uuid, jsonb, unique } from "drizzle-orm/pg-core";
import { user } from "./auth";

export const scopeKey = pgTable("scope_key", {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id").notNull().references(() => user.id, { onDelete: "cascade" }),
    key: text("key").notNull(),
    description: text("description"),
    valueType: text("value_type").notNull().default("string"), // string | number | boolean | enum
    allowedValues: jsonb("allowed_values"), // for enum type
    createdAt: timestamp("created_at").notNull().defaultNow(),
}, (table) => [
    unique("scope_key_user_key_unique").on(table.userId, table.key),
]);
```

- [ ] **Step 2: Create repository**

Create `packages/db/src/repository/scope-key.ts`:

```typescript
import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { scopeKey } from "../schema/scope-key";

export function listScopeKeys(userId: string) {
    return dbEffect(() =>
        db
            .select()
            .from(scopeKey)
            .where(eq(scopeKey.userId, userId))
            .orderBy(scopeKey.key),
    );
}

export function createScopeKey(params: {
    id: string;
    userId: string;
    key: string;
    description?: string;
    valueType?: string;
    allowedValues?: unknown;
}) {
    return dbEffect(async () => {
        const [created] = await db
            .insert(scopeKey)
            .values({
                id: params.id,
                userId: params.userId,
                key: params.key,
                description: params.description ?? null,
                valueType: params.valueType ?? "string",
                allowedValues: params.allowedValues ?? null,
            })
            .returning();
        return created!;
    });
}

export function deleteScopeKey(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(scopeKey)
            .where(and(eq(scopeKey.id, id), eq(scopeKey.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}
```

- [ ] **Step 3: Create routes**

Create `packages/api/src/scope-keys/routes.ts`:

```typescript
import { createScopeKey, deleteScopeKey, listScopeKeys } from "@fubbik/db/repository/scope-key";
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { NotFoundError } from "../errors";
import { requireSession } from "../require-session";

export const scopeKeyRoutes = new Elysia()
    .get(
        "/scope-keys",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => listScopeKeys(session.user.id)),
                ),
            ),
    )
    .post(
        "/scope-keys",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        createScopeKey({
                            id: crypto.randomUUID(),
                            userId: session.user.id,
                            key: ctx.body.key,
                            description: ctx.body.description,
                            valueType: ctx.body.valueType,
                            allowedValues: ctx.body.allowedValues,
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                key: t.String(),
                description: t.Optional(t.String()),
                valueType: t.Optional(
                    t.Union([
                        t.Literal("string"),
                        t.Literal("number"),
                        t.Literal("boolean"),
                        t.Literal("enum"),
                    ]),
                ),
                allowedValues: t.Optional(t.Array(t.String())),
            }),
        },
    )
    .delete(
        "/scope-keys/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        deleteScopeKey(ctx.params.id, session.user.id).pipe(
                            Effect.flatMap(deleted =>
                                deleted
                                    ? Effect.succeed({ deleted: true })
                                    : Effect.fail(new NotFoundError({ resource: "ScopeKey" })),
                            ),
                        ),
                    ),
                ),
            ),
        {
            params: t.Object({ id: t.String() }),
        },
    );
```

- [ ] **Step 4: Register routes in index.ts**

In `packages/api/src/index.ts`, add:

```typescript
import { scopeKeyRoutes } from "./scope-keys/routes";
```

And add `.use(scopeKeyRoutes)` in the route chain.

- [ ] **Step 5: Export from repository index**

Ensure `packages/db/src/repository/index.ts` re-exports `scope-key.ts`. Add:

```typescript
export { listScopeKeys, createScopeKey, deleteScopeKey } from "./scope-key";
```

- [ ] **Step 6: Push schema**

Run: `cd /Users/pontus/projects/fubbik && pnpm db:push`

- [ ] **Step 7: Verify types and run tests**

Run: `cd /Users/pontus/projects/fubbik && pnpm run check-types && pnpm test`

- [ ] **Step 8: Commit**

```bash
git add packages/db/src/schema/scope-key.ts packages/db/src/repository/scope-key.ts packages/api/src/scope-keys/routes.ts packages/api/src/index.ts packages/db/src/repository/index.ts
git commit -m "feat: add scope schema registry for structured scope key management"
```

---

### Task 20: Update CLAUDE.md Documentation

Spec item 4f. Document all changes.

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the Context Modules section**

In `CLAUDE.md`, update the "Context Modules" section under "Architecture Patterns" to describe the unified pipeline:

```markdown
### Context Pipeline (Unified)

All context retrieval flows through a single pipeline in `packages/api/src/context/`:

```
Input Source → Chunk Resolver → Enrichment (health, stale, features) → Scorer + Budgeter → Formatter → Output
```

- **Resolvers** (`context/resolvers.ts`): `resolveForPlan`, `resolveForConcept`, `resolveForFiles`. Each produces candidate chunk IDs.
- **Enrichment** (`enrichChunks`): Fetches full rows, connections, tags, stale flags, proposals, active feature overlays. Computes health scores.
- **Scoring** (`context/utils.ts`): `scoreChunk` combines health, type, rationale, connections, review status. `budgetChunks` greedily fills a token budget.
- **Formatting** (`context/formatter.ts`): Groups by section, adds `[health: N]`, `⚠ STALE`, `⚠ PENDING PROPOSAL` annotations.
- **Low-level retrieval** (`context-for-file/service.ts`): Five strategies — file-ref (+20 bonus), applies-to (+10), dependency (+3), semantic (+5, requires Ollama), connected (+2). Results scored and sorted.
- **CLAUDE.md generation** (`context/claude-md.ts`): Tag-based export with requirements and active plans. Supports `maxTokens` budget.
- **Snapshots** (`context/snapshot-service.ts`): Frozen context persisted as JSONB. User-scoped (auth-checked on retrieval).
```

- [ ] **Step 2: Add new API endpoints to the reference**

Add to the API Endpoints section under "Context":

```markdown
### Context
- `GET /api/context/for-file?path=<path>&codebaseId=<id>&deps=<csv>&format=<fmt>&maxTokens=<n>` — chunks relevant to a file (five strategies: file-ref, applies-to, dependency, semantic, connected)
- `GET /api/context/for-plan?planId=<id>&maxTokens=<n>&format=<fmt>` — chunks linked to a plan
- `GET /api/context/about?q=<concept>&maxTokens=<n>&codebaseId=<id>&format=<fmt>` — semantic + text search for a concept
- `GET /api/context/for-files?paths=<csv>&maxTokens=<n>&codebaseId=<id>&format=<fmt>` — chunks for multiple files
- `POST /api/context/snapshot` — create frozen context snapshot
- `GET /api/context/snapshot/:id` — retrieve snapshot (user-scoped)
- `GET /api/context/snapshots` — list user's snapshots
- `DELETE /api/context/snapshot/:id` — delete snapshot
- `GET /api/scope-keys` — list registered scope keys
- `POST /api/scope-keys` — register a scope key
- `DELETE /api/scope-keys/:id` — delete a scope key
```

- [ ] **Step 3: Document new parameters**

Add to Environment Variables section:

```markdown
- `STALENESS_SCAN_INTERVAL_HOURS` — Hours between automatic staleness scans (default: `24`, set to `0` to disable)
```

Add notes about `tagMode` and `maxTokens` in the relevant API endpoint descriptions.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for unified context pipeline and new features"
```

---

## Summary

| Phase | Tasks | Key Changes |
|-------|-------|-------------|
| **1: Fix** | Tasks 1-4 | MCP tool registration, snapshot auth, N+1 batch fix, CLI batching |
| **2: Consolidate** | Tasks 5-8 | Single scoreChunk, routes through resolvers, unified CLAUDE.md |
| **3: Enhance** | Tasks 9-14 | Path normalization, scoring, semantic search, feature overlays, token budget, auto-staleness |
| **4: Organize** | Tasks 15-20 | Tag AND mode, codebase scoping, tiktoken, connection expansion, scope registry, docs |
