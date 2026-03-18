# Requirements Structure & Organization — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add sub use cases (single-level nesting), requirement dependencies with dependency graph, and drag-to-reorder requirements within use cases.

**Architecture:** Schema changes (parentId on use_case, new requirement_dependency table, order on requirement) with corresponding repository/service/route layers. Frontend updates to sidebar (tree view), detail page (dependency section + React Flow graph), and list page (drag-and-drop via @dnd-kit).

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Start, React Query, @xyflow/react, @dagrejs/dagre, @dnd-kit/core + @dnd-kit/sortable

---

### Task 1: Schema changes and migration

Add `parentId` to use_case, `order` to requirement, and create `requirement_dependency` table.

**Files:**
- Modify: `packages/db/src/schema/use-case.ts`
- Modify: `packages/db/src/schema/requirement.ts`
- Create: `packages/db/src/schema/requirement-dependency.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Add `parentId` to use_case schema**

In `packages/db/src/schema/use-case.ts`:
- Add self-referential `parentId` column:
  ```typescript
  parentId: text("parent_id").references(() => useCase.id, { onDelete: "cascade" }),
  ```
- Add index to the table builder array:
  ```typescript
  index("use_case_parentId_idx").on(table.parentId)
  ```
- Update `useCaseRelations` to add self-referential relations:
  ```typescript
  export const useCaseRelations = relations(useCase, ({ one, many }) => ({
      user: one(user, { fields: [useCase.userId], references: [user.id] }),
      codebase: one(codebase, { fields: [useCase.codebaseId], references: [codebase.id] }),
      parent: one(useCase, { fields: [useCase.parentId], references: [useCase.id], relationName: "children" }),
      children: many(useCase, { relationName: "children" })
  }));
  ```

- [ ] **Step 2: Add `order` to requirement schema**

In `packages/db/src/schema/requirement.ts`:

First, add `integer` to the existing drizzle import:
```typescript
import { index, integer, jsonb, pgTable, primaryKey, text, timestamp } from "drizzle-orm/pg-core";
```

Then add to the requirement table columns:
```typescript
order: integer("order").notNull().default(0),
```

- [ ] **Step 3: Create requirement_dependency schema**

Create `packages/db/src/schema/requirement-dependency.ts`:
```typescript
import { pgTable, primaryKey, text, check } from "drizzle-orm/pg-core";
import { sql } from "drizzle-orm";
import { requirement } from "./requirement";

export const requirementDependency = pgTable(
    "requirement_dependency",
    {
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        dependsOnId: text("depends_on_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.requirementId, table.dependsOnId] }),
        check("no_self_dependency", sql`${table.requirementId} != ${table.dependsOnId}`)
    ]
);
```

- [ ] **Step 4: Add to schema barrel export**

In `packages/db/src/schema/index.ts`, add:
```typescript
export * from "./requirement-dependency";
```

- [ ] **Step 5: Generate and apply migration**

Run:
```bash
pnpm db:generate
```
Review the generated SQL file in `packages/db/src/migrations/`.

Run against local DB:
```bash
pnpm db:push
```

- [ ] **Step 6: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/schema/ packages/db/src/migrations/
git commit -m "feat: add schema for sub use cases, requirement deps, and ordering"
```

---

### Task 2: Sub use cases — repository and service

Add `parentId` support to use case CRUD and add child count aggregation.

**Files:**
- Modify: `packages/db/src/repository/use-case.ts`
- Modify: `packages/api/src/use-cases/service.ts`

- [ ] **Step 1: Update use case repository**

In `packages/db/src/repository/use-case.ts`:

Update `CreateUseCaseParams` to include `parentId?: string`.

Update `createUseCase` to persist `parentId`.

Update `listUseCases` to include `parentId` in the select and add child count. The current query selects from `useCase` with a left join on `requirement` for counts. Add `parentId` to the select fields:
```typescript
parentId: useCase.parentId,
```

Add a child count subquery. After the existing requirement count aggregation, add:
```typescript
childCount: sql<number>`(SELECT count(*) FROM use_case uc2 WHERE uc2.parent_id = ${useCase.id})`.as("child_count"),
```

Update `updateUseCase` to accept and persist `parentId`:
```typescript
if (params.parentId !== undefined) setClause.parentId = params.parentId;
```

Add `UpdateUseCaseParams` type to include `parentId?: string | null`.

- [ ] **Step 2: Add nesting validation in service**

In `packages/api/src/use-cases/service.ts`:

First, update the imports in the service file to include `ValidationError`:
```typescript
import { NotFoundError, ValidationError } from "../errors";
```

Update `createUseCase` to validate parentId if provided:
```typescript
export function createUseCase(userId: string, body: { name: string; description?: string; codebaseId?: string; parentId?: string }) {
    return Effect.gen(function* () {
        if (body.parentId) {
            const parent = yield* getUseCaseById(body.parentId, userId);
            if (!parent) return yield* Effect.fail(new NotFoundError({ resource: "Parent use case" }));
            if (parent.parentId) return yield* Effect.fail(new ValidationError({ message: "Cannot nest more than one level deep" }));
        }
        const id = crypto.randomUUID();
        return yield* createUseCaseRepo({ id, ...body, userId });
    });
}
```

Update `updateUseCase` to validate parentId changes:
```typescript
export function updateUseCase(id: string, userId: string, body: { name?: string; description?: string | null; parentId?: string | null; order?: number }) {
    return Effect.gen(function* () {
        if (body.parentId !== undefined && body.parentId !== null) {
            const parent = yield* getUseCaseById(body.parentId, userId);
            if (!parent) return yield* Effect.fail(new NotFoundError({ resource: "Parent use case" }));
            if (parent.parentId) return yield* Effect.fail(new ValidationError({ message: "Cannot nest more than one level deep" }));
            if (body.parentId === id) return yield* Effect.fail(new ValidationError({ message: "Cannot be its own parent" }));
            // Check target parent is not a child of this use case
            const children = yield* listUseCasesRepo(userId);
            const isChild = children.some(uc => uc.parentId === id && uc.id === body.parentId);
            if (isChild) return yield* Effect.fail(new ValidationError({ message: "Cannot set parent to own child" }));
        }
        return yield* updateUseCaseRepo(id, userId, body);
    });
}
```

- [ ] **Step 3: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/use-case.ts packages/api/src/use-cases/service.ts
git commit -m "feat: add sub use case support to repository and service"
```

---

### Task 3: Sub use cases — routes and frontend

Wire parentId through the API and update the sidebar to show a tree.

**Files:**
- Modify: `packages/api/src/use-cases/routes.ts`
- Modify: `apps/web/src/features/requirements/sidebar-filters.tsx`

- [ ] **Step 1: Update use case routes**

In `packages/api/src/use-cases/routes.ts`:

Add `parentId: t.Optional(t.String())` to the POST body schema.

Add `parentId: t.Optional(t.Union([t.String(), t.Null()]))` to the PATCH body schema.

- [ ] **Step 2: Update sidebar to show tree**

In `apps/web/src/features/requirements/sidebar-filters.tsx`:

Update the `useCases` prop type to include `parentId`:
```typescript
useCases: Array<{ id: string; name: string; requirementCount: number; parentId: string | null }>;
```

Replace the flat use case list with a tree:
```typescript
// Separate into parents and children
const topLevel = useCases.filter(uc => !uc.parentId);
const childrenMap = new Map<string, typeof useCases>();
for (const uc of useCases) {
    if (uc.parentId) {
        if (!childrenMap.has(uc.parentId)) childrenMap.set(uc.parentId, []);
        childrenMap.get(uc.parentId)!.push(uc);
    }
}
```

Render top-level use cases with expandable children:
- Each parent shows a chevron toggle (ChevronRight/ChevronDown)
- Children indented with `pl-4` and slightly smaller text
- Parent requirement counts include children's counts (sum client-side)
- Clicking a parent filters to that parent's ID (the requirements list page handles showing children's requirements)
- Track expanded state with `useState<Set<string>>`

- [ ] **Step 3: Update requirements list page to handle parent use case filtering**

In `apps/web/src/routes/requirements.tsx`:

When `activeUseCaseId` is set and it's a parent use case, also include requirements from its children. The simplest approach: when filtering client-side, check if `activeUseCaseId` matches the requirement's `useCaseId` OR if the requirement's `useCaseId` is a child of `activeUseCaseId`. Build a set of child IDs from the use cases data.

- [ ] **Step 4: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/use-cases/routes.ts apps/web/src/features/requirements/sidebar-filters.tsx apps/web/src/routes/requirements.tsx
git commit -m "feat: add sub use case UI with tree sidebar and parent filtering"
```

---

### Task 4: Requirement dependency — repository

Add the dependency CRUD and transitive query functions.

**Files:**
- Create: `packages/db/src/repository/requirement-dependency.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create dependency repository**

Create `packages/db/src/repository/requirement-dependency.ts`:

```typescript
import { and, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { requirement } from "../schema/requirement";
import { requirementDependency } from "../schema/requirement-dependency";

export function addDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: () =>
            db.insert(requirementDependency)
                .values({ requirementId, dependsOnId })
                .onConflictDoNothing()
                .returning(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function removeDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: () =>
            db.delete(requirementDependency)
                .where(and(
                    eq(requirementDependency.requirementId, requirementId),
                    eq(requirementDependency.dependsOnId, dependsOnId)
                )),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getDependencies(requirementId: string) {
    return Effect.tryPromise({
        try: async () => {
            const dependsOn = await db
                .select({
                    id: requirement.id,
                    title: requirement.title,
                    status: requirement.status,
                    priority: requirement.priority
                })
                .from(requirementDependency)
                .innerJoin(requirement, eq(requirementDependency.dependsOnId, requirement.id))
                .where(eq(requirementDependency.requirementId, requirementId));

            const dependedOnBy = await db
                .select({
                    id: requirement.id,
                    title: requirement.title,
                    status: requirement.status,
                    priority: requirement.priority
                })
                .from(requirementDependency)
                .innerJoin(requirement, eq(requirementDependency.requirementId, requirement.id))
                .where(eq(requirementDependency.dependsOnId, requirementId));

            return { dependsOn, dependedOnBy };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTransitiveDependencies(requirementId: string) {
    return Effect.tryPromise({
        try: async () => {
            // Recursive CTE for ancestors (everything this requirement depends on, transitively)
            const ancestors = await db.execute(sql`
                WITH RECURSIVE ancestors AS (
                    SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = ${requirementId}
                    UNION
                    SELECT rd.depends_on_id FROM requirement_dependency rd
                    INNER JOIN ancestors a ON rd.requirement_id = a.id
                )
                SELECT r.id, r.title, r.status, r.priority
                FROM ancestors a
                INNER JOIN requirement r ON r.id = a.id
            `);

            // Recursive CTE for descendants (everything that depends on this, transitively)
            const descendants = await db.execute(sql`
                WITH RECURSIVE descendants AS (
                    SELECT requirement_id AS id FROM requirement_dependency WHERE depends_on_id = ${requirementId}
                    UNION
                    SELECT rd.requirement_id FROM requirement_dependency rd
                    INNER JOIN descendants d ON rd.depends_on_id = d.id
                )
                SELECT r.id, r.title, r.status, r.priority
                FROM descendants d
                INNER JOIN requirement r ON r.id = d.id
            `);

            // All edges involving any node in the graph
            const allNodeIds = [
                requirementId,
                ...ancestors.rows.map((r: any) => r.id),
                ...descendants.rows.map((r: any) => r.id)
            ];

            const edges = allNodeIds.length > 0 ? await db.execute(sql`
                SELECT requirement_id AS source, depends_on_id AS target
                FROM requirement_dependency
                WHERE requirement_id = ANY(${allNodeIds})
                   OR depends_on_id = ANY(${allNodeIds})
            `) : { rows: [] };

            return { ancestors: ancestors.rows, descendants: descendants.rows, edges: edges.rows };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function checkCircularDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: async () => {
            // Check if adding requirementId → dependsOnId would create a cycle
            // by checking if requirementId is reachable from dependsOnId
            const result = await db.execute(sql`
                WITH RECURSIVE chain AS (
                    SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = ${dependsOnId}
                    UNION
                    SELECT rd.depends_on_id FROM requirement_dependency rd
                    INNER JOIN chain c ON rd.requirement_id = c.id
                )
                SELECT 1 FROM chain WHERE id = ${requirementId} LIMIT 1
            `);
            return result.rows.length > 0; // true = would create cycle
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Add to repository barrel export**

In `packages/db/src/repository/index.ts`, add:
```typescript
export * from "./requirement-dependency";
```

- [ ] **Step 3: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/requirement-dependency.ts packages/db/src/repository/index.ts
git commit -m "feat: add requirement dependency repository with transitive queries"
```

---

### Task 5: Requirement dependency — service and routes

Add the dependency service logic and API endpoints.

**Files:**
- Create: `packages/api/src/requirements/dependency-service.ts`
- Create: `packages/api/src/requirements/dependency-routes.ts`
- Modify: `packages/api/src/index.ts` (register routes)

- [ ] **Step 1: Create dependency service**

Create `packages/api/src/requirements/dependency-service.ts`:

```typescript
import {
    addDependency as addDependencyRepo,
    removeDependency as removeDependencyRepo,
    getDependencies as getDependenciesRepo,
    getTransitiveDependencies,
    checkCircularDependency,
    getRequirementById
} from "@fubbik/db/repository";
import { Effect } from "effect";
import { NotFoundError, ValidationError } from "../errors";

export function addDependency(requirementId: string, dependsOnId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));

        const dep = yield* getRequirementById(dependsOnId, userId);
        if (!dep) return yield* Effect.fail(new NotFoundError({ resource: "Dependency target" }));

        const wouldCycle = yield* checkCircularDependency(requirementId, dependsOnId);
        if (wouldCycle) return yield* Effect.fail(new ValidationError({ message: "Adding this dependency would create a circular reference" }));

        return yield* addDependencyRepo(requirementId, dependsOnId);
    });
}

export function removeDependency(requirementId: string, dependsOnId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));
        return yield* removeDependencyRepo(requirementId, dependsOnId);
    });
}

export function getDependencies(requirementId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));
        return yield* getDependenciesRepo(requirementId);
    });
}

export function getDependencyGraph(requirementId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));

        const { ancestors, descendants, edges } = yield* getTransitiveDependencies(requirementId);

        // Build React Flow nodes and edges
        const allReqs = [
            { id: req.id, title: req.title, status: req.status, priority: req.priority, isCurrent: true },
            ...ancestors.map((r: any) => ({ id: r.id, title: r.title, status: r.status, priority: r.priority, isCurrent: false })),
            ...descendants.map((r: any) => ({ id: r.id, title: r.title, status: r.status, priority: r.priority, isCurrent: false }))
        ];

        // Deduplicate
        const nodeMap = new Map(allReqs.map(r => [r.id, r]));
        const nodes = Array.from(nodeMap.values());

        return {
            nodes,
            edges: (edges as any[]).map((e: any) => ({
                source: e.source,
                target: e.target
            }))
        };
    });
}
```

- [ ] **Step 2: Create dependency routes**

Create `packages/api/src/requirements/dependency-routes.ts`:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as depService from "./dependency-service";

export const dependencyRoutes = new Elysia()
    .post(
        "/requirements/:id/dependencies",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* depService.addDependency(ctx.params.id, ctx.body.dependsOnId, session.user.id);
                    ctx.set.status = 201;
                    return { message: "Dependency added" };
                })
            ),
        {
            body: t.Object({
                dependsOnId: t.String()
            })
        }
    )
    .delete(
        "/requirements/:id/dependencies/:dependsOnId",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* depService.removeDependency(ctx.params.id, ctx.params.dependsOnId, session.user.id);
                    return { message: "Dependency removed" };
                })
            )
    )
    .get(
        "/requirements/:id/dependencies",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* depService.getDependencies(ctx.params.id, session.user.id);
                })
            )
    )
    .get(
        "/requirements/:id/dependencies/graph",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* depService.getDependencyGraph(ctx.params.id, session.user.id);
                })
            )
    );
```

- [ ] **Step 3: Register dependency routes in API index**

In `packages/api/src/index.ts`, import and `.use()` the dependency routes alongside the existing requirement routes. Find where `requirementRoutes` is used and add:
```typescript
import { dependencyRoutes } from "./requirements/dependency-routes";
// ...
.use(dependencyRoutes)
```

- [ ] **Step 4: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/requirements/dependency-service.ts packages/api/src/requirements/dependency-routes.ts packages/api/src/index.ts
git commit -m "feat: add requirement dependency API endpoints"
```

---

### Task 6: Requirement dependency — frontend

Add the dependency section and React Flow graph to the detail page.

**Files:**
- Create: `apps/web/src/features/requirements/dependency-section.tsx`
- Create: `apps/web/src/features/requirements/dependency-graph.tsx`
- Modify: `apps/web/src/routes/requirements_.$requirementId.tsx`

- [ ] **Step 1: Create DependencySection component**

Create `apps/web/src/features/requirements/dependency-section.tsx`:

Props:
```typescript
interface DependencySectionProps {
    requirementId: string;
    editing: boolean; // hide add/remove in view mode, or always show
}
```

Features:
- Fetches from `GET /api/requirements/:id/dependencies` using React Query
- Shows "Depends on" and "Depended on by" sub-sections
- Each item: clickable Link with title + status badge
- "Add dependency" button opens a search UI (similar to ChunkLinker pattern): text input, filtered dropdown of requirements, click to add
- Remove (x) button on each dependency calls DELETE endpoint
- Mutations invalidate the `["dependencies", requirementId]` query key

- [ ] **Step 2: Create DependencyGraph component**

Create `apps/web/src/features/requirements/dependency-graph.tsx`:

Props:
```typescript
interface DependencyGraphProps {
    requirementId: string;
}
```

Features:
- Fetches from `GET /api/requirements/:id/dependencies/graph`
- Uses React Flow (`@xyflow/react`) with `ReactFlowProvider`
- Converts API response to React Flow nodes/edges:
  - Nodes: positioned using dagre layout (`@dagrejs/dagre`) with `rankdir: 'TB'`
  - Node styling: colored border by status (green/red/gray), current node highlighted with thicker border
  - Edges: default arrows from dependent → prerequisite
- `fitView` on load
- Clicking a node navigates to that requirement
- Wrap in a collapsible section with "View dependency graph" / "Hide" toggle

Dagre layout helper:
```typescript
import dagre from "@dagrejs/dagre";

function layoutGraph(nodes: Node[], edges: Edge[]) {
    const g = new dagre.graphlib.Graph();
    g.setDefaultEdgeLabel(() => ({}));
    g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 80 });

    nodes.forEach(node => g.setNode(node.id, { width: 200, height: 60 }));
    edges.forEach(edge => g.setEdge(edge.source, edge.target));

    dagre.layout(g);

    return nodes.map(node => {
        const pos = g.node(node.id);
        return { ...node, position: { x: pos.x - 100, y: pos.y - 30 } };
    });
}
```

- [ ] **Step 3: Add to detail page**

In `apps/web/src/routes/requirements_.$requirementId.tsx`:

Import the new components:
```typescript
import { DependencySection } from "@/features/requirements/dependency-section";
import { DependencyGraph } from "@/features/requirements/dependency-graph";
```

Add to the view mode JSX, below the steps section and above warnings:
```tsx
{/* Dependencies */}
<DependencySection requirementId={requirementId} editing={false} />
<DependencyGraph requirementId={requirementId} />
```

Also add `DependencySection` to the edit mode if desired (to add/remove deps while editing).

- [ ] **Step 4: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/requirements/dependency-section.tsx apps/web/src/features/requirements/dependency-graph.tsx apps/web/src/routes/requirements_.\$requirementId.tsx
git commit -m "feat: add dependency section and graph to requirement detail page"
```

---

### Task 7: Requirement ordering — backend

Add the `order` column support and reorder endpoint.

**Files:**
- Modify: `packages/db/src/repository/requirement.ts`
- Modify: `packages/api/src/requirements/service.ts`
- Modify: `packages/api/src/requirements/routes.ts`

- [ ] **Step 1: Update requirement repository**

In `packages/db/src/repository/requirement.ts`:

Add `asc` to the drizzle-orm import:
```typescript
import { and, asc, eq, ilike, inArray, or, sql } from "drizzle-orm";
```

Update `listRequirements` to add ordering. After the `.offset(params.offset)` line, add:
```typescript
.orderBy(asc(requirement.order), asc(requirement.createdAt))
```

Add two repository functions — one for fetching requirements by IDs (for validation), one for updating order:
```typescript
export function getRequirementsByIds(ids: string[], userId: string) {
    return Effect.tryPromise({
        try: () =>
            db.select({ id: requirement.id, useCaseId: requirement.useCaseId })
                .from(requirement)
                .where(and(inArray(requirement.id, ids), eq(requirement.userId, userId))),
        catch: cause => new DatabaseError({ cause })
    });
}

export function setRequirementOrder(requirementIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            for (let i = 0; i < requirementIds.length; i++) {
                await db
                    .update(requirement)
                    .set({ order: i })
                    .where(eq(requirement.id, requirementIds[i]!));
            }
            return requirementIds.length;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Add reorder service function**

In `packages/api/src/requirements/service.ts`:

```typescript
import {
    // ... existing imports
    getRequirementsByIds,
    setRequirementOrder
} from "@fubbik/db/repository";

export function reorderRequirements(requirementIds: string[], userId: string) {
    return Effect.gen(function* () {
        const existing = yield* getRequirementsByIds(requirementIds, userId);

        if (existing.length !== requirementIds.length) {
            return yield* Effect.fail(new ValidationError({ message: "Some requirements not found or not owned by user" }));
        }

        const useCaseIds = new Set(existing.map(r => r.useCaseId));
        if (useCaseIds.size > 1) {
            return yield* Effect.fail(new ValidationError({ message: "Requirements must belong to the same use case" }));
        }

        return yield* setRequirementOrder(requirementIds);
    });
}
```

Note: `ValidationError` should already be imported from `"../errors"` (used by other functions in this file).

- [ ] **Step 3: Add reorder route**

In `packages/api/src/requirements/routes.ts`, add the reorder endpoint. Chain it after the bulk endpoint and before the list endpoint (before any `/:id` routes):

```typescript
// Reorder
.patch(
    "/requirements/reorder",
    ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                const updated = yield* requirementService.reorderRequirements(ctx.body.requirementIds, session.user.id);
                return { updated };
            })
        ),
    {
        body: t.Object({
            requirementIds: t.Array(t.String(), { minItems: 1, maxItems: 200 })
        })
    }
)
```

- [ ] **Step 4: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/repository/requirement.ts packages/api/src/requirements/service.ts packages/api/src/requirements/routes.ts
git commit -m "feat: add requirement ordering and reorder API"
```

---

### Task 8: Requirement ordering — drag-and-drop frontend

Add @dnd-kit and integrate drag-and-drop on the list page.

**Files:**
- Create: `apps/web/src/features/requirements/sortable-requirement-list.tsx`
- Modify: `apps/web/src/routes/requirements.tsx`
- Modify: `apps/web/src/features/requirements/requirement-card.tsx`

- [ ] **Step 1: Install @dnd-kit**

```bash
cd apps/web && pnpm add @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
```

- [ ] **Step 2: Add drag handle to RequirementCard**

In `apps/web/src/features/requirements/requirement-card.tsx`:

Add `GripVertical` to the lucide-react import.

Add `dragHandleProps` to the component props:
```typescript
dragHandleProps?: React.HTMLAttributes<HTMLDivElement>;
```

Add a drag handle div before the checkbox:
```tsx
{dragHandleProps && (
    <div {...dragHandleProps} className="cursor-grab text-muted-foreground hover:text-foreground">
        <GripVertical className="size-4" />
    </div>
)}
```

- [ ] **Step 3: Create SortableRequirementList component**

Create `apps/web/src/features/requirements/sortable-requirement-list.tsx`:

```typescript
import { DndContext, closestCenter, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { RequirementCard } from "./requirement-card";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
```

The component wraps a list of requirements in a DndContext + SortableContext. Each item uses `useSortable` to get drag handle props, transform, and transition styles.

On drag end:
1. Compute new order from the reordered array
2. Optimistically update the local state
3. Call `PATCH /api/requirements/reorder` with the new ID order
4. Invalidate queries on success, revert on error

Props:
```typescript
interface SortableRequirementListProps {
    requirements: Array<Record<string, unknown>>;
    selectedIds: string[];
    onToggleSelection: (id: string, selected: boolean) => void;
    useCaseMap: Map<string, { name: string }>;
}
```

- [ ] **Step 4: Integrate into list page**

In `apps/web/src/routes/requirements.tsx`:

Replace the direct `.map()` of `RequirementCard` components with `SortableRequirementList` when requirements are grouped by use case or when viewing a single use case. Keep the flat card list for ungrouped/unfiltered views if drag-and-drop doesn't apply.

- [ ] **Step 5: Verify types**

Run: `pnpm run check-types`

- [ ] **Step 6: Commit**

```bash
git add apps/web/package.json pnpm-lock.yaml apps/web/src/features/requirements/sortable-requirement-list.tsx apps/web/src/features/requirements/requirement-card.tsx apps/web/src/routes/requirements.tsx
git commit -m "feat: add drag-and-drop requirement ordering"
```

---

### Task 9: Final verification

Verify everything works end-to-end. (Migration was already generated and applied in Task 1.)

- [ ] **Step 1: Run full type check**

Run: `pnpm run check-types`

- [ ] **Step 4: Run tests**

Run: `pnpm test`

- [ ] **Step 5: Manual verification checklist**

Run `pnpm dev` and verify:
- [ ] Sidebar shows use cases as a tree (parents with expandable children)
- [ ] Creating a sub use case works (via API or UI)
- [ ] Clicking a parent use case filters to its requirements + children's
- [ ] Requirement detail page shows dependency section
- [ ] Adding a dependency works (search, select, save)
- [ ] Removing a dependency works
- [ ] Circular dependency is rejected
- [ ] Dependency graph renders with dagre layout
- [ ] Clicking a graph node navigates to that requirement
- [ ] Drag-and-drop reordering works within a use case group
- [ ] Reorder persists on page reload

- [ ] **Step 6: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: final cleanup for requirements structure feature"
```
