# Requirements Feature Redesign — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the requirements feature with a card+sidebar list page, inline edit mode on the detail page, a coverage matrix page, and bulk operations.

**Architecture:** Extract shared components (StepBuilder, ChunkLinker, validation) from the create page. Add backend support for search, bulk operations, and coverage matrix data. Rebuild the list page with sidebar filters and cards. Add inline edit toggle to the detail page. Create a new coverage matrix route.

**Tech Stack:** TanStack Start, React Query, shadcn-ui (Select, Checkbox), Elysia, Drizzle ORM, Effect, Tailwind CSS

---

### Task 1: Extract shared validation logic

Extract the step validation function from the create page into a shared module so both create and detail pages can use it.

**Files:**
- Create: `apps/web/src/features/requirements/validation.ts`
- Modify: `apps/web/src/routes/requirements_.new.tsx` (remove inline `validateSteps`)

- [ ] **Step 1: Create the shared validation module**

```typescript
// apps/web/src/features/requirements/validation.ts
export type Keyword = "given" | "when" | "then" | "and" | "but";

export interface StepRow {
    keyword: Keyword;
    text: string;
}

export interface StepError {
    step: number;
    error: string;
}

export function validateSteps(steps: StepRow[]): StepError[] {
    const errors: StepError[] = [];
    if (steps.length === 0) {
        errors.push({ step: 0, error: "Must have at least one step" });
        return errors;
    }

    const firstKeyword = steps[0]!.keyword;
    if (firstKeyword === "and" || firstKeyword === "but") {
        errors.push({ step: 0, error: "First step cannot be 'and' or 'but'" });
    } else if (firstKeyword !== "given") {
        errors.push({ step: 0, error: "First step must be 'given'" });
    }

    let phase: "given" | "when" | "then" = "given";
    for (let i = 0; i < steps.length; i++) {
        const { keyword } = steps[i]!;
        if (keyword === "and" || keyword === "but") continue;
        if (keyword === "given") {
            if (phase === "when" || phase === "then") {
                errors.push({ step: i, error: "Cannot use 'given' after 'when' or 'then'" });
            }
        } else if (keyword === "when") {
            if (phase === "then") {
                errors.push({ step: i, error: "Cannot use 'when' after 'then'" });
            } else {
                phase = "when";
            }
        } else if (keyword === "then") {
            if (phase === "given") {
                errors.push({ step: i, error: "'then' must come after 'when' phase" });
            } else {
                phase = "then";
            }
        }
    }

    if (!steps.some(s => s.keyword === "when")) {
        errors.push({ step: -1, error: "Must contain at least one 'when' step" });
    }
    if (!steps.some(s => s.keyword === "then")) {
        errors.push({ step: -1, error: "Must contain at least one 'then' step" });
    }

    return errors;
}
```

- [ ] **Step 2: Update the create page to import from the shared module**

In `apps/web/src/routes/requirements_.new.tsx`:
- Remove the `Keyword`, `StepRow`, `StepError` type definitions (lines 29-40)
- Remove the `validateSteps` function (lines 93-138)
- Add import: `import { validateSteps, type Keyword, type StepRow, type StepError } from "@/features/requirements/validation";`
- Remove the `KEYWORDS` constant and move to validation module if needed, or keep in the create page since it's UI-specific

- [ ] **Step 3: Verify the create page still works**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/requirements/validation.ts apps/web/src/routes/requirements_.new.tsx
git commit -m "refactor: extract requirement validation into shared module"
```

---

### Task 2: Extract StepBuilder component

Extract the step builder UI (keyword selector, text input, vocab parsing, add word form) from the create page into a reusable component.

**Files:**
- Create: `apps/web/src/features/requirements/step-builder.tsx`
- Modify: `apps/web/src/routes/requirements_.new.tsx` (replace inline step builder with component)

- [ ] **Step 1: Create the StepBuilder component**

The component should accept these props:
```typescript
interface StepBuilderProps {
    steps: StepRow[];
    onStepsChange: (steps: StepRow[]) => void;
    codebaseId: string | undefined;
    stepErrors: StepError[];
}
```

Extract from `requirements_.new.tsx`:
- The `ParsedToken`, `VocabularyWarning`, `ParseResult` interfaces (lines 42-58)
- The `KEYWORDS` constant (line 60)
- The `VOCAB_CATEGORIES`, `EXPECTS_OPTIONS` constants (lines 61-62)
- The `tokenBadgeColor` function (lines 71-91)
- The vocabulary parsing state and `triggerParse` callback (lines 187-222)
- The `addWordMutation` (lines 233-255)
- The `addingWordAtStep` state and handlers (lines 188-190, 343-357)
- The `updateStep`, `removeStep`, `addStep` functions (lines 297-319)
- The step builder JSX (lines 540-707)

The component manages its own parse results and add-word state internally. Parent only sees `steps` and `onStepsChange`.

- [ ] **Step 2: Update the create page to use StepBuilder**

Replace the inline step builder section in `requirements_.new.tsx` with:
```tsx
<StepBuilder
    steps={steps}
    onStepsChange={setSteps}
    codebaseId={codebaseId}
    stepErrors={stepErrors}
/>
```

Remove all the extracted state, callbacks, and JSX from the create page.

- [ ] **Step 3: Verify types and functionality**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/requirements/step-builder.tsx apps/web/src/routes/requirements_.new.tsx
git commit -m "refactor: extract StepBuilder into shared component"
```

---

### Task 3: Extract ChunkLinker component

Extract the chunk search-and-select UI from the create page into a reusable component.

**Files:**
- Create: `apps/web/src/features/requirements/chunk-linker.tsx`
- Modify: `apps/web/src/routes/requirements_.new.tsx` (replace inline chunk linker with component)

- [ ] **Step 1: Create the ChunkLinker component**

```typescript
interface ChunkLinkerProps {
    selectedChunkIds: string[];
    onSelectedChunkIdsChange: (ids: string[]) => void;
    codebaseId: string | undefined;
}
```

Extract from `requirements_.new.tsx`:
- The `chunksQuery` (lines 274-288)
- The `chunkSearch` state (line 155)
- The filtering logic (lines 291-295)
- The selected badges + search input + dropdown JSX (lines 711-757)

The component manages its own search state and chunks query internally.

- [ ] **Step 2: Update the create page to use ChunkLinker**

Replace the chunk linking section with:
```tsx
<ChunkLinker
    selectedChunkIds={selectedChunkIds}
    onSelectedChunkIdsChange={setSelectedChunkIds}
    codebaseId={codebaseId}
/>
```

- [ ] **Step 3: Verify types**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/requirements/chunk-linker.tsx apps/web/src/routes/requirements_.new.tsx
git commit -m "refactor: extract ChunkLinker into shared component"
```

---

### Task 4: Add search and useCaseId to the list API

Add text search to the repository and wire `useCaseId` (already in repo) + `search` through the service and route layers.

**Files:**
- Modify: `packages/db/src/repository/requirement.ts` (add `search` to `ListRequirementsParams` and ILIKE condition)
- Modify: `packages/api/src/requirements/service.ts` (add `search` and `useCaseId` to query type)
- Modify: `packages/api/src/requirements/routes.ts` (add `search` and `useCaseId` to query schema)

- [ ] **Step 1: Add ILIKE search to the repository**

In `packages/db/src/repository/requirement.ts`:

Update the existing import to add `ilike` and `or`:
```typescript
import { and, eq, ilike, or, sql } from "drizzle-orm";
```

Add `search?: string` to `ListRequirementsParams` (after the existing `reviewStatus` field).

Add to the conditions array inside `listRequirements`, after the existing filter conditions:
```typescript
if (params.search) {
    const pattern = `%${params.search}%`;
    conditions.push(
        or(
            ilike(requirement.title, pattern),
            ilike(requirement.description, pattern)
        )!
    );
}
```

- [ ] **Step 2: Update the service query type and pass through new fields**

In `packages/api/src/requirements/service.ts`, update the `listRequirements` function's query type:
```typescript
export function listRequirements(
    userId: string,
    query: {
        codebaseId?: string;
        useCaseId?: string;
        search?: string;
        status?: string;
        priority?: string;
        origin?: string;
        reviewStatus?: string;
        limit?: string;
        offset?: string;
    }
) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);

    return listRequirementsRepo({
        userId,
        codebaseId: query.codebaseId,
        useCaseId: query.useCaseId,
        search: query.search,
        status: query.status,
        priority: query.priority,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        limit,
        offset
    });
}
```

- [ ] **Step 3: Add query params to the route schema**

In `packages/api/src/requirements/routes.ts`, update the list endpoint query schema (line 88-96):
```typescript
query: t.Object({
    codebaseId: t.Optional(t.String()),
    useCaseId: t.Optional(t.String()),
    search: t.Optional(t.String()),
    status: t.Optional(t.String()),
    priority: t.Optional(t.String()),
    origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
    reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")])),
    limit: t.Optional(t.String()),
    offset: t.Optional(t.String())
})
```

- [ ] **Step 4: Verify types**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/repository/requirement.ts packages/api/src/requirements/service.ts packages/api/src/requirements/routes.ts
git commit -m "feat: add search and useCaseId filtering to requirements list API"
```

---

### Task 5: Add bulk operations API

Add a bulk endpoint for status changes, use case assignment, and deletion.

**Files:**
- Modify: `packages/db/src/repository/requirement.ts` (add `bulkUpdateStatus`, `bulkSetUseCase`, `bulkDelete`)
- Modify: `packages/api/src/requirements/service.ts` (add `bulkAction`)
- Modify: `packages/api/src/requirements/routes.ts` (add `PATCH /requirements/bulk`)

- [ ] **Step 1: Add bulk repository functions**

In `packages/db/src/repository/requirement.ts`:

Update the existing drizzle-orm import to also include `inArray`:
```typescript
import { and, eq, ilike, inArray, or, sql } from "drizzle-orm";
```

Then add two new functions at the end of the file:

```typescript
export function bulkUpdateRequirements(
    ids: string[],
    userId: string,
    params: { status?: string; useCaseId?: string | null }
) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.status !== undefined) setClause.status = params.status;
            if (params.useCaseId !== undefined) setClause.useCaseId = params.useCaseId;

            if (Object.keys(setClause).length === 0) return 0;

            const result = await db
                .update(requirement)
                .set(setClause)
                .where(and(inArray(requirement.id, ids), eq(requirement.userId, userId)));
            return result.rowCount ?? 0;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function bulkDeleteRequirements(ids: string[], userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await db
                .delete(requirement)
                .where(and(inArray(requirement.id, ids), eq(requirement.userId, userId)));
            return result.rowCount ?? 0;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Add bulk service function**

In `packages/api/src/requirements/service.ts`, add:

```typescript
import {
    // ... existing imports
    bulkUpdateRequirements,
    bulkDeleteRequirements
} from "@fubbik/db/repository";

export function bulkAction(
    userId: string,
    body: {
        ids: string[];
        action: "set_status" | "set_use_case" | "delete";
        status?: string;
        useCaseId?: string | null;
    }
) {
    switch (body.action) {
        case "set_status":
            return bulkUpdateRequirements(body.ids, userId, { status: body.status });
        case "set_use_case":
            return bulkUpdateRequirements(body.ids, userId, { useCaseId: body.useCaseId });
        case "delete":
            return bulkDeleteRequirements(body.ids, userId);
    }
}
```

- [ ] **Step 3: Add the bulk route**

In `packages/api/src/requirements/routes.ts`, chain a new `.patch()` call after the stats endpoint's closing `)` and before the export endpoint's `.get()`. This must come before the `/:id` routes so Elysia doesn't match `"bulk"` as an `:id` param:

```typescript
// Bulk operations (chain after stats endpoint, before list/export)
.patch(
    "/requirements/bulk",
    ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                return yield* requirementService.bulkAction(session.user.id, ctx.body);
            })
        ),
    {
        body: t.Object({
            ids: t.Array(t.String(), { minItems: 1, maxItems: 100 }),
            action: t.Union([
                t.Literal("set_status"),
                t.Literal("set_use_case"),
                t.Literal("delete")
            ]),
            status: t.Optional(StatusSchema),
            useCaseId: t.Optional(t.Union([t.String(), t.Null()]))
        })
    }
)
```

- [ ] **Step 4: Export the new repository functions**

Verify `packages/db/src/repository/index.ts` re-exports from `./requirement` (it does via `export * from "./requirement"`).

- [ ] **Step 5: Verify types**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/requirement.ts packages/api/src/requirements/service.ts packages/api/src/requirements/routes.ts
git commit -m "feat: add bulk operations API for requirements"
```

---

### Task 6: Enhance coverage API for matrix data

Add detailed requirement-chunk pair data to the coverage endpoint.

**Files:**
- Modify: `packages/db/src/repository/coverage.ts` (add `getChunkCoverageMatrix`)
- Modify: `packages/api/src/coverage/service.ts` (add matrix support)
- Modify: `packages/api/src/coverage/routes.ts` (add `detail` query param)

- [ ] **Step 1: Add matrix repository function**

In `packages/db/src/repository/coverage.ts`, add:

```typescript
import { requirement } from "../schema/requirement";

export function getChunkCoverageMatrix(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.userId, userId), isNull(chunk.archivedAt)];

            // Get all requirement-chunk pairs
            let pairsQuery;
            if (codebaseId) {
                pairsQuery = db
                    .select({
                        chunkId: requirementChunk.chunkId,
                        chunkTitle: chunk.title,
                        requirementId: requirementChunk.requirementId,
                        requirementTitle: requirement.title,
                        requirementStatus: requirement.status
                    })
                    .from(requirementChunk)
                    .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                    .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                    .innerJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                    .where(and(
                        eq(chunk.userId, userId),
                        isNull(chunk.archivedAt),
                        eq(chunkCodebase.codebaseId, codebaseId)
                    ));
            } else {
                pairsQuery = db
                    .select({
                        chunkId: requirementChunk.chunkId,
                        chunkTitle: chunk.title,
                        requirementId: requirementChunk.requirementId,
                        requirementTitle: requirement.title,
                        requirementStatus: requirement.status
                    })
                    .from(requirementChunk)
                    .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                    .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                    .where(and(...conditions));
            }

            return pairsQuery;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Update coverage service**

In `packages/api/src/coverage/service.ts`, add:

```typescript
import { getChunkCoverage, getChunkCoverageMatrix } from "@fubbik/db/repository";

export function getCoverageMatrix(userId: string, codebaseId?: string) {
    return Effect.all({
        coverage: getCoverage(userId, codebaseId),
        matrix: getChunkCoverageMatrix(userId, codebaseId)
    }).pipe(
        Effect.map(({ coverage, matrix }) => ({
            ...coverage,
            matrix
        }))
    );
}
```

- [ ] **Step 3: Update coverage route**

In `packages/api/src/coverage/routes.ts`, update to accept `detail` param:

```typescript
export const coverageRoutes = new Elysia().get(
    "/requirements/coverage",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    ctx.query.detail === "true"
                        ? coverageService.getCoverageMatrix(session.user.id, ctx.query.codebaseId)
                        : coverageService.getCoverage(session.user.id, ctx.query.codebaseId)
                )
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String()),
            detail: t.Optional(t.String())
        })
    }
);
```

- [ ] **Step 4: Verify types**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/repository/coverage.ts packages/api/src/coverage/service.ts packages/api/src/coverage/routes.ts
git commit -m "feat: add coverage matrix detail data to coverage API"
```

---

### Task 7: Redesign the requirements list page

Replace the current flat list with the card + sidebar layout.

**Files:**
- Create: `apps/web/src/features/requirements/requirement-card.tsx`
- Create: `apps/web/src/features/requirements/sidebar-filters.tsx`
- Create: `apps/web/src/features/requirements/bulk-actions.tsx`
- Modify: `apps/web/src/routes/requirements.tsx` (full rewrite)

- [ ] **Step 1: Create the RequirementCard component**

```typescript
// apps/web/src/features/requirements/requirement-card.tsx
import { Link } from "@tanstack/react-router";
import { Bot } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";

interface RequirementCardProps {
    id: string;
    title: string;
    description: string | null;
    status: string;
    priority: string | null;
    steps: Array<{ keyword: string; text: string }>;
    origin: string;
    reviewStatus: string;
    useCaseName?: string;
    chunkCount?: number;
    selected: boolean;
    onSelectChange: (selected: boolean) => void;
}

// Status badge color helper
function statusColor(status: string) { /* same as existing */ }
function priorityLabel(priority: string) { /* same as existing */ }

// Step preview: condensed "Given X · When Y · Then Z"
function stepPreview(steps: Array<{ keyword: string; text: string }>) {
    const given = steps.find(s => s.keyword === "given");
    const when = steps.find(s => s.keyword === "when");
    const then_ = steps.find(s => s.keyword === "then");
    const parts: string[] = [];
    if (given) parts.push(`Given ${given.text}`);
    if (when) parts.push(`When ${when.text}`);
    if (then_) parts.push(`Then ${then_.text}`);
    return parts.join(" · ");
}

export function RequirementCard({ ... }: RequirementCardProps) {
    // Render card with:
    // - Checkbox for selection
    // - Title as Link to detail page
    // - Status badge, priority badge, AI badge
    // - Step preview text (truncated)
    // - Footer: step count, chunk count
    // - Failing requirements get subtle red border
}
```

Implement the full JSX following the card layout from the brainstorm mockup:
- Clickable title linking to `/requirements/$requirementId`
- Color-coded status badge
- Condensed step preview as muted text
- Footer with step count, chunk count, priority
- Checkbox in top-left corner
- Failing cards get `border-red-500/30 bg-red-500/5` styling

- [ ] **Step 2: Create the SidebarFilters component**

```typescript
// apps/web/src/features/requirements/sidebar-filters.tsx
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";

interface SidebarFiltersProps {
    search: string;
    onSearchChange: (value: string) => void;
    statusFilters: string[];
    onStatusFiltersChange: (statuses: string[]) => void;
    priorityFilters: string[];
    onPriorityFiltersChange: (priorities: string[]) => void;
    originFilter: string;
    onOriginFilterChange: (origin: string) => void;
    useCases: Array<{ id: string; name: string; requirementCount: number }>;
    activeUseCaseId: string | null;
    onUseCaseClick: (id: string | null) => void;
    onCreateUseCase: () => void;
}
```

Layout:
- Search input at top
- STATUS section with checkboxes (Passing, Failing, Untested)
- PRIORITY section with checkboxes (Must, Should, Could, Won't)
- ORIGIN section with toggle (All / Human / AI)
- USE CASES section with clickable list items showing counts
- Highlighted active use case
- "Ungrouped" entry at bottom

- [ ] **Step 3: Create the BulkActions component**

```typescript
// apps/web/src/features/requirements/bulk-actions.tsx
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface BulkActionsProps {
    selectedIds: string[];
    onClearSelection: () => void;
    useCases: Array<{ id: string; name: string }>;
}
```

Renders a sticky bar at the bottom when `selectedIds.length > 0`:
- "N selected" count
- "Set Status" dropdown (passing/failing/untested) — calls `PATCH /requirements/bulk` with `action: "set_status"`
- "Assign Use Case" dropdown — calls with `action: "set_use_case"`
- "Delete" button with confirm — calls with `action: "delete"`
- "Clear" button to deselect all
- On success: invalidate queries, clear selection, toast

- [ ] **Step 4: Rewrite the requirements list page**

Rewrite `apps/web/src/routes/requirements.tsx`:

Structure:
```tsx
function RequirementsPage() {
    // State
    const [search, setSearch] = useState("");
    const [statusFilters, setStatusFilters] = useState<string[]>([]);
    const [priorityFilters, setPriorityFilters] = useState<string[]>([]);
    const [originFilter, setOriginFilter] = useState("");
    const [activeUseCaseId, setActiveUseCaseId] = useState<string | null>(null);
    const [selectedIds, setSelectedIds] = useState<string[]>([]);
    const [page, setPage] = useState(0);
    const pageSize = 20;

    // Queries
    const statsQuery = useQuery({ ... }); // existing
    const useCasesQuery = useQuery({ ... }); // existing
    const listQuery = useQuery({
        queryKey: ["requirements", codebaseId, search, statusFilters, priorityFilters, originFilter, activeUseCaseId, page],
        queryFn: async () => {
            const query: Record<string, string> = {
                limit: String(pageSize),
                offset: String(page * pageSize)
            };
            if (codebaseId) query.codebaseId = codebaseId;
            if (search) query.search = search;
            // Status/priority filters: when checkboxes are used, apply client-side post-filtering
            // since the API only accepts single values. Send single value if exactly one selected.
            if (statusFilters.length === 1) query.status = statusFilters[0]!;
            if (priorityFilters.length === 1) query.priority = priorityFilters[0]!;
            if (originFilter) query.origin = originFilter;
            if (activeUseCaseId) query.useCaseId = activeUseCaseId;
            return unwrapEden(await api.api.requirements.get({ query }));
        }
    });

    // Client-side post-filtering for multi-select status/priority
    const filteredRequirements = useMemo(() => {
        if (!data?.requirements) return [];
        let reqs = data.requirements as Array<Record<string, unknown>>;
        if (statusFilters.length > 1) {
            reqs = reqs.filter(r => statusFilters.includes(r.status as string));
        }
        if (priorityFilters.length > 1) {
            reqs = reqs.filter(r => priorityFilters.includes(r.priority as string));
        }
        return reqs;
    }, [data, statusFilters, priorityFilters]);

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            {/* Header with title + New Requirement button */}
            {/* Stats bar */}
            <div className="flex gap-6">
                {/* Sidebar */}
                <div className="w-64 shrink-0">
                    <SidebarFilters ... />
                </div>
                {/* Main content */}
                <div className="flex-1">
                    {/* Cards */}
                    {requirements.map(req => (
                        <RequirementCard ... />
                    ))}
                    {/* Pagination */}
                </div>
            </div>
            {/* Bulk actions bar */}
            <BulkActions ... />
        </div>
    );
}
```

Move the use case CRUD from the current page into the sidebar. The `SidebarFilters` component should:
- Accept `onCreateUseCase`, `onEditUseCase`, `onDeleteUseCase` callbacks
- Show a "+" button next to the USE CASES heading that reveals an inline input for creating a new use case (name + optional description)
- Each use case item in the list gets a hover-visible edit (pencil) and delete (trash) icon
- Editing replaces the use case name with an inline input + save/cancel
- All mutations (create/edit/delete use case) are handled in the parent page component and passed down as callbacks, keeping the sidebar component presentational
- After mutation success, invalidate `["use-cases"]` and `["requirements"]` queries

Add pagination controls at the bottom of the card list:
```tsx
{data && data.total > pageSize && (
    <div className="mt-4 flex items-center justify-between text-sm text-muted-foreground">
        <span>Showing {page * pageSize + 1}-{Math.min((page + 1) * pageSize, data.total)} of {data.total}</span>
        <div className="flex gap-2">
            <Button variant="outline" size="sm" disabled={page === 0} onClick={() => setPage(p => p - 1)}>Previous</Button>
            <Button variant="outline" size="sm" disabled={(page + 1) * pageSize >= data.total} onClick={() => setPage(p => p + 1)}>Next</Button>
        </div>
    </div>
)}
```

- [ ] **Step 5: Verify types and test manually**

Run: `pnpm run check-types`
Expected: No type errors

Run: `pnpm dev` and navigate to `/requirements` to visually verify the new layout.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/requirements/requirement-card.tsx apps/web/src/features/requirements/sidebar-filters.tsx apps/web/src/features/requirements/bulk-actions.tsx apps/web/src/routes/requirements.tsx
git commit -m "feat: redesign requirements list page with card+sidebar layout and bulk actions"
```

---

### Task 8: Add inline edit mode to the detail page

Add a view/edit toggle to the requirement detail page using the shared StepBuilder and ChunkLinker components.

**Files:**
- Modify: `apps/web/src/routes/requirements_.$requirementId.tsx` (add edit mode)

- [ ] **Step 1: Add edit mode state and mutations**

At the top of the `RequirementDetail` component, add:
```typescript
const [editing, setEditing] = useState(false);

// Edit form state — initialized from data when entering edit mode
const [editTitle, setEditTitle] = useState("");
const [editDescription, setEditDescription] = useState("");
const [editPriority, setEditPriority] = useState("");
const [editUseCaseId, setEditUseCaseId] = useState("");
const [editSteps, setEditSteps] = useState<StepRow[]>([]);
const [editChunkIds, setEditChunkIds] = useState<string[]>([]);
const [editStepErrors, setEditStepErrors] = useState<StepError[]>([]);
```

Add a function to enter edit mode:
```typescript
function enterEditMode() {
    setEditTitle(title);
    setEditDescription(description ?? "");
    setEditPriority(priority ?? "");
    setEditUseCaseId((data.useCaseId as string) ?? "");
    setEditSteps(steps.map(s => ({ keyword: s.keyword as Keyword, text: s.text })));
    setEditChunkIds(chunks.map(c => c.id));
    setEditStepErrors([]);
    setEditing(true);
}
```

Add a save mutation:
```typescript
const updateMutation = useMutation({
    mutationFn: async () => {
        const body: Record<string, unknown> = {};
        if (editTitle !== title) body.title = editTitle.trim();
        if (editDescription !== (description ?? "")) body.description = editDescription.trim() || null;
        if (editPriority !== (priority ?? "")) body.priority = editPriority || null;
        if (editUseCaseId !== ((data.useCaseId as string) ?? "")) body.useCaseId = editUseCaseId || null;

        const stepsChanged = JSON.stringify(editSteps) !== JSON.stringify(steps);
        if (stepsChanged) body.steps = editSteps.map(s => ({ keyword: s.keyword, text: s.text.trim() }));

        if (Object.keys(body).length > 0) {
            await unwrapEden(api.api.requirements({ id: requirementId }).patch(body));
        }

        const chunksChanged = JSON.stringify(editChunkIds.sort()) !== JSON.stringify(chunks.map(c => c.id).sort());
        if (chunksChanged) {
            await unwrapEden(api.api.requirements({ id: requirementId }).chunks.put({ chunkIds: editChunkIds }));
        }
    },
    onSuccess: () => {
        queryClient.invalidateQueries({ queryKey: ["requirement", requirementId] });
        queryClient.invalidateQueries({ queryKey: ["requirements"] });
        setEditing(false);
        toast.success("Requirement updated");
    },
    onError: () => toast.error("Failed to update")
});
```

- [ ] **Step 2: Add imports for shared components**

```typescript
import { StepBuilder } from "@/features/requirements/step-builder";
import { ChunkLinker } from "@/features/requirements/chunk-linker";
import { validateSteps, type Keyword, type StepRow, type StepError } from "@/features/requirements/validation";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
```

Add use case query:
```typescript
const { codebaseId } = useActiveCodebase();
const useCasesQuery = useQuery({
    queryKey: ["use-cases", codebaseId],
    queryFn: async () => {
        const query: { codebaseId?: string } = {};
        if (codebaseId) query.codebaseId = codebaseId;
        return unwrapEden(await api.api["use-cases"].get({ query })) as Array<{ id: string; name: string }>;
    }
});
```

- [ ] **Step 3: Update JSX for view/edit toggle**

Add an Edit button to the header (view mode):
```tsx
<div className="mb-6 flex items-center justify-between">
    <h1 className="text-2xl font-bold tracking-tight">
        {editing ? (
            <Input value={editTitle} onChange={e => setEditTitle(e.target.value)} className="text-2xl font-bold" />
        ) : title}
    </h1>
    {!editing && (
        <Button variant="outline" size="sm" onClick={enterEditMode}>
            <Pencil className="mr-1 size-3.5" /> Edit
        </Button>
    )}
</div>
```

Replace the steps section with a conditional:
```tsx
{editing ? (
    <>
        {/* Description textarea */}
        <div className="mb-4">
            <label className="mb-1.5 block text-sm font-medium">Description</label>
            <textarea
                value={editDescription}
                onChange={e => setEditDescription(e.target.value)}
                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                rows={3}
            />
        </div>
        {/* Priority + Use Case selects */}
        <div className="mb-4 flex gap-4">
            {/* Priority select */}
            {/* Use Case select */}
        </div>
        {/* Step builder */}
        <StepBuilder steps={editSteps} onStepsChange={setEditSteps} codebaseId={codebaseId} stepErrors={editStepErrors} />
        {/* Chunk linker */}
        <ChunkLinker selectedChunkIds={editChunkIds} onSelectedChunkIdsChange={setEditChunkIds} codebaseId={codebaseId} />
        {/* Save/Cancel */}
        <div className="mt-6 flex gap-2 justify-end border-t pt-4">
            <Button variant="outline" onClick={() => setEditing(false)}>Cancel</Button>
            <Button onClick={() => {
                const errors = validateSteps(editSteps);
                editSteps.forEach((s, i) => { if (!s.text.trim()) errors.push({ step: i, error: "Step text is required" }); });
                setEditStepErrors(errors);
                if (errors.length === 0) updateMutation.mutate();
            }} disabled={updateMutation.isPending}>
                {updateMutation.isPending ? "Saving..." : "Save Changes"}
            </Button>
        </div>
    </>
) : (
    /* existing view-mode JSX for steps, chunks, export, etc. */
)}
```

- [ ] **Step 4: Add Pencil import**

Add `Pencil` to the lucide-react imports at the top of the file.

- [ ] **Step 5: Verify types**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/routes/requirements_.\$requirementId.tsx
git commit -m "feat: add inline edit mode to requirement detail page"
```

---

### Task 9: Create the coverage matrix page

Add a new page at `/requirements/coverage` showing the chunk-requirement coverage matrix.

**Files:**
- Create: `apps/web/src/routes/requirements_.coverage.tsx`

- [ ] **Step 1: Create the coverage matrix page**

```typescript
// apps/web/src/routes/requirements_.coverage.tsx
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckCircle2, XCircle } from "lucide-react";

import { BackLink } from "@/components/back-link";
import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/requirements_/coverage")({
    component: CoverageMatrix,
    beforeLoad: async () => {
        let session = null;
        try { session = await getUser(); } catch {}
        return { session };
    }
});

interface MatrixPair {
    chunkId: string;
    chunkTitle: string;
    requirementId: string;
    requirementTitle: string;
    requirementStatus: string;
}

function CoverageMatrix() {
    const { codebaseId } = useActiveCodebase();

    const coverageQuery = useQuery({
        queryKey: ["coverage-matrix", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string; detail: string } = { detail: "true" };
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.requirements.coverage.get({ query })) as {
                covered: Array<{ id: string; title: string; requirementCount: number }>;
                uncovered: Array<{ id: string; title: string }>;
                stats: { total: number; covered: number; uncovered: number; percentage: number };
                matrix: MatrixPair[];
            };
        }
    });

    const data = coverageQuery.data;

    // Build matrix data structure
    // Rows = chunks, Columns = requirements
    // Use a Map<chunkId, Set<requirementId>> for lookup

    return (
        <div className="container mx-auto max-w-7xl px-4 py-8">
            <BackLink to="/requirements" label="Requirements" />
            <h1 className="mb-2 text-2xl font-bold tracking-tight">Requirement Coverage</h1>

            {/* Stats summary */}
            {data && (
                <div className="mb-6">
                    <div className="flex items-center gap-4 mb-2">
                        <span className="text-sm text-muted-foreground">
                            {data.stats.covered} of {data.stats.total} chunks covered ({data.stats.percentage}%)
                        </span>
                    </div>
                    {/* Progress bar */}
                    <div className="h-2 w-full rounded-full bg-muted">
                        <div
                            className="h-2 rounded-full bg-emerald-500 transition-all"
                            style={{ width: `${data.stats.percentage}%` }}
                        />
                    </div>
                </div>
            )}

            {/* Matrix table */}
            {data && (
                <Card>
                    <CardPanel className="overflow-x-auto p-0">
                        <table className="w-full text-sm">
                            <thead>
                                <tr className="border-b">
                                    <th className="sticky left-0 bg-background px-4 py-2 text-left font-medium">Chunk</th>
                                    {/* Column headers = unique requirements from matrix */}
                                    {uniqueRequirements.map(req => (
                                        <th key={req.id} className="px-3 py-2 text-center">
                                            <Link to="/requirements/$requirementId" params={{ requirementId: req.id }}
                                                className="text-xs font-medium hover:underline block max-w-[120px] truncate">
                                                {req.title}
                                            </Link>
                                        </th>
                                    ))}
                                </tr>
                            </thead>
                            <tbody>
                                {allChunks.map(chunk => (
                                    <tr key={chunk.id} className={isCovered(chunk.id) ? "" : "bg-amber-500/5"}>
                                        <td className="sticky left-0 bg-background px-4 py-2 border-t">
                                            <Link to="/chunks/$chunkId" params={{ chunkId: chunk.id }}
                                                className="hover:underline truncate block max-w-[250px]">
                                                {chunk.title}
                                            </Link>
                                        </td>
                                        {uniqueRequirements.map(req => (
                                            <td key={req.id} className="px-3 py-2 text-center border-t">
                                                {coverageMap.get(chunk.id)?.has(req.id) ? (
                                                    <CheckCircle2 className="inline size-4 text-emerald-500" />
                                                ) : null}
                                            </td>
                                        ))}
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </CardPanel>
                </Card>
            )}
        </div>
    );
}
```

Build the matrix data structures from the query data:
```typescript
// Derive unique requirements and chunks from matrix + uncovered
const uniqueRequirements = useMemo(() => {
    if (!data) return [];
    const map = new Map<string, { id: string; title: string }>();
    for (const pair of data.matrix) {
        map.set(pair.requirementId, { id: pair.requirementId, title: pair.requirementTitle });
    }
    return Array.from(map.values());
}, [data]);

const allChunks = useMemo(() => {
    if (!data) return [];
    const coveredChunks = data.covered.map(c => ({ id: c.id, title: c.title }));
    const uncoveredChunks = data.uncovered.map(c => ({ id: c.id, title: c.title }));
    return [...coveredChunks, ...uncoveredChunks];
}, [data]);

const coverageMap = useMemo(() => {
    if (!data) return new Map<string, Set<string>>();
    const map = new Map<string, Set<string>>();
    for (const pair of data.matrix) {
        if (!map.has(pair.chunkId)) map.set(pair.chunkId, new Set());
        map.get(pair.chunkId)!.add(pair.requirementId);
    }
    return map;
}, [data]);

function isCovered(chunkId: string) {
    return coverageMap.has(chunkId) && coverageMap.get(chunkId)!.size > 0;
}
```

- [ ] **Step 2: Add a link to the coverage page from the requirements list**

In `apps/web/src/routes/requirements.tsx`, add a "Coverage" button in the header next to "New Requirement":

```tsx
<Button variant="outline" size="sm" render={<Link to="/requirements/coverage" />}>
    <BarChart3 className="mr-1 size-4" />
    Coverage
</Button>
```

- [ ] **Step 3: Verify types**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/requirements_.coverage.tsx apps/web/src/routes/requirements.tsx
git commit -m "feat: add requirement coverage matrix page"
```

---

### Task 10: Final integration and cleanup

Wire everything together, verify the full flow, clean up unused code.

**Files:**
- Modify: `apps/web/src/features/nav/mobile-nav.tsx` (add coverage to nav if desired)
- Various files for cleanup

- [ ] **Step 1: Run full type check**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 2: Run tests**

Run: `pnpm test`
Expected: All tests pass

- [ ] **Step 3: Run linting**

Run: `pnpm ci`
Expected: Clean CI run

- [ ] **Step 4: Manual verification checklist**

Run `pnpm dev` and verify:
- [ ] Requirements list page shows card+sidebar layout
- [ ] Search works in sidebar
- [ ] Status/priority checkbox filters work
- [ ] Use case filter in sidebar works
- [ ] Pagination works
- [ ] Bulk select + status change works
- [ ] Bulk select + use case assign works
- [ ] Bulk select + delete works
- [ ] Clicking a card navigates to detail page
- [ ] Detail page shows "Edit" button
- [ ] Edit mode toggles inline editing
- [ ] Step builder works in edit mode (add/remove/reorder steps)
- [ ] Chunk linker works in edit mode
- [ ] Save/Cancel in edit mode works
- [ ] Coverage matrix page loads and shows data
- [ ] Coverage matrix links navigate correctly
- [ ] Create new requirement still works with refactored components

- [ ] **Step 5: Commit any remaining cleanup**

```bash
git add -A
git commit -m "chore: final cleanup for requirements redesign"
```
