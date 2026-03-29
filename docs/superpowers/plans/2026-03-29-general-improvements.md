# General Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve code quality, performance, testing, UX, CI, and security across the fubbik codebase.

**Architecture:** Seven independent phases, each producing a shippable improvement. Phases can be executed in any order, though Phase 1 (component splitting) reduces file sizes that make later phases easier.

**Tech Stack:** vitest, drizzle, Effect, Elysia, React/TanStack, Turbo, GitHub Actions.

---

## Phase 1: Component Splitting

Split the two largest files (`graph-view.tsx` at 1,916 lines and `chunks.index.tsx` at 1,548 lines) into focused modules.

---

### Task 1: Extract graph utility functions

**Files:**
- Create: `apps/web/src/features/graph/graph-utils.ts`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create graph-utils.ts with path-finding and utility functions**

Extract `findShortestPath` (lines 77-98) and `getMostConnected` (lines 100-115) from `graph-view.tsx`:

```typescript
// apps/web/src/features/graph/graph-utils.ts
import type { Node, Edge } from "@xyflow/react";

export function findShortestPath(
    nodes: Node[],
    edges: Edge[],
    startId: string,
    endId: string
): string[] | null {
    const adj = new Map<string, string[]>();
    for (const n of nodes) adj.set(n.id, []);
    for (const e of edges) {
        adj.get(e.source)?.push(e.target);
        adj.get(e.target)?.push(e.source);
    }
    const queue: string[][] = [[startId]];
    const visited = new Set<string>([startId]);
    while (queue.length > 0) {
        const path = queue.shift()!;
        const current = path[path.length - 1]!;
        if (current === endId) return path;
        for (const neighbor of adj.get(current) ?? []) {
            if (!visited.has(neighbor)) {
                visited.add(neighbor);
                queue.push([...path, neighbor]);
            }
        }
    }
    return null;
}

export function getMostConnected(nodes: Node[], edges: Edge[]): string | null {
    const degree = new Map<string, number>();
    for (const n of nodes) degree.set(n.id, 0);
    for (const e of edges) {
        degree.set(e.source, (degree.get(e.source) ?? 0) + 1);
        degree.set(e.target, (degree.get(e.target) ?? 0) + 1);
    }
    let maxId: string | null = null;
    let maxDeg = 0;
    for (const [id, deg] of degree) {
        if (deg > maxDeg) {
            maxDeg = deg;
            maxId = id;
        }
    }
    return maxId;
}
```

- [ ] **Step 2: Update graph-view.tsx imports**

Replace the inline functions (lines 77-115) with:

```typescript
import { findShortestPath, getMostConnected } from "./graph-utils";
```

Delete lines 77-115 from `graph-view.tsx`.

- [ ] **Step 3: Verify the app builds**

Run: `pnpm --filter web check-types`
Expected: No new type errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/graph-utils.ts apps/web/src/features/graph/graph-view.tsx
git commit -m "refactor: extract graph utility functions to graph-utils.ts"
```

---

### Task 2: Extract graph keyboard shortcuts hook

**Files:**
- Create: `apps/web/src/features/graph/use-graph-keyboard.ts`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create the hook**

Extract the keyboard handler (lines 1092-1137) into a custom hook. Read the exact code at those lines and adapt it into:

```typescript
// apps/web/src/features/graph/use-graph-keyboard.ts
import { useEffect } from "react";

interface UseGraphKeyboardOptions {
    selectedChunkId: string | null;
    // ... all the state/callbacks the keyboard handler needs
    // Read the actual lines 1092-1137 to determine exact deps
}

export function useGraphKeyboard(options: UseGraphKeyboardOptions) {
    useEffect(() => {
        // Move the keyboard event listener from graph-view.tsx lines 1092-1137 here
    }, [/* deps */]);
}
```

- [ ] **Step 2: Replace inline keyboard handler with hook call in graph-view.tsx**

- [ ] **Step 3: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/use-graph-keyboard.ts apps/web/src/features/graph/graph-view.tsx
git commit -m "refactor: extract graph keyboard shortcuts to custom hook"
```

---

### Task 3: Extract graph settings panel

**Files:**
- Create: `apps/web/src/features/graph/graph-settings-panel.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create the settings panel component**

Extract lines 1481-1628 from `graph-view.tsx` (the settings menu with layout, tools, views, custom graphs sections) into a dedicated component. Read those lines to determine the exact props interface needed.

- [ ] **Step 2: Replace inline JSX with component in graph-view.tsx**

- [ ] **Step 3: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/graph-settings-panel.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "refactor: extract graph settings panel to separate component"
```

---

### Task 4: Extract graph dialog components

**Files:**
- Create: `apps/web/src/features/graph/graph-dialogs.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create graph-dialogs.tsx**

Extract these dialog sections from `graph-view.tsx`:
- Change connection type dialog (lines 1664-1703)
- Save view dialog (lines 1762-1815)
- Save custom graph dialog (lines 1817-1888)

Each becomes a named export component with the state it needs passed as props.

- [ ] **Step 2: Replace inline dialogs with components in graph-view.tsx**

- [ ] **Step 3: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/graph-dialogs.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "refactor: extract graph dialog components"
```

---

### Task 5: Extract chunk filters popover

**Files:**
- Create: `apps/web/src/features/chunks/chunk-filters-popover.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Create chunk-filters-popover.tsx**

Extract the filters popover content (lines 571-895 of `chunks.index.tsx`) — the Popover containing 10+ filter sections (type, sort, tags, date, size, enrichment, connections, origin, review status, saved filters). Read those lines and create a component that accepts the filter state and callbacks as props.

- [ ] **Step 2: Replace inline popover with component in chunks.index.tsx**

- [ ] **Step 3: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/chunk-filters-popover.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "refactor: extract chunk filters popover to separate component"
```

---

### Task 6: Extract chunk bulk action bar

**Files:**
- Create: `apps/web/src/features/chunks/chunk-bulk-action-bar.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Create chunk-bulk-action-bar.tsx**

Extract the floating bulk action bar (lines 1362-1517 of `chunks.index.tsx`) — the bottom bar with select count, delete/archive/tag/connect actions. Read those lines and create a component.

- [ ] **Step 2: Replace inline bar with component in chunks.index.tsx**

- [ ] **Step 3: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/chunk-bulk-action-bar.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "refactor: extract chunk bulk action bar to separate component"
```

---

### Task 7: Extract chunk list hooks

**Files:**
- Create: `apps/web/src/features/chunks/use-chunk-filters.ts`
- Create: `apps/web/src/features/chunks/use-bulk-chunk-operations.ts`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Create use-chunk-filters.ts**

Extract filter/search state management from `chunks.index.tsx`:
- `updateSearch()` function (lines 314-350)
- `clearAllFilters()` function (lines 352-372)
- `toggleTag()` function (lines 192-196)
- Active filter count computation

Return a hook that wraps URL search params and provides filter helpers.

- [ ] **Step 2: Create use-bulk-chunk-operations.ts**

Extract bulk operation mutations and handlers from `chunks.index.tsx`:
- `bulkUpdateMutation` (lines 393-420)
- `singleDeleteMutation` (lines 422-432)
- `reviewMutation` (lines ~455-475)
- `bulkConnectMutation` (lines ~445-453)
- Selection state (`selectedIds`, `toggleSelection`, `toggleAll`)

- [ ] **Step 3: Update chunks.index.tsx to use the new hooks**

- [ ] **Step 4: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/chunks/use-chunk-filters.ts apps/web/src/features/chunks/use-bulk-chunk-operations.ts apps/web/src/routes/chunks.index.tsx
git commit -m "refactor: extract chunk filter and bulk operation hooks"
```

---

## Phase 2: Database Performance

Add missing indexes and batch-loading functions to eliminate N+1 queries.

---

### Task 8: Add missing database indexes

**Files:**
- Modify: `packages/db/src/schema/chunk.ts`
- Modify: `packages/db/src/schema/tag.ts`

- [ ] **Step 1: Add indexes to chunk table**

In `packages/db/src/schema/chunk.ts`, add to the existing indexes (after line 48):

```typescript
chunkArchivedAtIdx: index("chunk_archivedAt_idx").on(chunk.archivedAt),
chunkUpdatedAtIdx: index("chunk_updatedAt_idx").on(chunk.updatedAt),
```

- [ ] **Step 2: Add tag_id index to chunkTag table**

In `packages/db/src/schema/tag.ts`, add an index on `tag_id` to the `chunkTag` table definition:

```typescript
// Add after the primaryKey definition
chunkTagTagIdIdx: index("chunk_tag_tagId_idx").on(chunkTag.tagId),
```

- [ ] **Step 3: Push schema changes**

Run: `pnpm db:push`
Expected: Schema updates applied successfully.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/chunk.ts packages/db/src/schema/tag.ts
git commit -m "perf: add missing database indexes for chunk and tag queries"
```

---

### Task 9: Add batch-loading repository functions

**Files:**
- Modify: `packages/db/src/repository/file-ref.ts` — add `getFileRefsForChunks()`
- Modify: `packages/db/src/repository/applies-to.ts` — add `getAppliesToForChunks()`
- Modify: `packages/db/src/repository/connection.ts` — add `getConnectionsForChunks()`

- [ ] **Step 1: Write test for getFileRefsForChunks**

Create or extend the test file for file-ref repository. Test that given multiple chunk IDs, it returns file refs grouped by chunk ID.

- [ ] **Step 2: Implement getFileRefsForChunks**

In `packages/db/src/repository/file-ref.ts`, add (following the pattern of existing `getFileRefsForChunk`):

```typescript
export function getFileRefsForChunks(chunkIds: string[]) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkFileRef)
                .where(inArray(chunkFileRef.chunkId, chunkIds)),
        catch: err => new DatabaseError({ cause: err })
    });
}
```

- [ ] **Step 3: Implement getAppliesToForChunks**

In `packages/db/src/repository/applies-to.ts`, add:

```typescript
export function getAppliesToForChunks(chunkIds: string[]) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkAppliesTo)
                .where(inArray(chunkAppliesTo.chunkId, chunkIds)),
        catch: err => new DatabaseError({ cause: err })
    });
}
```

- [ ] **Step 4: Implement getConnectionsForChunks**

In `packages/db/src/repository/connection.ts`, add a function that loads all connections where either source or target is in the given chunk IDs:

```typescript
export function getConnectionsForChunks(chunkIds: string[]) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkConnection)
                .where(
                    or(
                        inArray(chunkConnection.sourceId, chunkIds),
                        inArray(chunkConnection.targetId, chunkIds)
                    )
                ),
        catch: err => new DatabaseError({ cause: err })
    });
}
```

- [ ] **Step 5: Export new functions from repository index**

Add the new exports to `packages/db/src/repository/index.ts`.

- [ ] **Step 6: Run tests**

Run: `pnpm --filter @fubbik/db test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/repository/file-ref.ts packages/db/src/repository/applies-to.ts packages/db/src/repository/connection.ts packages/db/src/repository/index.ts
git commit -m "perf: add batch-loading functions for file refs, applies-to, and connections"
```

---

## Phase 3: Test Coverage for Critical Paths

Add tests for the most important untested service functions.

---

### Task 10: Test plan generation from requirements

**Files:**
- Create: `packages/api/src/plans/generate-from-requirements.test.ts`

- [ ] **Step 1: Read the source**

Read `packages/api/src/plans/generate-from-requirements.ts` to understand the function signature, inputs, and outputs.

- [ ] **Step 2: Write tests**

Test cases:
- Generates a plan with steps from a list of requirement IDs
- Each requirement produces at least one plan step
- Plan title is derived from requirements
- Empty requirement list returns validation error
- Invalid requirement IDs are handled gracefully

Use vitest mocks for the repository layer (don't hit the database).

- [ ] **Step 3: Run tests**

Run: `pnpm --filter @fubbik/api exec vitest run src/plans/generate-from-requirements.test.ts`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/plans/generate-from-requirements.test.ts
git commit -m "test: add tests for plan generation from requirements"
```

---

### Task 11: Test bulk operations service

**Files:**
- Create: `packages/api/src/chunks/bulk-service.test.ts`

- [ ] **Step 1: Read the source**

Read `packages/api/src/chunks/bulk-service.ts` to understand bulk update operations (add_tags, remove_tags, set_type, set_codebase, set_review_status).

- [ ] **Step 2: Write tests**

Test each bulk action type with mocked repository calls:
- `add_tags` creates tags and links them to all specified chunks
- `remove_tags` removes tag associations
- `set_type` updates the type field on all chunks
- `set_codebase` updates codebase associations
- `set_review_status` updates review status
- Empty IDs array returns early
- IDs exceeding max (100) are rejected by route validation

- [ ] **Step 3: Run tests**

Run: `pnpm --filter @fubbik/api exec vitest run src/chunks/bulk-service.test.ts`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/chunks/bulk-service.test.ts
git commit -m "test: add tests for chunk bulk operations"
```

---

### Task 12: Test context export service

**Files:**
- Create: `packages/api/src/context-export/service.test.ts`

- [ ] **Step 1: Read the source**

Read `packages/api/src/context-export/service.ts` and `packages/api/src/context-export/claude-md.ts`.

- [ ] **Step 2: Write tests**

Test:
- Token-budgeted context export respects token limits
- `forPath` relevance boosting prioritizes matching chunks
- Claude MD generation produces valid markdown
- Empty chunk list produces minimal output

- [ ] **Step 3: Run and commit**

```bash
git add packages/api/src/context-export/service.test.ts
git commit -m "test: add tests for context export service"
```

---

## Phase 4: UX Polish

Add error boundaries, loading states, and empty states to routes that are missing them.

---

### Task 13: Add nested error boundaries

**Files:**
- Create: `apps/web/src/components/route-error-boundary.tsx`
- Modify: `apps/web/src/routes/graph.tsx`
- Modify: `apps/web/src/routes/settings.tsx`

- [ ] **Step 1: Create a reusable route error boundary**

```typescript
// apps/web/src/components/route-error-boundary.tsx
import { Component, type ReactNode } from "react";
import { PageContainer } from "@/components/ui/page";
import { Button } from "@/components/ui/button";
import { AlertTriangle } from "lucide-react";

interface Props {
    children: ReactNode;
    fallbackTitle?: string;
}

interface State {
    hasError: boolean;
    error: Error | null;
}

export class RouteErrorBoundary extends Component<Props, State> {
    state: State = { hasError: false, error: null };

    static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error };
    }

    render() {
        if (this.state.hasError) {
            return (
                <PageContainer>
                    <div className="flex flex-col items-center justify-center py-16">
                        <AlertTriangle className="text-destructive mb-3 size-10" />
                        <h2 className="mb-2 text-lg font-semibold">
                            {this.props.fallbackTitle ?? "Something went wrong"}
                        </h2>
                        <p className="text-muted-foreground mb-4 text-sm">
                            {this.state.error?.message ?? "An unexpected error occurred"}
                        </p>
                        <Button
                            variant="outline"
                            onClick={() => this.setState({ hasError: false, error: null })}
                        >
                            Try again
                        </Button>
                    </div>
                </PageContainer>
            );
        }
        return this.props.children;
    }
}
```

- [ ] **Step 2: Wrap graph route with error boundary**

In `apps/web/src/routes/graph.tsx`, wrap the graph component with `<RouteErrorBoundary fallbackTitle="Graph failed to render">`.

- [ ] **Step 3: Wrap settings route with error boundary**

In `apps/web/src/routes/settings.tsx`, wrap with `<RouteErrorBoundary>`.

- [ ] **Step 4: Verify build**

Run: `pnpm --filter web check-types`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/route-error-boundary.tsx apps/web/src/routes/graph.tsx apps/web/src/routes/settings.tsx
git commit -m "feat: add nested error boundaries for graph and settings routes"
```

---

### Task 14: Add loading state to settings page

**Files:**
- Modify: `apps/web/src/routes/settings.tsx`

- [ ] **Step 1: Read the settings route**

Read `apps/web/src/routes/settings.tsx` to identify queries without loading states.

- [ ] **Step 2: Add loading skeleton**

Add `PageLoading` or `SkeletonList` while data is loading, following the pattern used in other routes (e.g., `codebases.tsx` line 118-119).

- [ ] **Step 3: Verify build and commit**

```bash
git add apps/web/src/routes/settings.tsx
git commit -m "feat: add loading state to settings page"
```

---

### Task 15: Add loading state to landing page stats

**Files:**
- Modify: `apps/web/src/routes/index.tsx`

- [ ] **Step 1: Read the landing page**

Read `apps/web/src/routes/index.tsx` to find the LiveStats query.

- [ ] **Step 2: Add loading skeleton for stats**

Replace raw numbers with skeleton placeholders while the query is loading.

- [ ] **Step 3: Verify build and commit**

```bash
git add apps/web/src/routes/index.tsx
git commit -m "feat: add loading skeleton for landing page stats"
```

---

## Phase 5: CI & Build Improvements

Optimize CI pipeline and Turbo configuration.

---

### Task 16: Enable Turbo test caching

**Files:**
- Modify: `turbo.json`

- [ ] **Step 1: Enable test caching with proper inputs**

In `turbo.json`, change the `test` task:

```json
"test": {
    "dependsOn": ["^build"],
    "inputs": ["src/**", "tests/**", "*.config.*", "package.json"],
    "outputs": [],
    "cache": true
}
```

This caches test results based on source file changes, so unchanged packages skip tests.

- [ ] **Step 2: Verify caching works**

Run: `pnpm test` twice. Second run should show cached results.

- [ ] **Step 3: Commit**

```bash
git add turbo.json
git commit -m "perf: enable Turbo test caching with source file inputs"
```

---

### Task 17: Parallelize CI jobs

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Split CI into parallel jobs**

```yaml
name: CI

on:
    push:
        branches: [main]
    pull_request:
        branches: [main]

jobs:
    lint-and-format:
        runs-on: ubuntu-latest
        steps:
            - uses: actions/checkout@v4
            - uses: oven-sh/setup-bun@v2
              with:
                  bun-version: "1.3.9"
            - run: bun install --frozen-lockfile
            - name: Sherif
              run: bun sherif
            - name: Lint
              run: bun lint
            - name: Format check
              run: bun fmt:check

    typecheck-and-test:
        runs-on: ubuntu-latest
        steps:
            - uses: actions/checkout@v4
            - uses: oven-sh/setup-bun@v2
              with:
                  bun-version: "1.3.9"
            - run: bun install --frozen-lockfile
            - name: Type check
              run: bun check-types
            - name: Test
              run: bun test

    build:
        needs: [lint-and-format, typecheck-and-test]
        runs-on: ubuntu-latest
        steps:
            - uses: actions/checkout@v4
            - uses: oven-sh/setup-bun@v2
              with:
                  bun-version: "1.3.9"
            - run: bun install --frozen-lockfile
            - name: Build
              run: bun run build
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "perf: parallelize CI jobs (lint/format || types/test, then build)"
```

---

### Task 18: Add path filtering for docs-only PRs

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add paths-ignore for documentation changes**

Add to the `on` section:

```yaml
on:
    push:
        branches: [main]
    pull_request:
        branches: [main]
        paths-ignore:
            - "docs/**"
            - "*.md"
            - ".claude/**"
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "perf: skip CI for docs-only PRs"
```

---

## Phase 6: Security Hardening

Add input validation for JSONB fields and rate limiting for expensive operations.

---

### Task 19: Validate JSONB fields before persistence

**Files:**
- Create: `packages/api/src/validation/jsonb-schemas.ts`
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Create JSONB validation schemas**

Using Elysia's `t` schema (consistent with the rest of the codebase):

```typescript
// packages/api/src/validation/jsonb-schemas.ts
import { t } from "elysia";

export const scopeSchema = t.Optional(
    t.Record(t.String({ maxLength: 100 }), t.String({ maxLength: 500 }), { maxProperties: 20 })
);

export const aliasesSchema = t.Optional(
    t.Array(t.String({ maxLength: 200 }), { maxItems: 20 })
);

export const alternativesSchema = t.Optional(
    t.Array(t.String({ maxLength: 2000 }), { maxItems: 10 })
);
```

- [ ] **Step 2: Apply schemas to chunk create/update routes**

In `packages/api/src/chunks/routes.ts`, import the schemas and add them to the `POST /chunks` and `PATCH /chunks/:id` body validation where `scope`, `aliases`, and `alternatives` are accepted.

- [ ] **Step 3: Run existing tests**

Run: `pnpm --filter @fubbik/api test`
Expected: All existing tests pass (validation is additive, not breaking).

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/validation/jsonb-schemas.ts packages/api/src/chunks/routes.ts
git commit -m "security: add schema validation for JSONB fields (scope, aliases, alternatives)"
```

---

### Task 20: Add rate limiting for expensive operations

**Files:**
- Create: `packages/api/src/middleware/rate-limit.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create rate limit middleware**

Simple in-memory rate limiter using a Map (appropriate for single-server deployment):

```typescript
// packages/api/src/middleware/rate-limit.ts
const windows = new Map<string, { count: number; resetAt: number }>();

export function checkRateLimit(
    key: string,
    maxRequests: number,
    windowMs: number
): { allowed: boolean; remaining: number; resetAt: number } {
    const now = Date.now();
    const entry = windows.get(key);

    if (!entry || now > entry.resetAt) {
        windows.set(key, { count: 1, resetAt: now + windowMs });
        return { allowed: true, remaining: maxRequests - 1, resetAt: now + windowMs };
    }

    entry.count++;
    const allowed = entry.count <= maxRequests;
    return { allowed, remaining: Math.max(0, maxRequests - entry.count), resetAt: entry.resetAt };
}

// Cleanup old entries every 5 minutes
setInterval(() => {
    const now = Date.now();
    for (const [key, entry] of windows) {
        if (now > entry.resetAt) windows.delete(key);
    }
}, 5 * 60 * 1000);
```

- [ ] **Step 2: Apply rate limiting to expensive routes**

In `packages/api/src/index.ts` or at the route level, add rate limiting to:
- `POST /api/chunks/:id/enrich` — 10 requests per minute per user
- `POST /api/chunks/import-docs` — 5 requests per minute per user
- `GET /api/chunks/search/semantic` — 30 requests per minute per user

Use the session user ID as the rate limit key.

- [ ] **Step 3: Verify API still works**

Run: `pnpm --filter @fubbik/api test`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/middleware/rate-limit.ts packages/api/src/index.ts
git commit -m "security: add per-user rate limiting for expensive operations"
```

---

## Phase 7: Cleanup & Documentation

Remove dead patterns and update CLAUDE.md.

---

### Task 21: Audit and document context-export vs context-for-file

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Read both directories**

Read `packages/api/src/context-export/` and `packages/api/src/context-for-file/` to understand their distinct purposes.

- [ ] **Step 2: Document the distinction in CLAUDE.md**

Add a note under the Architecture Patterns section explaining:
- `context-export/` — token-budgeted context export and CLAUDE.md generation
- `context-for-file/` — file-specific context lookup with dependency detection

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: clarify context-export vs context-for-file architecture"
```

---

### Task 22: Update CLAUDE.md with new features and patterns

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add import-docs to API endpoints section**

Add under Chunks endpoints:
```
- `POST /api/chunks/import-docs` — bulk import from markdown files with frontmatter parsing
```

- [ ] **Step 2: Add import-docs to CLI commands section**

Add:
```
- `fubbik import-docs <path> --codebase <name>` — import folder of markdown docs as chunks
```

- [ ] **Step 3: Add /import to Web Pages section**

Add:
```
- `/import` — dedicated markdown docs import with folder upload, preview table, codebase selection
```

- [ ] **Step 4: Add any new dependencies or patterns introduced**

Review recent changes and ensure CLAUDE.md reflects current state.

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with import-docs feature and latest patterns"
```
