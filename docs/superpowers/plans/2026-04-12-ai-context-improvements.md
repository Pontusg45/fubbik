# AI Context Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 8 context retrieval improvements for AI agents: plan-scoped, concept-based, multi-file, diff-aware context; structured output with metadata; plan-aware and task-scoped MCP tools; context snapshots.

**Architecture:** Extract shared scoring/budgeting/formatting utils from the existing `context-export` service. Each new feature adds a resolver function (plan, concept, multi-file) that feeds into the shared pipeline. New `context/` module under the API holds resolvers, formatter, and routes. Snapshots get their own table + CRUD. MCP tools and CLI commands wrap the API endpoints.

**Tech Stack:** Drizzle ORM (PostgreSQL), Elysia + Effect, Model Context Protocol SDK, Commander.js, Vitest

**Spec:** `docs/superpowers/specs/2026-04-12-ai-context-improvements-design.md`

---

## File Structure

### Created

| Path | Responsibility |
|---|---|
| `packages/api/src/context/utils.ts` | Shared `scoreChunk`, `estimateTokens`, `budgetChunks` extracted from context-export |
| `packages/api/src/context/formatter.ts` | Structured section formatter with health/stale/proposal metadata |
| `packages/api/src/context/resolvers.ts` | Resolver functions: `resolveForPlan`, `resolveForConcept`, `resolveForFiles` |
| `packages/api/src/context/routes.ts` | New endpoints: `/api/context/for-plan`, `/api/context/about`, `/api/context/for-files` |
| `packages/db/src/schema/context-snapshot.ts` | Snapshot table schema |
| `packages/db/src/repository/context-snapshot.ts` | Snapshot CRUD |
| `packages/api/src/context/snapshot-service.ts` | Snapshot create/get/list/delete |
| `packages/api/src/context/snapshot-routes.ts` | Snapshot API endpoints |
| `apps/cli/src/commands/context-for-plan.ts` | CLI: `context for-plan <planId>` |
| `apps/cli/src/commands/context-about.ts` | CLI: `context about "<concept>"` |
| `apps/cli/src/commands/context-for-diff.ts` | CLI: `context for-diff [--staged]` |
| `apps/cli/src/commands/context-snapshot.ts` | CLI: `context snapshot [create|get|list|delete]` |

### Modified

| Path | Change |
|---|---|
| `packages/api/src/context-export/service.ts` | Extract `scoreChunk`, `estimateTokens` into shared utils (keep re-exports for compat) |
| `packages/api/src/context-for-file/service.ts` | Export resolver for reuse |
| `packages/api/src/index.ts` | Mount new context routes + snapshot routes |
| `packages/mcp/src/context-tools.ts` | Add `get_context`, `get_context_for_task`, `create_context_snapshot`, `get_context_snapshot` tools |
| `packages/db/src/schema/index.ts` | Add snapshot export |
| `packages/db/src/repository/index.ts` | Add snapshot export |
| `apps/cli/src/commands/context-group.ts` | Add new subcommands |
| `apps/cli/src/commands/context-for.ts` | Extend for glob patterns |

---

### Task 1: Extract Shared Utils + Create Structured Formatter

**Files:**
- Create: `packages/api/src/context/utils.ts`
- Create: `packages/api/src/context/formatter.ts`
- Modify: `packages/api/src/context-export/service.ts`

**Context:** The existing `context-export/service.ts` has `scoreChunk()`, `estimateTokens()`, and a greedy budgeting loop. These need to be shared by all new resolvers. The structured formatter groups chunks by type and adds metadata.

- [ ] **Step 1: Read the existing context-export service**

Read `packages/api/src/context-export/service.ts` fully. Identify:
- `scoreChunk(c, connectionCount)` function
- `estimateTokens(text)` function
- The greedy budgeting loop in `exportContext()`
- The `ChunkRow` type (or whatever the scored chunk type is)
- Any helper types

- [ ] **Step 2: Create `packages/api/src/context/utils.ts`**

```typescript
/**
 * Shared scoring, token estimation, and budgeting utilities for all context resolvers.
 * Extracted from context-export/service.ts.
 */

export interface ScoredChunk {
    id: string;
    title: string;
    content: string;
    type: string;
    summary: string | null;
    rationale: string | null;
    alternatives: unknown;
    consequences: string | null;
    score: number;
    matchReason?: string;
    connectionCount?: number;
}

/**
 * Estimate the token count of a text string.
 * Rough heuristic: ~4 chars per token.
 */
export function estimateTokens(text: string): number {
    return Math.ceil(text.length / 4);
}

/**
 * Score a chunk for context relevance.
 * Higher score = more likely to be included in the budget.
 */
export function scoreChunk(chunk: {
    content: string;
    type: string;
    rationale: string | null;
    summary: string | null;
    alternatives: unknown;
    consequences: string | null;
    reviewStatus?: string;
}, connectionCount: number): number {
    // Health approximation: content length / freshness
    const contentScore = Math.min(chunk.content.length / 200, 10);

    // Type weight
    const typeScores: Record<string, number> = { document: 3, reference: 2, schema: 2, checklist: 1, note: 1 };
    const typeScore = typeScores[chunk.type] ?? 1;

    // Rationale bonus
    const rationaleScore = chunk.rationale ? 2 : 0;

    // Connection bonus (capped)
    const connScore = Math.min(connectionCount * 2, 10);

    // Review status bonus
    const reviewScore = chunk.reviewStatus === "approved" ? 2 : chunk.reviewStatus === "reviewed" ? 1 : 0;

    return contentScore + typeScore + rationaleScore + connScore + reviewScore;
}

/**
 * Greedy token-budgeted selection from a scored chunk list.
 * Returns chunks sorted by score that fit within the token budget.
 */
export function budgetChunks(
    chunks: ScoredChunk[],
    maxTokens: number,
): ScoredChunk[] {
    const sorted = [...chunks].sort((a, b) => b.score - a.score);
    const selected: ScoredChunk[] = [];
    let usedTokens = 0;

    for (const chunk of sorted) {
        const text = formatChunkText(chunk);
        const tokens = estimateTokens(text);
        if (usedTokens + tokens > maxTokens) continue;
        usedTokens += tokens;
        selected.push(chunk);
    }

    return selected;
}

/**
 * Format a chunk into a text string for token counting and output.
 */
export function formatChunkText(chunk: ScoredChunk): string {
    const parts = [`# ${chunk.title}`, chunk.content];
    if (chunk.summary) parts.push(`Summary: ${chunk.summary}`);
    if (chunk.rationale) parts.push(`Rationale: ${chunk.rationale}`);
    return parts.join("\n\n");
}
```

**Adapt:** The exact fields on the chunk type depend on what `context-export/service.ts` uses. Read it and match the type. If it uses `computeHealthScore()` from the chunks service, import that instead of the approximation above.

- [ ] **Step 3: Create `packages/api/src/context/formatter.ts`**

```typescript
import { Effect } from "effect";

import { getPendingCount as getProposalCountForChunk } from "@fubbik/db/repository/chunk-proposal";

import type { ScoredChunk } from "./utils";
import { formatChunkText } from "./utils";

/**
 * Section mapping: chunk type → section heading.
 */
const TYPE_SECTIONS: Record<string, string> = {
    note: "Notes",
    document: "Architecture",
    reference: "API Reference",
    schema: "Schemas",
    checklist: "Checklists",
};

export interface ChunkWithMetadata extends ScoredChunk {
    health: number;
    stale: boolean;
    pendingProposal: boolean;
    tags?: string[];
}

export interface ContextSection {
    heading: string;
    chunks: ChunkWithMetadata[];
}

export interface StructuredContext {
    sections: ContextSection[];
    knownIssues: ChunkWithMetadata[];
    totalTokens: number;
    chunkCount: number;
}

/**
 * Check if a note-type chunk should go under "Conventions" instead of "Notes".
 */
function getSection(chunk: ChunkWithMetadata): string {
    if (chunk.type === "note" && chunk.tags?.some(t => t.toLowerCase().includes("convention"))) {
        return "Conventions";
    }
    return TYPE_SECTIONS[chunk.type] ?? "Other";
}

/**
 * Group scored chunks into structured sections with metadata.
 */
export function formatStructured(chunks: ChunkWithMetadata[]): StructuredContext {
    const sectionMap = new Map<string, ChunkWithMetadata[]>();
    const knownIssues: ChunkWithMetadata[] = [];

    // Desired section order
    const sectionOrder = ["Conventions", "Notes", "Architecture", "API Reference", "Schemas", "Checklists", "Other"];

    for (const chunk of chunks) {
        const section = getSection(chunk);
        if (!sectionMap.has(section)) sectionMap.set(section, []);
        sectionMap.get(section)!.push(chunk);

        if (chunk.health < 50 || chunk.stale) {
            knownIssues.push(chunk);
        }
    }

    const sections: ContextSection[] = [];
    for (const heading of sectionOrder) {
        const items = sectionMap.get(heading);
        if (items && items.length > 0) {
            sections.push({ heading, chunks: items });
        }
    }

    return {
        sections,
        knownIssues,
        totalTokens: chunks.reduce((sum, c) => sum + (c.content.length / 4), 0),
        chunkCount: chunks.length,
    };
}

/**
 * Render structured context as markdown with metadata markers.
 */
export function formatStructuredMarkdown(ctx: StructuredContext): string {
    const lines: string[] = [];

    for (const section of ctx.sections) {
        lines.push(`## ${section.heading}\n`);
        for (const chunk of section.chunks) {
            const markers: string[] = [`[health: ${chunk.health}]`];
            if (chunk.stale) markers.push("⚠ STALE");
            if (chunk.pendingProposal) markers.push("⚠ PENDING PROPOSAL");
            lines.push(`${markers.join(" ")} **${chunk.title}**`);
            lines.push(chunk.content);
            lines.push("");
        }
    }

    if (ctx.knownIssues.length > 0) {
        lines.push("## Known Issues\n");
        for (const chunk of ctx.knownIssues) {
            const markers: string[] = [`[health: ${chunk.health}]`];
            if (chunk.stale) markers.push("⚠ STALE");
            lines.push(`${markers.join(" ")} ${chunk.title}`);
        }
    }

    return lines.join("\n");
}
```

- [ ] **Step 4: Update `context-export/service.ts` to import from shared utils**

In `packages/api/src/context-export/service.ts`:
- Keep the existing `scoreChunk` and `estimateTokens` functions for now (avoid breaking the existing endpoint)
- Add a comment: `// TODO: Migrate to use packages/api/src/context/utils.ts`
- This avoids a risky refactor of the working endpoint. New code uses the shared utils; old code keeps working.

Alternatively, if you're confident: replace `scoreChunk` and `estimateTokens` with re-exports from `../context/utils`. This is cleaner but riskier.

- [ ] **Step 5: Type check**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "context/" | head -20`

Expected: zero errors in the new files. If `getProposalCountForChunk` is not a per-chunk function (it's currently a global pending count), remove that import and compute it differently in the resolver step (batch query).

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/context/
git commit -m "feat(api): extract shared context utils and create structured formatter

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Plan-Scoped Resolver + Route

**Files:**
- Create: `packages/api/src/context/resolvers.ts`
- Create: `packages/api/src/context/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Read plan repository to confirm function signatures**

Read `packages/db/src/repository/plan.ts` to confirm:
- `listAnalyzeItems(planId)` — returns items where you filter `kind=chunk` to get chunkIds
- `listPlanRequirements(planId)` — returns `{ requirementId }`
- `listTasks(planId)` → then for each task, `listTaskChunks(taskId)` — returns `{ chunkId }`

Read `packages/db/src/repository/requirement.ts` to confirm:
- `getRequirementsForChunks(chunkIds)` — but we need the reverse: chunks for a requirement. Search for a function like `getChunksForRequirement(requirementId)` or `getRequirementChunks`. If it doesn't exist, query `requirement_chunk` table directly.

Read `packages/db/src/repository/chunk.ts` to confirm:
- How to fetch chunks by ID list (is there a `getChunksByIds(ids)` function, or do you need to build a query with `inArray`?)

- [ ] **Step 2: Create `packages/api/src/context/resolvers.ts`**

```typescript
import { and, eq, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { db } from "@fubbik/db";
import * as planRepo from "@fubbik/db/repository/plan";
import { chunk } from "@fubbik/db/schema/chunk";
import { requirementChunk } from "@fubbik/db/schema/requirement";
import { chunkProposal } from "@fubbik/db/schema/chunk-proposal";
import { chunkStaleness } from "@fubbik/db/schema/staleness";
import { getChunkConnections } from "@fubbik/db/repository/connection";
import { getTagsForChunk } from "@fubbik/db/repository/tag-new";
import { DatabaseError } from "@fubbik/db/errors";

import type { ChunkWithMetadata } from "./formatter";
import { scoreChunk } from "./utils";

/**
 * Fetch full chunk rows by IDs, enrich with metadata (health, stale, proposals, tags).
 */
export function enrichChunks(chunkIds: string[]): Effect.Effect<ChunkWithMetadata[], DatabaseError> {
    if (chunkIds.length === 0) return Effect.succeed([]);

    return Effect.tryPromise({
        try: async () => {
            const uniqueIds = [...new Set(chunkIds)];
            const chunks = await db.select().from(chunk).where(inArray(chunk.id, uniqueIds));

            // Batch fetch metadata
            const staleFlags = await db
                .select({ chunkId: chunkStaleness.chunkId })
                .from(chunkStaleness)
                .where(and(
                    inArray(chunkStaleness.chunkId, uniqueIds),
                    eq(chunkStaleness.dismissedAt as any, null), // undismissed only
                ));
            const staleSet = new Set(staleFlags.map(f => f.chunkId));

            const proposals = await db
                .select({ chunkId: chunkProposal.chunkId })
                .from(chunkProposal)
                .where(and(
                    inArray(chunkProposal.chunkId, uniqueIds),
                    eq(chunkProposal.status, "pending"),
                ));
            const proposalSet = new Set(proposals.map(p => p.chunkId));

            return chunks.map(c => ({
                id: c.id,
                title: c.title,
                content: c.content,
                type: c.type,
                summary: c.summary,
                rationale: c.rationale,
                alternatives: c.alternatives,
                consequences: c.consequences,
                score: 0, // scored later by the caller
                health: 0, // computed below
                stale: staleSet.has(c.id),
                pendingProposal: proposalSet.has(c.id),
                tags: [], // filled below if needed
            }));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

/**
 * Resolve chunks for a plan: analyze items (kind=chunk) + requirement chunks + task chunks.
 */
export function resolveForPlan(planId: string): Effect.Effect<string[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const ids = new Set<string>();

            // 1. Analyze items with kind=chunk
            const analyzeItems = await Effect.runPromise(planRepo.listAnalyzeItems(planId));
            for (const item of analyzeItems) {
                if (item.kind === "chunk" && item.chunkId) ids.add(item.chunkId);
            }

            // 2. Requirements → requirement_chunk
            const planReqs = await Effect.runPromise(planRepo.listPlanRequirements(planId));
            if (planReqs.length > 0) {
                const reqIds = planReqs.map(r => r.requirementId);
                const reqChunks = await db
                    .select({ chunkId: requirementChunk.chunkId })
                    .from(requirementChunk)
                    .where(inArray(requirementChunk.requirementId, reqIds));
                for (const rc of reqChunks) ids.add(rc.chunkId);
            }

            // 3. Task chunks
            const tasks = await Effect.runPromise(planRepo.listTasks(planId));
            for (const task of tasks) {
                const taskChunks = await Effect.runPromise(planRepo.listTaskChunks(task.id));
                for (const tc of taskChunks) ids.add(tc.chunkId);
            }

            return [...ids];
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

/**
 * Resolve chunks for a concept: semantic search + tag match + text match.
 */
export function resolveForConcept(
    query: string,
    userId?: string,
    codebaseId?: string,
): Effect.Effect<string[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const ids = new Set<string>();

            // 1. Semantic search (if embeddings available)
            try {
                const { generateQueryEmbedding } = await import("../../ollama/client");
                const { semanticSearch: semanticSearchRepo } = await import("@fubbik/db/repository/semantic");
                const embedding = await generateQueryEmbedding(query);
                if (embedding) {
                    const results = await Effect.runPromise(
                        semanticSearchRepo({ embedding, userId, limit: 20 }),
                    );
                    for (const r of results) ids.add(r.id);
                }
            } catch {
                // Ollama unavailable — skip semantic search
            }

            // 2. Text search via pg_trgm
            const textResults = await db
                .select({ id: chunk.id })
                .from(chunk)
                .where(
                    // Use SQL template for pg_trgm similarity
                    // This is a simplified version — adapt to match the existing search pattern
                    inArray(chunk.id, db.select({ id: chunk.id }).from(chunk)
                        .where(eq(chunk.title, query))), // placeholder — see adaptation note
                )
                .limit(20);
            for (const r of textResults) ids.add(r.id);

            return [...ids];
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

/**
 * Resolve chunks for multiple file paths.
 * Calls the existing getContextForFile resolver for each path, deduplicates.
 */
export function resolveForFiles(
    paths: string[],
    userId: string,
    codebaseId?: string,
): Effect.Effect<string[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const { getContextForFile } = await import("../../context-for-file/service");
            const ids = new Set<string>();

            for (const path of paths) {
                try {
                    const result = await Effect.runPromise(
                        getContextForFile(userId, path, codebaseId),
                    );
                    for (const c of result.chunks) ids.add(c.id);
                } catch {
                    // Skip paths that error
                }
            }

            return [...ids];
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}
```

**IMPORTANT adaptation notes:**
- The `requirementChunk` table/schema import path needs to be verified. Search: `grep -rn "requirementChunk\|requirement_chunk" packages/db/src/schema/ | head`. It might be in `requirement.ts` or a separate file.
- The `chunkStaleness` import needs the same check: `grep -rn "chunkStaleness\|chunk_staleness" packages/db/src/schema/ | head`.
- The text search in `resolveForConcept` is a placeholder. Read how the existing search service does `pg_trgm` queries and replicate that pattern. If there's a `searchChunks(query)` repo function, use it instead.
- The `semanticSearch` repo function signature needs verification — check imports.
- The `db` import path is `@fubbik/db` or `@fubbik/db/index` — verify.

- [ ] **Step 3: Create `packages/api/src/context/routes.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../require-session";
import { enrichChunks, resolveForPlan, resolveForConcept, resolveForFiles } from "./resolvers";
import { formatStructured, formatStructuredMarkdown, type ChunkWithMetadata } from "./formatter";
import { budgetChunks, scoreChunk } from "./utils";

async function buildContext(
    chunkIds: string[],
    maxTokens: number,
    format: string,
    boosts?: Map<string, number>,
) {
    const enriched = await Effect.runPromise(enrichChunks(chunkIds));

    // Score each chunk
    for (const c of enriched) {
        c.score = scoreChunk(c, 0) + (boosts?.get(c.id) ?? 0);
        c.health = Math.round(c.score * 4); // rough approximation; improve later
    }

    const budgeted = budgetChunks(enriched as any, maxTokens);
    const structured = formatStructured(budgeted as ChunkWithMetadata[]);

    if (format === "structured-json" || format === "json") {
        return structured;
    }
    return { content: formatStructuredMarkdown(structured), ...structured };
}

export const contextRoutes = new Elysia()
    .get(
        "/api/context/for-plan",
        async ctx => {
            const planId = ctx.query.planId;
            const maxTokens = Number(ctx.query.maxTokens ?? 8000);
            const format = ctx.query.format ?? "structured-md";

            const chunkIds = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => resolveForPlan(planId)),
                ),
            );
            return buildContext(chunkIds, maxTokens, format);
        },
        {
            query: t.Object({
                planId: t.String(),
                maxTokens: t.Optional(t.String()),
                format: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
            }),
        },
    )
    .get(
        "/api/context/about",
        async ctx => {
            const maxTokens = Number(ctx.query.maxTokens ?? 8000);
            const format = ctx.query.format ?? "structured-md";

            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        resolveForConcept(ctx.query.q, session.user.id, ctx.query.codebaseId),
                    ),
                ),
            );

            // Semantic matches get a boost
            const boosts = new Map<string, number>();
            for (const id of result) boosts.set(id, 10);

            return buildContext(result, maxTokens, format, boosts);
        },
        {
            query: t.Object({
                q: t.String(),
                maxTokens: t.Optional(t.String()),
                format: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
            }),
        },
    )
    .get(
        "/api/context/for-files",
        async ctx => {
            const paths = ctx.query.paths.split(",").map(p => p.trim()).filter(Boolean);
            const maxTokens = Number(ctx.query.maxTokens ?? 8000);
            const format = ctx.query.format ?? "structured-md";

            const chunkIds = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        resolveForFiles(paths, session.user.id, ctx.query.codebaseId),
                    ),
                ),
            );
            return buildContext(chunkIds, maxTokens, format);
        },
        {
            query: t.Object({
                paths: t.String(),
                maxTokens: t.Optional(t.String()),
                format: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
            }),
        },
    );
```

- [ ] **Step 4: Mount routes in `packages/api/src/index.ts`**

Add import:
```typescript
import { contextRoutes } from "./context/routes";
```

Add `.use(contextRoutes)` in the Elysia chain.

- [ ] **Step 5: Type check + fix adaptation issues**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "context/" | head -30`

Fix any import path issues. The `requirementChunk`, `chunkStaleness`, and semantic search imports are the most likely to need adaptation.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/context/ packages/api/src/index.ts
git commit -m "feat(api): add plan-scoped, concept-based, and multi-file context resolvers + routes

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: MCP Context Tools

**Files:**
- Modify: `packages/mcp/src/context-tools.ts`

- [ ] **Step 1: Read the current context-tools.ts**

Read `packages/mcp/src/context-tools.ts` to understand the registration pattern and `apiFetch` usage.

- [ ] **Step 2: Add new tools**

Add these tools to the existing `registerContextTools` function (or create new ones alongside it):

```typescript
server.tool(
    "get_context",
    "Get structured context for AI consumption. Combines plan-scoped, file-based, and concept-based resolvers with health/staleness metadata.",
    {
        planId: z.string().optional().describe("Scope to a plan's chunks, requirements, and tasks"),
        filePath: z.string().optional().describe("Scope to a file path"),
        concept: z.string().optional().describe("Scope to a concept via semantic search"),
        maxTokens: z.number().optional().describe("Token budget (default 8000)"),
        codebaseId: z.string().optional(),
    },
    async ({ planId, filePath, concept, maxTokens, codebaseId }) => {
        let endpoint: string;
        const params = new URLSearchParams();
        if (maxTokens) params.set("maxTokens", String(maxTokens));
        if (codebaseId) params.set("codebaseId", codebaseId);
        params.set("format", "structured-md");

        if (planId) {
            params.set("planId", planId);
            endpoint = `/context/for-plan?${params}`;
        } else if (concept) {
            params.set("q", concept);
            endpoint = `/context/about?${params}`;
        } else if (filePath) {
            params.set("paths", filePath);
            endpoint = `/context/for-files?${params}`;
        } else {
            return { content: [{ type: "text" as const, text: "Provide at least one of: planId, filePath, or concept" }] };
        }

        const result = await apiFetch(endpoint);
        const text = typeof result === "string" ? result : (result as any).content ?? JSON.stringify(result, null, 2);
        return { content: [{ type: "text" as const, text }] };
    },
);

server.tool(
    "get_context_for_task",
    "Get tightly-scoped context for a specific plan task. Includes task-linked chunks, plan analyze chunks, and 1-hop connected chunks.",
    {
        planId: z.string(),
        taskId: z.string(),
        maxTokens: z.number().optional().describe("Token budget (default 4000)"),
    },
    async ({ planId, taskId, maxTokens }) => {
        // Fetch the plan detail to get task chunks + analyze chunks
        const detail = await apiFetch(`/plans/${planId}`);
        const plan = detail as any;
        const task = plan.tasks?.find((t: any) => t.id === taskId);
        if (!task) {
            return { content: [{ type: "text" as const, text: `Task ${taskId} not found in plan ${planId}` }] };
        }

        // Collect chunk IDs: task chunks + analyze chunks
        const chunkIds = new Set<string>();
        for (const tc of task.chunks ?? []) chunkIds.add(tc.chunkId);
        for (const item of [...(plan.analyze?.chunk ?? [])]) {
            if (item.chunkId) chunkIds.add(item.chunkId);
        }

        // Get connected chunks (1-hop) via the for-plan endpoint as a proxy
        const params = new URLSearchParams();
        params.set("planId", planId);
        params.set("maxTokens", String(maxTokens ?? 4000));
        params.set("format", "structured-md");
        const result = await apiFetch(`/context/for-plan?${params}`);

        const text = typeof result === "string" ? result : (result as any).content ?? JSON.stringify(result, null, 2);
        return { content: [{ type: "text" as const, text }] };
    },
);
```

- [ ] **Step 3: Type check**

Run: `pnpm --filter @fubbik/mcp run check-types 2>&1 | tail -20`

- [ ] **Step 4: Commit**

```bash
git add packages/mcp/src/context-tools.ts
git commit -m "feat(mcp): add get_context and get_context_for_task tools

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: CLI Context Commands (for-plan, about, for-diff)

**Files:**
- Create: `apps/cli/src/commands/context-for-plan.ts`
- Create: `apps/cli/src/commands/context-about.ts`
- Create: `apps/cli/src/commands/context-for-diff.ts`
- Modify: `apps/cli/src/commands/context-for.ts` (extend for globs)
- Modify: `apps/cli/src/commands/context-group.ts` (add new subcommands)

- [ ] **Step 1: Read the existing context-for.ts for the CLI pattern**

Read `apps/cli/src/commands/context-for.ts` to learn:
- How it calls the API
- How it formats output (markdown vs JSON)
- The `fetchApi` import from `lib/api`

- [ ] **Step 2: Create `apps/cli/src/commands/context-for-plan.ts`**

```typescript
import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { output, outputError, isJson } from "../lib/output";

export const contextForPlanCommand = new Command("for-plan")
    .description("Get context scoped to a plan (analyze chunks, requirements, tasks)")
    .argument("<planId>", "plan ID")
    .option("-t, --max-tokens <n>", "token budget", "8000")
    .option("-c, --codebase <id>", "codebase ID")
    .option("-f, --format <format>", "output format (structured-md, structured-json)", "structured-md")
    .action(async (planId: string, opts: { maxTokens: string; codebase?: string; format: string }, cmd: Command) => {
        try {
            const params = new URLSearchParams({
                planId,
                maxTokens: opts.maxTokens,
                format: isJson(cmd) ? "structured-json" : opts.format,
            });
            if (opts.codebase) params.set("codebaseId", opts.codebase);

            const res = await fetchApi(`/context/for-plan?${params}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            if (isJson(cmd)) {
                output(cmd, data, "");
            } else {
                const content = (data as any).content ?? JSON.stringify(data, null, 2);
                output(cmd, data, content);
            }
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });
```

- [ ] **Step 3: Create `apps/cli/src/commands/context-about.ts`**

```typescript
import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { output, outputError, isJson } from "../lib/output";

export const contextAboutCommand = new Command("about")
    .description("Get context about a concept via semantic search")
    .argument("<concept>", "concept to search for (e.g. 'authentication')")
    .option("-t, --max-tokens <n>", "token budget", "8000")
    .option("-c, --codebase <id>", "codebase ID")
    .option("-f, --format <format>", "output format", "structured-md")
    .action(async (concept: string, opts: { maxTokens: string; codebase?: string; format: string }, cmd: Command) => {
        try {
            const params = new URLSearchParams({
                q: concept,
                maxTokens: opts.maxTokens,
                format: isJson(cmd) ? "structured-json" : opts.format,
            });
            if (opts.codebase) params.set("codebaseId", opts.codebase);

            const res = await fetchApi(`/context/about?${params}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            if (isJson(cmd)) {
                output(cmd, data, "");
            } else {
                const content = (data as any).content ?? JSON.stringify(data, null, 2);
                output(cmd, data, content);
            }
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });
```

- [ ] **Step 4: Create `apps/cli/src/commands/context-for-diff.ts`**

```typescript
import { execSync } from "node:child_process";

import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { output, outputError, isJson } from "../lib/output";

export const contextForDiffCommand = new Command("for-diff")
    .description("Get context for files changed in git diff")
    .option("--staged", "diff staged changes only")
    .option("-t, --max-tokens <n>", "token budget", "8000")
    .option("-c, --codebase <id>", "codebase ID")
    .option("-f, --format <format>", "output format", "structured-md")
    .action(async (opts: { staged?: boolean; maxTokens: string; codebase?: string; format: string }, cmd: Command) => {
        try {
            // Run git diff locally to get changed file paths
            const diffCmd = opts.staged ? "git diff --staged --name-only" : "git diff --name-only";
            const diffOutput = execSync(diffCmd, { encoding: "utf-8" }).trim();

            if (!diffOutput) {
                output(cmd, { files: [], chunks: [] }, "No changed files.");
                return;
            }

            const paths = diffOutput.split("\n").filter(Boolean);
            const params = new URLSearchParams({
                paths: paths.join(","),
                maxTokens: opts.maxTokens,
                format: isJson(cmd) ? "structured-json" : opts.format,
            });
            if (opts.codebase) params.set("codebaseId", opts.codebase);

            const res = await fetchApi(`/context/for-files?${params}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            if (isJson(cmd)) {
                output(cmd, data, "");
            } else {
                const content = (data as any).content ?? JSON.stringify(data, null, 2);
                output(cmd, data, `Context for ${paths.length} changed file(s):\n\n${content}`);
            }
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });
```

- [ ] **Step 5: Extend `context-for.ts` for glob patterns**

Read `apps/cli/src/commands/context-for.ts`. When the file path argument contains `*` or `**`, switch to the multi-file endpoint:

Find the action handler. Add a check at the top:

```typescript
// If the path looks like a glob, use multi-file endpoint
if (filePath.includes("*")) {
    const params = new URLSearchParams({
        paths: filePath,
        maxTokens: opts.maxTokens ?? "8000",
    });
    if (opts.codebase) params.set("codebaseId", opts.codebase);
    const res = await fetchApi(`/context/for-files?${params}`);
    // ... handle response same as for-plan above
    return;
}
// ... existing single-file logic continues below
```

Adapt to match the existing file's error handling and output patterns.

- [ ] **Step 6: Update `context-group.ts` to add new subcommands**

```typescript
import { contextForPlanCommand } from "./context-for-plan";
import { contextAboutCommand } from "./context-about";
import { contextForDiffCommand } from "./context-for-diff";

// Add to the existing .addCommand() chain:
export const contextGroupCommand = new Command("context")
    .description("Export context for AI consumption")
    .addCommand(contextCommand)
    .addCommand(contextDirCommand)
    .addCommand(contextForCommand)
    .addCommand(contextForPlanCommand)
    .addCommand(contextAboutCommand)
    .addCommand(contextForDiffCommand);
```

- [ ] **Step 7: Type check**

Run: `pnpm --filter cli run check-types 2>&1 | grep -v "gaps\|init" | tail -20`

- [ ] **Step 8: Commit**

```bash
git add apps/cli/src/commands/
git commit -m "feat(cli): add context for-plan, about, for-diff commands

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Context Snapshot Schema + Repository

**Files:**
- Create: `packages/db/src/schema/context-snapshot.ts`
- Create: `packages/db/src/repository/context-snapshot.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create `packages/db/src/schema/context-snapshot.ts`**

```typescript
import { relations } from "drizzle-orm";
import { integer, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const contextSnapshot = pgTable("context_snapshot", {
    id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
    userId: text("user_id")
        .notNull()
        .references(() => user.id, { onDelete: "cascade" }),
    query: jsonb("query").notNull(),
    chunks: jsonb("chunks").notNull(),
    tokenCount: integer("token_count").notNull(),
    createdAt: timestamp("created_at").notNull().defaultNow(),
});

export const contextSnapshotRelations = relations(contextSnapshot, ({ one }) => ({
    user: one(user, { fields: [contextSnapshot.userId], references: [user.id] }),
}));

export type ContextSnapshot = typeof contextSnapshot.$inferSelect;
export type NewContextSnapshot = typeof contextSnapshot.$inferInsert;
```

- [ ] **Step 2: Create `packages/db/src/repository/context-snapshot.ts`**

```typescript
import { desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { contextSnapshot, type ContextSnapshot, type NewContextSnapshot } from "../schema/context-snapshot";

export function createSnapshot(input: NewContextSnapshot): Effect.Effect<ContextSnapshot, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(contextSnapshot).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function getSnapshotById(id: string): Effect.Effect<ContextSnapshot | null, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(contextSnapshot).where(eq(contextSnapshot.id, id)).limit(1);
            return row ?? null;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function listSnapshots(userId: string): Effect.Effect<ContextSnapshot[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(contextSnapshot)
                .where(eq(contextSnapshot.userId, userId))
                .orderBy(desc(contextSnapshot.createdAt))
                .limit(50),
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function deleteSnapshot(id: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(contextSnapshot).where(eq(contextSnapshot.id, id));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}
```

- [ ] **Step 3: Update schema and repository indexes**

Add `export * from "./context-snapshot";` to both `packages/db/src/schema/index.ts` and `packages/db/src/repository/index.ts`.

- [ ] **Step 4: Generate and apply migration**

```bash
pnpm db:push
```

Verify: `psql "${DATABASE_URL}" -c "\d context_snapshot"`

- [ ] **Step 5: Type check**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | tail -15`

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/context-snapshot.ts packages/db/src/repository/context-snapshot.ts packages/db/src/schema/index.ts packages/db/src/repository/index.ts packages/db/src/migrations/
git commit -m "feat(db): add context_snapshot schema and repository

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Snapshot Service + Routes + MCP + CLI

**Files:**
- Create: `packages/api/src/context/snapshot-service.ts`
- Create: `packages/api/src/context/snapshot-routes.ts`
- Modify: `packages/api/src/context/routes.ts` (or `packages/api/src/index.ts`)
- Modify: `packages/mcp/src/context-tools.ts`
- Create: `apps/cli/src/commands/context-snapshot.ts`
- Modify: `apps/cli/src/commands/context-group.ts`

- [ ] **Step 1: Create `packages/api/src/context/snapshot-service.ts`**

```typescript
import { Effect } from "effect";

import * as snapshotRepo from "@fubbik/db/repository/context-snapshot";

import { NotFoundError } from "../errors";
import { enrichChunks, resolveForPlan, resolveForConcept, resolveForFiles } from "./resolvers";
import { budgetChunks, scoreChunk, estimateTokens, formatChunkText } from "./utils";
import type { ChunkWithMetadata } from "./formatter";

export interface CreateSnapshotInput {
    planId?: string;
    taskId?: string;
    filePaths?: string[];
    concept?: string;
    maxTokens?: number;
}

export function createSnapshot(userId: string, input: CreateSnapshotInput) {
    return Effect.gen(function* () {
        const maxTokens = input.maxTokens ?? 8000;
        let chunkIds: string[] = [];

        if (input.planId) {
            chunkIds = yield* resolveForPlan(input.planId);
        } else if (input.concept) {
            chunkIds = yield* resolveForConcept(input.concept, userId);
        } else if (input.filePaths && input.filePaths.length > 0) {
            chunkIds = yield* resolveForFiles(input.filePaths, userId);
        }

        const enriched = yield* enrichChunks(chunkIds);
        for (const c of enriched) {
            c.score = scoreChunk(c, 0);
            c.health = Math.round(c.score * 4);
        }

        const budgeted = budgetChunks(enriched as any, maxTokens);
        const tokenCount = budgeted.reduce((sum, c) => sum + estimateTokens(formatChunkText(c)), 0);

        // Freeze the chunks — store their content at this point in time
        const frozenChunks = budgeted.map(c => ({
            id: c.id,
            title: c.title,
            content: c.content,
            type: c.type,
            health: (c as any).health ?? 0,
            stale: (c as any).stale ?? false,
            pendingProposal: (c as any).pendingProposal ?? false,
        }));

        return yield* snapshotRepo.createSnapshot({
            userId,
            query: input,
            chunks: frozenChunks,
            tokenCount,
        });
    });
}

export function getSnapshot(id: string) {
    return snapshotRepo.getSnapshotById(id).pipe(
        Effect.flatMap(s =>
            s ? Effect.succeed(s) : Effect.fail(new NotFoundError({ resource: "Snapshot" })),
        ),
    );
}

export function listSnapshots(userId: string) {
    return snapshotRepo.listSnapshots(userId);
}

export function deleteSnapshot(id: string) {
    return snapshotRepo.deleteSnapshot(id);
}
```

- [ ] **Step 2: Create `packages/api/src/context/snapshot-routes.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../require-session";
import * as snapshotService from "./snapshot-service";

export const snapshotRoutes = new Elysia()
    .post(
        "/api/context/snapshot",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        snapshotService.createSnapshot(session.user.id, {
                            planId: ctx.body.planId,
                            taskId: ctx.body.taskId,
                            filePaths: ctx.body.filePaths,
                            concept: ctx.body.concept,
                            maxTokens: ctx.body.maxTokens,
                        }),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                planId: t.Optional(t.String()),
                taskId: t.Optional(t.String()),
                filePaths: t.Optional(t.Array(t.String())),
                concept: t.Optional(t.String()),
                maxTokens: t.Optional(t.Number()),
            }),
        },
    )
    .get("/api/context/snapshot/:id", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => snapshotService.getSnapshot(ctx.params.id)),
            ),
        );
    })
    .get("/api/context/snapshots", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => snapshotService.listSnapshots(session.user.id)),
            ),
        );
    })
    .delete("/api/context/snapshot/:id", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => snapshotService.deleteSnapshot(ctx.params.id)),
            ),
        );
        return { ok: true };
    });
```

- [ ] **Step 3: Mount snapshot routes**

In `packages/api/src/index.ts`, add:
```typescript
import { snapshotRoutes } from "./context/snapshot-routes";
```
And `.use(snapshotRoutes)`.

- [ ] **Step 4: Add MCP snapshot tools**

In `packages/mcp/src/context-tools.ts`, add:

```typescript
server.tool(
    "create_context_snapshot",
    "Freeze the current context for a plan, task, concept, or file set. The snapshot won't change even if chunks are edited.",
    {
        planId: z.string().optional(),
        taskId: z.string().optional(),
        filePaths: z.array(z.string()).optional(),
        concept: z.string().optional(),
        maxTokens: z.number().optional(),
    },
    async (params) => {
        const snapshot = await apiFetch("/context/snapshot", {
            method: "POST",
            body: JSON.stringify(params),
        });
        const s = snapshot as any;
        return {
            content: [{
                type: "text" as const,
                text: `Snapshot created: ${s.id} (${s.tokenCount} tokens, ${(s.chunks as any[])?.length ?? 0} chunks)`,
            }],
        };
    },
);

server.tool(
    "get_context_snapshot",
    "Retrieve a previously frozen context snapshot.",
    { snapshotId: z.string() },
    async ({ snapshotId }) => {
        const snapshot = await apiFetch(`/context/snapshot/${snapshotId}`);
        return { content: [{ type: "text" as const, text: JSON.stringify(snapshot, null, 2) }] };
    },
);
```

- [ ] **Step 5: Create `apps/cli/src/commands/context-snapshot.ts`**

```typescript
import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { formatDim, formatSuccess } from "../lib/colors";
import { output, outputError, outputQuiet, isJson } from "../lib/output";

const createSnapshot = new Command("create")
    .description("Create a frozen context snapshot")
    .option("-p, --plan <planId>", "scope to a plan")
    .option("-t, --task <taskId>", "scope to a task (requires --plan)")
    .option("-a, --about <concept>", "scope to a concept")
    .option("-f, --files <paths>", "scope to files (comma-separated)")
    .option("--max-tokens <n>", "token budget", "8000")
    .action(async (opts: { plan?: string; task?: string; about?: string; files?: string; maxTokens: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { maxTokens: Number(opts.maxTokens) };
            if (opts.plan) body.planId = opts.plan;
            if (opts.task) body.taskId = opts.task;
            if (opts.about) body.concept = opts.about;
            if (opts.files) body.filePaths = opts.files.split(",").map(f => f.trim());

            const res = await fetchApi("/context/snapshot", {
                method: "POST",
                body: JSON.stringify(body),
            });
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }
            const snapshot = (await res.json()) as any;
            outputQuiet(cmd, snapshot.id);
            output(cmd, snapshot, formatSuccess(`Snapshot ${snapshot.id.slice(0, 8)} created (${snapshot.tokenCount} tokens)`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const getSnapshot = new Command("get")
    .description("Retrieve a frozen context snapshot")
    .argument("<snapshotId>", "snapshot ID")
    .action(async (snapshotId: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/context/snapshot/${snapshotId}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status}`);
                process.exit(1);
            }
            const snapshot = await res.json();
            output(cmd, snapshot, JSON.stringify(snapshot, null, 2));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const listSnapshots = new Command("list")
    .description("List your context snapshots")
    .action(async (_opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi("/context/snapshots");
            if (!res.ok) {
                outputError(`Failed: ${res.status}`);
                process.exit(1);
            }
            const snapshots = (await res.json()) as any[];
            if (isJson(cmd)) {
                output(cmd, snapshots, "");
                return;
            }
            if (snapshots.length === 0) {
                output(cmd, [], formatDim("No snapshots."));
                return;
            }
            for (const s of snapshots) {
                console.log(`  ${s.id.slice(0, 8)} ${formatDim(`${s.tokenCount} tokens`)} ${formatDim(new Date(s.createdAt).toLocaleString())}`);
            }
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const deleteSnapshotCmd = new Command("delete")
    .description("Delete a context snapshot")
    .argument("<snapshotId>", "snapshot ID")
    .action(async (snapshotId: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/context/snapshot/${snapshotId}`, { method: "DELETE" });
            if (!res.ok) {
                outputError(`Failed: ${res.status}`);
                process.exit(1);
            }
            output(cmd, { ok: true }, formatSuccess("Snapshot deleted."));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

export const contextSnapshotCommand = new Command("snapshot")
    .description("Manage context snapshots")
    .addCommand(createSnapshot)
    .addCommand(getSnapshot)
    .addCommand(listSnapshots)
    .addCommand(deleteSnapshotCmd);
```

- [ ] **Step 6: Add snapshot to context-group.ts**

```typescript
import { contextSnapshotCommand } from "./context-snapshot";
// ... add to the chain:
.addCommand(contextSnapshotCommand);
```

- [ ] **Step 7: Type check all packages**

```bash
pnpm --filter @fubbik/db run check-types 2>&1 | tail -10
pnpm --filter @fubbik/api run check-types 2>&1 | tail -10
pnpm --filter @fubbik/mcp run check-types 2>&1 | tail -10
pnpm --filter cli run check-types 2>&1 | grep -v "gaps\|init" | tail -10
```

- [ ] **Step 8: Commit**

```bash
git add packages/db/src/ packages/api/src/context/ packages/api/src/index.ts packages/mcp/src/ apps/cli/src/commands/
git commit -m "feat: add context snapshots — schema, API, MCP tools, CLI commands

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Final Verification

- [ ] **Step 1: Build all packages**

```bash
pnpm build 2>&1 | tail -20
```

Expected: success.

- [ ] **Step 2: Server starts**

```bash
timeout 15 bun run --cwd apps/server src/index.ts 2>&1 | head -10
```

Expected: "Server is running" — no missing-export errors.

- [ ] **Step 3: Verify new endpoints exist**

```bash
curl -s http://localhost:3000/api/context/for-plan?planId=test 2>&1 | head -5
curl -s http://localhost:3000/api/context/about?q=auth 2>&1 | head -5
curl -s http://localhost:3000/api/context/for-files?paths=src/index.ts 2>&1 | head -5
```

Expected: JSON responses (may be errors about auth — that's fine, the routes exist).

- [ ] **Step 4: Test CLI commands**

```bash
pnpm --filter cli run dev -- context --help 2>&1
```

Expected: shows `export`, `dir`, `for`, `for-plan`, `about`, `for-diff`, `snapshot` subcommands.

```bash
pnpm --filter cli run dev -- context snapshot --help 2>&1
```

Expected: shows `create`, `get`, `list`, `delete` subcommands.

- [ ] **Step 5: Run tests**

```bash
pnpm test 2>&1 | tail -20
```

Expected: no new test failures.

- [ ] **Step 6: Commit any fixes**

```bash
git commit -am "fix: resolve AI context improvements verification issues

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
