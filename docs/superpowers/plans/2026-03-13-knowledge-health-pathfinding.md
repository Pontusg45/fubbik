# Knowledge Health + Path Finding Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a knowledge health page (orphan/stale/thin chunk detection) and improve path finding UX in the graph view.

**Architecture:** New knowledge-health repository with three SQL queries (orphans, stale, thin). New service + routes following Repository → Service → Route pattern. Frontend: new `/knowledge-health` page, path finding panel in graph view, "Find path to..." action on chunk detail page.

**Tech Stack:** Drizzle ORM, Effect, Elysia, React Flow, TanStack Router/Query, Vitest

**Spec:** `docs/superpowers/specs/2026-03-13-knowledge-health-pathfinding-design.md`

---

## File Structure

### New files
- `packages/db/src/repository/knowledge-health.ts` — orphan, stale, thin chunk queries
- `packages/api/src/knowledge-health/service.ts` — composes the three queries
- `packages/api/src/knowledge-health/routes.ts` — `GET /health/knowledge`
- `apps/web/src/routes/knowledge-health.tsx` — health page
- `apps/web/src/features/graph/path-panel.tsx` — path finding panel component

### Modified files
- `packages/db/src/repository/index.ts` — export knowledge-health
- `packages/api/src/index.ts` — register knowledge-health routes
- `apps/web/src/routes/__root.tsx` — add Health nav link
- `apps/web/src/features/nav/mobile-nav.tsx` — add Health nav link
- `apps/web/src/routes/graph.tsx` — add `pathFrom`/`pathTo` search params
- `apps/web/src/features/graph/graph-view.tsx` — integrate path panel, accept path params
- `apps/web/src/routes/chunks.$chunkId.tsx` — add "Find path to..." action
- `apps/web/src/routeTree.gen.ts` — register new route (auto-generated)

---

## Chunk 1: Knowledge Health Backend

### Task 1: Knowledge health repository

**Files:**
- Create: `packages/db/src/repository/knowledge-health.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write the repository**

```typescript
// packages/db/src/repository/knowledge-health.ts
import { and, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";

const RESULT_LIMIT = 50;

function codebaseConditions(userId: string, codebaseId?: string) {
    const conditions = [eq(chunk.userId, userId)];
    if (codebaseId) {
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
    return conditions;
}

export function getOrphanChunks(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                ...codebaseConditions(userId, codebaseId),
                sql`${chunk.id} NOT IN (SELECT ${chunkConnection.sourceId} FROM ${chunkConnection})`,
                sql`${chunk.id} NOT IN (SELECT ${chunkConnection.targetId} FROM ${chunkConnection})`
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    createdAt: chunk.createdAt
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(chunk.createdAt)
                .limit(RESULT_LIMIT);

            const [countResult] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, count: Number(countResult?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getStaleChunks(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const thirtyDaysAgo = new Date(Date.now() - 30 * 86400000);
            const sevenDaysAgo = new Date(Date.now() - 7 * 86400000);

            // Chunks not updated in 30 days that have a neighbor updated in last 7 days
            const staleConditions = [
                ...codebaseConditions(userId, codebaseId),
                sql`${chunk.updatedAt} < ${thirtyDaysAgo}`,
                sql`EXISTS (
                    SELECT 1 FROM ${chunkConnection} cc
                    JOIN ${chunk} neighbor ON (
                        (cc.${chunkConnection.sourceId} = ${chunk.id} AND cc.${chunkConnection.targetId} = neighbor.id)
                        OR (cc.${chunkConnection.targetId} = ${chunk.id} AND cc.${chunkConnection.sourceId} = neighbor.id)
                    )
                    WHERE neighbor.${chunk.updatedAt} > ${sevenDaysAgo}
                )`
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    updatedAt: chunk.updatedAt,
                    newestNeighborUpdate: sql<string>`(
                        SELECT MAX(neighbor.updated_at) FROM chunk_connection cc
                        JOIN chunk neighbor ON (
                            (cc.source_id = ${chunk.id} AND cc.target_id = neighbor.id)
                            OR (cc.target_id = ${chunk.id} AND cc.source_id = neighbor.id)
                        )
                    )`
                })
                .from(chunk)
                .where(and(...staleConditions))
                .orderBy(chunk.updatedAt)
                .limit(RESULT_LIMIT);

            const [countResult] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...staleConditions));

            return { chunks, count: Number(countResult?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getThinChunks(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                ...codebaseConditions(userId, codebaseId),
                sql`LENGTH(${chunk.content}) < 100`
            ];

            const chunks = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    contentLength: sql<number>`LENGTH(${chunk.content})`
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(sql`LENGTH(${chunk.content})`)
                .limit(RESULT_LIMIT);

            const [countResult] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, count: Number(countResult?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

Note: The stale query uses raw SQL column names in the EXISTS subquery because Drizzle's SQL template interpolation doesn't work cleanly inside correlated subqueries with self-joins. The implementer should verify the generated SQL is correct and adjust if Drizzle produces invalid output — fall back to fully raw `sql` if needed.

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./knowledge-health";
```

- [ ] **Step 3: Run tests to verify nothing breaks**

Run: `cd packages/db && pnpm vitest run`
Expected: All existing tests PASS

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/knowledge-health.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add knowledge health repository (orphan, stale, thin queries)"
```

---

### Task 2: Knowledge health service and routes

**Files:**
- Create: `packages/api/src/knowledge-health/service.ts`
- Create: `packages/api/src/knowledge-health/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Write the service**

```typescript
// packages/api/src/knowledge-health/service.ts
import { getOrphanChunks, getStaleChunks, getThinChunks } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getKnowledgeHealth(userId: string, codebaseId?: string) {
    return Effect.all(
        {
            orphans: getOrphanChunks(userId, codebaseId),
            stale: getStaleChunks(userId, codebaseId),
            thin: getThinChunks(userId, codebaseId)
        },
        { concurrency: "unbounded" }
    );
}
```

- [ ] **Step 2: Write the route**

```typescript
// packages/api/src/knowledge-health/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as knowledgeHealthService from "./service";

export const knowledgeHealthRoutes = new Elysia().get(
    "/health/knowledge",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    knowledgeHealthService.getKnowledgeHealth(session.user.id, ctx.query.codebaseId)
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String())
        })
    }
);
```

- [ ] **Step 3: Register in API index**

In `packages/api/src/index.ts`, add import and `.use(knowledgeHealthRoutes)` after `.use(codebaseRoutes)`:

```typescript
import { knowledgeHealthRoutes } from "./knowledge-health/routes";
// ...
    .use(codebaseRoutes)
    .use(knowledgeHealthRoutes);
```

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run src/codebases/normalize-url.test.ts`
Expected: PASS (no regressions in compilation)

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/knowledge-health/service.ts packages/api/src/knowledge-health/routes.ts packages/api/src/index.ts
git commit -m "feat(api): add knowledge health endpoint with orphan, stale, thin detection"
```

---

## Chunk 2: Knowledge Health Page

### Task 3: Knowledge health frontend page

**Files:**
- Create: `apps/web/src/routes/knowledge-health.tsx`
- Modify: `apps/web/src/routes/__root.tsx`
- Modify: `apps/web/src/features/nav/mobile-nav.tsx`
- Modify: `apps/web/src/routeTree.gen.ts`

- [ ] **Step 1: Read existing pages for patterns**

Read `apps/web/src/routes/tags.tsx` and `apps/web/src/routes/codebases.tsx` for the page pattern (query, layout, cards).

- [ ] **Step 2: Create the knowledge health page**

Create `apps/web/src/routes/knowledge-health.tsx`:

The page should:
1. Use `createFileRoute("/knowledge-health")` with `beforeLoad` for optional auth (same pattern as dashboard)
2. Query `api.api.health.knowledge.get({ query: { codebaseId } })` using `useQuery` with key `["knowledge-health", codebaseId]`
3. Get `codebaseId` from `useActiveCodebase()` hook
4. Render three card sections:

**Orphan Chunks card:**
- Count badge in header
- Description text
- List of chunks: title (Link to `/chunks/$chunkId`), type Badge, created date
- Quick actions: "View" link, "Delete" button (with mutation)

**Stale Chunks card:**
- Count badge in header
- Description text
- List: title (linked), type badge, "Updated X days ago", "Neighbor updated Y days ago"
- Quick action: "Edit" link to `/chunks/$chunkId/edit`

**Thin Chunks card:**
- Count badge in header
- Description text
- List: title (linked), type badge, "N characters" content length
- Quick action: "Edit" link

Use shadcn Card component, Badge, Button, Link from tanstack router. Follow existing page styling (`container mx-auto max-w-5xl px-4 py-8`).

- [ ] **Step 3: Add nav links**

In `apps/web/src/routes/__root.tsx`, add a "Health" Link between "Tags" and "Codebases":
```tsx
<Link
    to="/knowledge-health"
    className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
>
    Health
</Link>
```

Add the same link to `apps/web/src/features/nav/mobile-nav.tsx`.

- [ ] **Step 4: Verify**

Run: `pnpm run check-types`
Expected: No new type errors

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/knowledge-health.tsx apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add knowledge health page with orphan, stale, thin sections"
```

---

## Chunk 3: Path Finding UX

### Task 4: Path finding panel component

**Files:**
- Create: `apps/web/src/features/graph/path-panel.tsx`

- [ ] **Step 1: Create the path panel component**

The panel receives the graph's chunk list, current path state, and callbacks. It's a self-contained component that renders in the graph toolbar area.

```tsx
// apps/web/src/features/graph/path-panel.tsx
// Props:
// - chunks: Array<{ id: string; title: string }> — all chunks in the graph
// - pathStartId: string | null
// - pathEndId: string | null
// - pathResult: { pathNodeIds: Set<string>; pathEdgeIds: Set<string>; length: number } | null
// - edges: Array<{ id: string; source: string; target: string; data?: { relation?: string } }>
// - onSetStart: (id: string | null) => void
// - onSetEnd: (id: string | null) => void
// - onClear: () => void

// UI:
// 1. Two searchable select dropdowns (Combobox pattern with shadcn Popover + Command)
//    - Populated from chunks prop
//    - Filtered by typing
// 2. When both selected and path found:
//    - Relation chain strip showing path with direction
//    - For each consecutive pair (A, B) in pathResult, find matching edge:
//      - If edge.source === A && edge.target === B: "A →relation→ B"
//      - If edge.source === B && edge.target === A: "A ←relation← B"
//      - If multiple edges between same pair, join relations with "/"
//    - "N hops" indicator
// 3. When both selected and no path: "No path found" message
// 4. "Clear" button to reset

// Use existing shadcn components: Button, Popover, Command (if available),
// or a simple filtered input + dropdown list
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/graph/path-panel.tsx
git commit -m "feat(web): add path finding panel component with relation chain display"
```

---

### Task 5: Integrate path panel into graph view

**Files:**
- Modify: `apps/web/src/routes/graph.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Add search params to graph route**

Read `apps/web/src/routes/graph.tsx`. Add `validateSearch` to the route:

```tsx
export const Route = createFileRoute("/graph")({
    validateSearch: (search: Record<string, unknown>) => ({
        pathFrom: (search.pathFrom as string) ?? undefined,
        pathTo: (search.pathTo as string) ?? undefined
    }),
    // ... rest stays the same
});
```

- [ ] **Step 2: Pass path params to GraphView**

The graph view needs to receive the initial `pathFrom`/`pathTo` values. Since GraphView is lazy-loaded, pass them as props or read them from the route inside the component.

In `graph-view.tsx`:
- Read `pathFrom`/`pathTo` from `useSearch({ from: "/graph" })` (or accept as props)
- On mount, if `pathFrom` and `pathTo` are present, set `pathStartId` and `pathEndId` from them
- Use a `useEffect` with the search params as dependencies

```typescript
const search = useSearch({ from: "/graph", strict: false }) as { pathFrom?: string; pathTo?: string };

useEffect(() => {
    if (search.pathFrom) setPathStartId(search.pathFrom);
    if (search.pathTo) setPathEndId(search.pathTo);
}, [search.pathFrom, search.pathTo]);
```

- [ ] **Step 3: Add PathPanel to graph toolbar**

In `graph-view.tsx`, find the toolbar area (where layout/filter controls are). Add a "Find Path" button that toggles a `showPathPanel` state. When open, render `<PathPanel>` with the existing `pathStartId`, `pathEndId`, `pathResult`, chunks, edges, and setter callbacks.

Replace or augment the existing path result display (around line 1579) with the PathPanel component.

Wire the clear button to reset both `pathStartId` and `pathEndId` to null.

- [ ] **Step 4: Handle missing chunks error for deep links**

When `pathFrom`/`pathTo` are set but the chunks aren't found in the loaded graph data, show an error message in the path panel: "One or both chunks are not in the current graph view. Try switching to 'All' codebases."

Check by verifying the IDs exist in the `chunkMap` before triggering path finding.

- [ ] **Step 5: Verify**

Run: `pnpm run check-types`
Expected: No new type errors

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/routes/graph.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "feat(web): integrate path finding panel into graph view with deep link support"
```

---

### Task 6: Add "Find path to..." on chunk detail page

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Read the current chunk detail page**

Read `apps/web/src/routes/chunks.$chunkId.tsx` to understand the actions area layout.

- [ ] **Step 2: Add "Find path to..." button**

In the actions area (around line 121-134, where Edit and Delete buttons are), add a new button with a chunk selector.

Implementation options (pick simplest that works):

**Option A — Simple:** "Find path to..." button that navigates to `/graph?pathFrom=<chunkId>`. The user then selects the target in the graph's path panel. This is the simplest and avoids needing a chunk search dropdown on this page.

**Option B — Full:** Button opens a Popover with a searchable chunk list. On selection, navigates to `/graph?pathFrom=<chunkId>&pathTo=<selectedId>`.

Start with Option A. The implementer can upgrade to Option B if time permits.

```tsx
import { Network } from "lucide-react"; // already imported

<Button
    variant="outline"
    size="sm"
    render={<Link to="/graph" search={{ pathFrom: chunkId }} />}
>
    <Network className="size-3.5" />
    Find path
</Button>
```

- [ ] **Step 3: Verify**

Run: `pnpm run check-types`
Expected: No new type errors

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.$chunkId.tsx
git commit -m "feat(web): add 'Find path' action to chunk detail page"
```

---

### Task 7: Final verification

- [ ] **Step 1: Run full CI**

Run: `pnpm ci`
Expected: Type-check, lint, test, build pass (same baseline failures as before)

- [ ] **Step 2: Fix any issues**

- [ ] **Step 3: Commit if needed**

```bash
git add -A && git commit -m "fix: resolve CI issues from knowledge health + path finding"
```
