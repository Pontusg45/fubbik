# Plans as a Central Entity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite `Plan` as the central unit of work in fubbik — a single entity holding description, linked requirements, structured analyze fields (chunks, files, risks, assumptions, questions), and enriched tasks — while deleting the entire `implementation_session` subsystem.

**Architecture:** Clean rewrite of six new DB tables (`plan`, `plan_requirement`, `plan_analyze_item`, `plan_task`, `plan_task_chunk`, `plan_task_dependency`), backend API, MCP tools, CLI commands, and the `/plans` web pages. All seven old plan/session tables are dropped in a single Drizzle migration. Web navigation swaps `Requirements` out of the primary nav in favor of `Plans`.

**Tech Stack:** Drizzle ORM (PostgreSQL), Elysia + Effect (backend), Model Context Protocol SDK, Commander.js (CLI), TanStack Start + TanStack Query (web), shadcn-ui on base-ui, Tailwind CSS, Vitest.

**Spec:** `docs/superpowers/specs/2026-04-11-plans-as-central-entity-design.md`

---

## File Structure

### Created

| Path | Responsibility |
|---|---|
| `packages/db/src/migrations/NNNN_plans_rewrite.sql` | Drop old tables + create six new tables |
| `packages/api/src/plans/requirements.ts` | Service for plan→requirement linking |
| `packages/api/src/plans/analyze.ts` | Service for analyze item CRUD |
| `packages/api/src/plans/tasks.ts` | Service for task CRUD + auto-unblock |
| `apps/web/src/features/plans/plan-detail-header.tsx` | Sticky detail page header |
| `apps/web/src/features/plans/plan-description-section.tsx` | Section 1 |
| `apps/web/src/features/plans/plan-requirements-section.tsx` | Section 2 |
| `apps/web/src/features/plans/plan-analyze-section.tsx` | Section 3 (+ 5 sub-components) |
| `apps/web/src/features/plans/plan-analyze-chunks.tsx` | Analyze sub-section |
| `apps/web/src/features/plans/plan-analyze-files.tsx` | Analyze sub-section |
| `apps/web/src/features/plans/plan-analyze-risks.tsx` | Analyze sub-section |
| `apps/web/src/features/plans/plan-analyze-assumptions.tsx` | Analyze sub-section |
| `apps/web/src/features/plans/plan-analyze-questions.tsx` | Analyze sub-section |
| `apps/web/src/features/plans/plan-tasks-section.tsx` | Section 4 |
| `apps/web/src/features/plans/plan-task-card.tsx` | Single task card |
| `apps/web/src/features/plans/plan-detail-right-rail.tsx` | Summary rail (desktop) |
| `apps/web/src/features/plans/plan-status-pill.tsx` | Status pill (reused across pages) |

### Rewritten (full replacement)

| Path | Responsibility |
|---|---|
| `packages/db/src/schema/plan.ts` | New six-table schema |
| `packages/db/src/repository/plan.ts` | Effect-based data access |
| `packages/api/src/plans/service.ts` | Plan CRUD + status transitions |
| `packages/api/src/plans/routes.ts` | Elysia route definitions |
| `packages/mcp/src/plan-tools.ts` | MCP tool registrations |
| `apps/cli/src/commands/plan.ts` | CLI commands |
| `apps/web/src/routes/plans.index.tsx` | Plans list page |
| `apps/web/src/routes/plans.new.tsx` | Plan creation page |
| `apps/web/src/routes/plans.$planId.tsx` | Plan detail page |
| `packages/db/src/seed.ts` | Seed data (plans section) |

### Modified

| Path | Responsibility |
|---|---|
| `packages/db/src/schema/index.ts` | Drop session exports, update plan exports |
| `packages/api/src/index.ts` | Unmount session routes |
| `packages/mcp/src/index.ts` | Unregister session tools |
| `apps/cli/src/index.ts` | (if session commands exist) Remove session command registration |
| `apps/web/src/routes/__root.tsx` | Nav swap, Manage dropdown update |
| `CLAUDE.md` | Update Plans + API Endpoints sections, remove session references |

### Deleted

| Path | Reason |
|---|---|
| `packages/db/src/schema/implementation-session.ts` | Sessions removed |
| `packages/db/src/repository/implementation-session.ts` | Sessions removed |
| `packages/api/src/sessions/routes.ts` | Sessions removed |
| `packages/api/src/sessions/service.ts` | Sessions removed |
| `packages/api/src/sessions/brief-generator.ts` | Sessions removed |
| `packages/api/src/plans/generate-from-requirements.ts` | Templated generation parked |
| `packages/api/src/plans/generate-from-requirements.test.ts` | Templated generation parked |
| `packages/api/src/plans/parse-plan-markdown.ts` | Markdown import parked |
| `packages/api/src/plans/parse-plan-markdown.test.ts` | Markdown import parked |
| `packages/mcp/src/session-tools.ts` | Sessions removed |
| `apps/web/src/routes/reviews.tsx` | Sessions removed |
| `apps/web/src/routes/reviews_.queue.tsx` | Sessions removed |
| `apps/web/src/routes/reviews_.$sessionId.tsx` | Sessions removed |
| `apps/web/src/features/reviews/assumption-resolver.tsx` | Sessions removed |
| `apps/web/src/features/reviews/review-queue-content.tsx` | Sessions removed |
| `apps/web/src/features/reviews/session-card.tsx` | Sessions removed |
| `apps/web/src/features/plans/plan-progress-bar.tsx` | Replaced by new component |
| `apps/web/src/features/plans/plan-step-item.tsx` | Steps → tasks |
| `apps/web/src/features/plans/plan-timeline.tsx` | Not in new detail page design |
| `apps/web/src/features/plans/plans-list-content.tsx` | Replaced by new list |

---

## Task Overview

1. **Schema & migration** — drop old, create new
2. **Repository rewrite** — Effect-based data access
3. **Plan CRUD service + routes**
4. **Requirements link service + routes**
5. **Analyze service + routes**
6. **Tasks service + routes (+ auto-unblock test)**
7. **Delete sessions from API**
8. **Rewrite MCP plan tools + delete session tools**
9. **Update CLI plan commands**
10. **Nav swap + delete /reviews web routes + delete old plans components**
11. **Plans list page**
12. **Plan create page**
13. **Plan detail — shell + sticky header + description**
14. **Plan detail — requirements section**
15. **Plan detail — analyze section**
16. **Plan detail — tasks section**
17. **Plan detail — right rail + polish**
18. **Seed script rewrite**
19. **CLAUDE.md update + final verification**

---

## Task 1: Schema Rewrite + Migration

**Files:**
- Rewrite: `packages/db/src/schema/plan.ts`
- Delete: `packages/db/src/schema/implementation-session.ts`
- Modify: `packages/db/src/schema/index.ts` (drop session exports)
- Create: `packages/db/src/migrations/NNNN_plans_rewrite.sql` (generated)

- [ ] **Step 1: Read the current schema files to understand existing imports and patterns**

Run: `cat packages/db/src/schema/plan.ts` and `cat packages/db/src/schema/implementation-session.ts` and `cat packages/db/src/schema/index.ts` — take note of imported helpers (`pgTable`, `text`, `uuid`, `timestamp`, etc.), foreign-key patterns, and existing `chunk` / `requirement` / `user` / `codebase` table imports.

- [ ] **Step 2: Rewrite `packages/db/src/schema/plan.ts`**

Replace the file entirely with:

```typescript
import { relations } from "drizzle-orm";
import { integer, jsonb, pgTable, text, timestamp, uniqueIndex, uuid } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { requirement } from "./requirement";
import { user } from "./auth";

/**
 * Plan: the central unit of work. Holds description, linked requirements,
 * structured analyze fields, and enriched tasks.
 */
export const plan = pgTable("plan", {
    id: uuid("id").primaryKey().defaultRandom(),
    title: text("title").notNull(),
    description: text("description"),
    status: text("status").notNull().default("draft"),
    // draft | analyzing | ready | in_progress | completed | archived — labels, ungated
    userId: text("user_id")
        .notNull()
        .references(() => user.id, { onDelete: "cascade" }),
    codebaseId: uuid("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
    completedAt: timestamp("completed_at", { withTimezone: true }),
});

export const planRequirement = pgTable(
    "plan_requirement",
    {
        id: uuid("id").primaryKey().defaultRandom(),
        planId: uuid("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        requirementId: uuid("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    },
    t => ({
        planRequirementUnique: uniqueIndex("plan_requirement_unique_idx").on(t.planId, t.requirementId),
    }),
);

/**
 * plan_analyze_item: one discriminated table holding all five analyze kinds
 * (chunk, file, risk, assumption, question). See spec Section 1 for metadata shapes.
 */
export const planAnalyzeItem = pgTable("plan_analyze_item", {
    id: uuid("id").primaryKey().defaultRandom(),
    planId: uuid("plan_id")
        .notNull()
        .references(() => plan.id, { onDelete: "cascade" }),
    kind: text("kind").notNull(), // chunk | file | risk | assumption | question
    order: integer("order").notNull().default(0),
    chunkId: uuid("chunk_id").references(() => chunk.id, { onDelete: "cascade" }),
    filePath: text("file_path"),
    text: text("text"),
    metadata: jsonb("metadata").notNull().default({}),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
});

export const planTask = pgTable("plan_task", {
    id: uuid("id").primaryKey().defaultRandom(),
    planId: uuid("plan_id")
        .notNull()
        .references(() => plan.id, { onDelete: "cascade" }),
    title: text("title").notNull(),
    description: text("description"),
    acceptanceCriteria: jsonb("acceptance_criteria").notNull().default([]),
    status: text("status").notNull().default("pending"),
    // pending | in_progress | done | skipped | blocked
    order: integer("order").notNull().default(0),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
});

export const planTaskChunk = pgTable(
    "plan_task_chunk",
    {
        id: uuid("id").primaryKey().defaultRandom(),
        taskId: uuid("task_id")
            .notNull()
            .references(() => planTask.id, { onDelete: "cascade" }),
        chunkId: uuid("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        relation: text("relation").notNull(), // context | created | modified
        createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    },
    t => ({
        taskChunkUnique: uniqueIndex("plan_task_chunk_unique_idx").on(t.taskId, t.chunkId, t.relation),
    }),
);

export const planTaskDependency = pgTable(
    "plan_task_dependency",
    {
        id: uuid("id").primaryKey().defaultRandom(),
        taskId: uuid("task_id")
            .notNull()
            .references(() => planTask.id, { onDelete: "cascade" }),
        dependsOnTaskId: uuid("depends_on_task_id")
            .notNull()
            .references(() => planTask.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    },
    t => ({
        taskDepUnique: uniqueIndex("plan_task_dependency_unique_idx").on(t.taskId, t.dependsOnTaskId),
    }),
);

// Relations
export const planRelations = relations(plan, ({ many, one }) => ({
    requirements: many(planRequirement),
    analyzeItems: many(planAnalyzeItem),
    tasks: many(planTask),
    codebase: one(codebase, { fields: [plan.codebaseId], references: [codebase.id] }),
    user: one(user, { fields: [plan.userId], references: [user.id] }),
}));

export const planRequirementRelations = relations(planRequirement, ({ one }) => ({
    plan: one(plan, { fields: [planRequirement.planId], references: [plan.id] }),
    requirement: one(requirement, { fields: [planRequirement.requirementId], references: [requirement.id] }),
}));

export const planAnalyzeItemRelations = relations(planAnalyzeItem, ({ one }) => ({
    plan: one(plan, { fields: [planAnalyzeItem.planId], references: [plan.id] }),
    chunk: one(chunk, { fields: [planAnalyzeItem.chunkId], references: [chunk.id] }),
}));

export const planTaskRelations = relations(planTask, ({ many, one }) => ({
    plan: one(plan, { fields: [planTask.planId], references: [plan.id] }),
    chunks: many(planTaskChunk),
}));

export const planTaskChunkRelations = relations(planTaskChunk, ({ one }) => ({
    task: one(planTask, { fields: [planTaskChunk.taskId], references: [planTask.id] }),
    chunk: one(chunk, { fields: [planTaskChunk.chunkId], references: [chunk.id] }),
}));

// Inferred types
export type Plan = typeof plan.$inferSelect;
export type NewPlan = typeof plan.$inferInsert;
export type PlanRequirement = typeof planRequirement.$inferSelect;
export type PlanAnalyzeItem = typeof planAnalyzeItem.$inferSelect;
export type NewPlanAnalyzeItem = typeof planAnalyzeItem.$inferInsert;
export type PlanTask = typeof planTask.$inferSelect;
export type NewPlanTask = typeof planTask.$inferInsert;
export type PlanTaskChunk = typeof planTaskChunk.$inferSelect;
export type PlanTaskDependency = typeof planTaskDependency.$inferSelect;

export type PlanStatus = "draft" | "analyzing" | "ready" | "in_progress" | "completed" | "archived";
export type PlanTaskStatus = "pending" | "in_progress" | "done" | "skipped" | "blocked";
export type PlanAnalyzeKind = "chunk" | "file" | "risk" | "assumption" | "question";
export type PlanTaskChunkRelation = "context" | "created" | "modified";
```

- [ ] **Step 3: Delete the session schema file**

```bash
rm packages/db/src/schema/implementation-session.ts
```

- [ ] **Step 4: Update `packages/db/src/schema/index.ts`**

Read it first, then remove every line that exports from `./implementation-session`. Verify the `./plan` re-export still exists and now picks up the new exports automatically (same file path). Add `planRequirement`, `planAnalyzeItem`, `planTask`, `planTaskChunk`, `planTaskDependency` to the export list if it uses named re-exports.

Example, if the file currently reads:

```typescript
export * from "./plan";
export * from "./implementation-session";
```

Change to:

```typescript
export * from "./plan";
```

If it uses named exports, list every symbol exported from plan.ts above.

- [ ] **Step 5: Generate the Drizzle migration**

Run: `pnpm db:generate`

Expected: A new migration file appears under `packages/db/src/migrations/` with `DROP TABLE` statements for all old tables and `CREATE TABLE` statements for the new ones. If Drizzle's generated diff is missing the `DROP` statements (because it doesn't see the old session schema file any more), manually edit the migration file to prepend these statements in order at the top:

```sql
DROP TABLE IF EXISTS "session_requirement_ref" CASCADE;
DROP TABLE IF EXISTS "session_assumption" CASCADE;
DROP TABLE IF EXISTS "session_chunk_ref" CASCADE;
DROP TABLE IF EXISTS "implementation_session" CASCADE;
DROP TABLE IF EXISTS "plan_chunk_ref" CASCADE;
DROP TABLE IF EXISTS "plan_step" CASCADE;
DROP TABLE IF EXISTS "plan" CASCADE;
```

Note: `plan` is listed last among drops because `plan_step` and `plan_chunk_ref` reference it. Listing the drops in this order (child-then-parent) works even without CASCADE, but `CASCADE` covers residual fk constraints.

- [ ] **Step 6: Apply the migration**

Run: `pnpm db:push`

Expected: No errors. If you see "relation X already exists," the migration has issues — revert and regenerate.

Verify the new shape:

```bash
psql "$DATABASE_URL" -c "\dt plan*"
```

Expected output includes: `plan`, `plan_requirement`, `plan_analyze_item`, `plan_task`, `plan_task_chunk`, `plan_task_dependency`. No `implementation_session`, `session_*`, `plan_step`, or `plan_chunk_ref`.

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/schema/plan.ts packages/db/src/schema/index.ts packages/db/src/migrations/
git commit -m "feat(db): rewrite plan schema, drop sessions and old plan tables

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Rewrite Plan Repository

**Files:**
- Rewrite: `packages/db/src/repository/plan.ts`
- Delete: `packages/db/src/repository/implementation-session.ts`

**Context:** The repository layer returns `Effect<T, DatabaseError>`. Services compose these Effects. The existing `DatabaseError` type and the pattern of `Effect.tryPromise` with tagged errors already exist in the codebase — follow the shape used by `packages/db/src/repository/chunk.ts` (read it first for reference).

- [ ] **Step 1: Read the reference repository to match the existing pattern**

Run: `cat packages/db/src/repository/chunk.ts | head -100`

Take note of: how `DatabaseError` is imported, how `Effect.tryPromise` is used, how the `db` instance is imported, how to return `Effect.succeed` vs `Effect.fail`.

- [ ] **Step 2: Rewrite `packages/db/src/repository/plan.ts`**

```typescript
import { and, asc, eq, inArray, ne } from "drizzle-orm";
import { Effect } from "effect";

import { db } from "../client";
import { DatabaseError } from "../errors";
import {
    plan,
    planAnalyzeItem,
    planRequirement,
    planTask,
    planTaskChunk,
    planTaskDependency,
    type NewPlan,
    type NewPlanAnalyzeItem,
    type NewPlanTask,
    type Plan,
    type PlanAnalyzeItem,
    type PlanAnalyzeKind,
    type PlanRequirement,
    type PlanStatus,
    type PlanTask,
    type PlanTaskChunk,
    type PlanTaskChunkRelation,
    type PlanTaskDependency,
    type PlanTaskStatus,
} from "../schema/plan";

// --- Plan CRUD ---

export interface ListPlansFilter {
    userId: string;
    codebaseId?: string;
    status?: PlanStatus;
    requirementId?: string;
    includeArchived?: boolean;
}

export function listPlans(filter: ListPlansFilter): Effect.Effect<Plan[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.userId, filter.userId)];
            if (filter.codebaseId) conditions.push(eq(plan.codebaseId, filter.codebaseId));
            if (filter.status) conditions.push(eq(plan.status, filter.status));
            if (!filter.includeArchived && !filter.status) {
                conditions.push(ne(plan.status, "archived"));
            }
            let rows: Plan[];
            if (filter.requirementId) {
                const reqPlanIds = await db
                    .select({ planId: planRequirement.planId })
                    .from(planRequirement)
                    .where(eq(planRequirement.requirementId, filter.requirementId));
                const ids = reqPlanIds.map(r => r.planId);
                if (ids.length === 0) return [];
                conditions.push(inArray(plan.id, ids));
            }
            rows = await db.select().from(plan).where(and(...conditions)).orderBy(asc(plan.createdAt));
            return rows;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to list plans" }),
    });
}

export function getPlan(id: string): Effect.Effect<Plan | null, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(plan).where(eq(plan.id, id)).limit(1);
            return row ?? null;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to get plan" }),
    });
}

export function createPlan(input: NewPlan): Effect.Effect<Plan, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(plan).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to create plan" }),
    });
}

export function updatePlan(id: string, patch: Partial<NewPlan>): Effect.Effect<Plan, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(plan)
                .set({ ...patch, updatedAt: new Date() })
                .where(eq(plan.id, id))
                .returning();
            if (!row) throw new Error("Plan not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to update plan" }),
    });
}

export function deletePlan(id: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(plan).where(eq(plan.id, id));
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to delete plan" }),
    });
}

// --- Requirements links ---

export function listPlanRequirements(planId: string): Effect.Effect<PlanRequirement[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planRequirement)
                .where(eq(planRequirement.planId, planId))
                .orderBy(asc(planRequirement.order)),
        catch: e => new DatabaseError({ cause: e, message: "Failed to list plan requirements" }),
    });
}

export function addPlanRequirement(planId: string, requirementId: string): Effect.Effect<PlanRequirement, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [maxRow] = await db
                .select({ order: planRequirement.order })
                .from(planRequirement)
                .where(eq(planRequirement.planId, planId))
                .orderBy(asc(planRequirement.order));
            const nextOrder = maxRow ? maxRow.order + 1 : 0;
            const [row] = await db
                .insert(planRequirement)
                .values({ planId, requirementId, order: nextOrder })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to add plan requirement" }),
    });
}

export function removePlanRequirement(planId: string, requirementId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db
                .delete(planRequirement)
                .where(and(eq(planRequirement.planId, planId), eq(planRequirement.requirementId, requirementId)));
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to remove plan requirement" }),
    });
}

export function reorderPlanRequirements(planId: string, requirementIds: string[]): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (let i = 0; i < requirementIds.length; i++) {
                    const rid = requirementIds[i];
                    if (!rid) continue;
                    await tx
                        .update(planRequirement)
                        .set({ order: i })
                        .where(and(eq(planRequirement.planId, planId), eq(planRequirement.requirementId, rid)));
                }
            });
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to reorder plan requirements" }),
    });
}

// --- Analyze items ---

export function listAnalyzeItems(planId: string): Effect.Effect<PlanAnalyzeItem[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planAnalyzeItem)
                .where(eq(planAnalyzeItem.planId, planId))
                .orderBy(asc(planAnalyzeItem.kind), asc(planAnalyzeItem.order)),
        catch: e => new DatabaseError({ cause: e, message: "Failed to list analyze items" }),
    });
}

export function createAnalyzeItem(input: NewPlanAnalyzeItem): Effect.Effect<PlanAnalyzeItem, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [maxRow] = await db
                .select({ order: planAnalyzeItem.order })
                .from(planAnalyzeItem)
                .where(and(eq(planAnalyzeItem.planId, input.planId), eq(planAnalyzeItem.kind, input.kind)))
                .orderBy(asc(planAnalyzeItem.order));
            const nextOrder = maxRow ? maxRow.order + 1 : 0;
            const [row] = await db
                .insert(planAnalyzeItem)
                .values({ ...input, order: nextOrder })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to create analyze item" }),
    });
}

export function updateAnalyzeItem(
    itemId: string,
    patch: Partial<NewPlanAnalyzeItem>,
): Effect.Effect<PlanAnalyzeItem, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(planAnalyzeItem)
                .set({ ...patch, updatedAt: new Date() })
                .where(eq(planAnalyzeItem.id, itemId))
                .returning();
            if (!row) throw new Error("Analyze item not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to update analyze item" }),
    });
}

export function deleteAnalyzeItem(itemId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planAnalyzeItem).where(eq(planAnalyzeItem.id, itemId));
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to delete analyze item" }),
    });
}

export function reorderAnalyzeItems(
    planId: string,
    kind: PlanAnalyzeKind,
    itemIds: string[],
): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (let i = 0; i < itemIds.length; i++) {
                    const id = itemIds[i];
                    if (!id) continue;
                    await tx
                        .update(planAnalyzeItem)
                        .set({ order: i })
                        .where(
                            and(
                                eq(planAnalyzeItem.id, id),
                                eq(planAnalyzeItem.planId, planId),
                                eq(planAnalyzeItem.kind, kind),
                            ),
                        );
                }
            });
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to reorder analyze items" }),
    });
}

// --- Tasks ---

export function listTasks(planId: string): Effect.Effect<PlanTask[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select()
                .from(planTask)
                .where(eq(planTask.planId, planId))
                .orderBy(asc(planTask.order)),
        catch: e => new DatabaseError({ cause: e, message: "Failed to list tasks" }),
    });
}

export function createTask(input: NewPlanTask): Effect.Effect<PlanTask, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [maxRow] = await db
                .select({ order: planTask.order })
                .from(planTask)
                .where(eq(planTask.planId, input.planId))
                .orderBy(asc(planTask.order));
            const nextOrder = maxRow ? maxRow.order + 1 : 0;
            const [row] = await db
                .insert(planTask)
                .values({ ...input, order: nextOrder })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to create task" }),
    });
}

export function updateTask(taskId: string, patch: Partial<NewPlanTask>): Effect.Effect<PlanTask, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(planTask)
                .set({ ...patch, updatedAt: new Date() })
                .where(eq(planTask.id, taskId))
                .returning();
            if (!row) throw new Error("Task not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to update task" }),
    });
}

export function deleteTask(taskId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTask).where(eq(planTask.id, taskId));
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to delete task" }),
    });
}

export function reorderTasks(planId: string, taskIds: string[]): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.transaction(async tx => {
                for (let i = 0; i < taskIds.length; i++) {
                    const id = taskIds[i];
                    if (!id) continue;
                    await tx
                        .update(planTask)
                        .set({ order: i })
                        .where(and(eq(planTask.id, id), eq(planTask.planId, planId)));
                }
            });
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to reorder tasks" }),
    });
}

// --- Task chunk links ---

export function listTaskChunks(taskId: string): Effect.Effect<PlanTaskChunk[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => db.select().from(planTaskChunk).where(eq(planTaskChunk.taskId, taskId)),
        catch: e => new DatabaseError({ cause: e, message: "Failed to list task chunks" }),
    });
}

export function addTaskChunk(
    taskId: string,
    chunkId: string,
    relation: PlanTaskChunkRelation,
): Effect.Effect<PlanTaskChunk, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(planTaskChunk).values({ taskId, chunkId, relation }).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to add task chunk" }),
    });
}

export function removeTaskChunk(linkId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTaskChunk).where(eq(planTaskChunk.id, linkId));
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to remove task chunk" }),
    });
}

// --- Task dependencies ---

export function listTaskDependencies(planId: string): Effect.Effect<PlanTaskDependency[], DatabaseError> {
    return Effect.tryPromise({
        try: async () =>
            db
                .select({
                    id: planTaskDependency.id,
                    taskId: planTaskDependency.taskId,
                    dependsOnTaskId: planTaskDependency.dependsOnTaskId,
                    createdAt: planTaskDependency.createdAt,
                })
                .from(planTaskDependency)
                .innerJoin(planTask, eq(planTask.id, planTaskDependency.taskId))
                .where(eq(planTask.planId, planId)),
        catch: e => new DatabaseError({ cause: e, message: "Failed to list task dependencies" }),
    });
}

export function addTaskDependency(
    taskId: string,
    dependsOnTaskId: string,
): Effect.Effect<PlanTaskDependency, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .insert(planTaskDependency)
                .values({ taskId, dependsOnTaskId })
                .returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to add task dependency" }),
    });
}

export function removeTaskDependency(depId: string): Effect.Effect<void, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(planTaskDependency).where(eq(planTaskDependency.id, depId));
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to remove task dependency" }),
    });
}

/**
 * When a task marks done, any tasks that depend on it and are currently
 * `blocked` should flip to `pending`. Returns the IDs of unblocked tasks.
 */
export function unblockDependentsOf(taskId: string): Effect.Effect<string[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const deps = await db
                .select({ dependent: planTaskDependency.taskId })
                .from(planTaskDependency)
                .where(eq(planTaskDependency.dependsOnTaskId, taskId));
            const dependentIds = deps.map(d => d.dependent);
            if (dependentIds.length === 0) return [];
            const result = await db
                .update(planTask)
                .set({ status: "pending", updatedAt: new Date() })
                .where(and(inArray(planTask.id, dependentIds), eq(planTask.status, "blocked")))
                .returning({ id: planTask.id });
            return result.map(r => r.id);
        },
        catch: e => new DatabaseError({ cause: e, message: "Failed to unblock dependents" }),
    });
}

export type PlanTaskStatusType = PlanTaskStatus;
```

- [ ] **Step 3: Delete the session repository**

```bash
rm packages/db/src/repository/implementation-session.ts
```

- [ ] **Step 4: Find any callers of the deleted repository symbols**

Run: `grep -r "implementation-session" packages/ apps/ --include="*.ts" --include="*.tsx"`

Expected: several hits in API and MCP files. Note them — those files will be fixed in later tasks. For now, they'll be broken imports.

- [ ] **Step 5: Type check only the db package**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | tail -30`

Expected: the db package type-checks cleanly. (The broken imports live in other packages and will be fixed in their respective tasks.)

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/plan.ts packages/db/src/repository/implementation-session.ts
git commit -m "feat(db): rewrite plan repository, delete implementation-session repository

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Plan CRUD Service + Routes

**Files:**
- Rewrite: `packages/api/src/plans/service.ts`
- Rewrite: `packages/api/src/plans/routes.ts`
- Delete: `packages/api/src/plans/generate-from-requirements.ts`
- Delete: `packages/api/src/plans/generate-from-requirements.test.ts`
- Delete: `packages/api/src/plans/parse-plan-markdown.ts`
- Delete: `packages/api/src/plans/parse-plan-markdown.test.ts`

**Context:** Plan routes live in Elysia. Services return Effects. The global `.onError` handler in `packages/api/src/index.ts` maps Effect tagged errors to HTTP status codes. Read `packages/api/src/chunks/routes.ts` or similar for the `Effect.runPromise(requireSession(ctx).pipe(...))` pattern before writing.

- [ ] **Step 1: Read the reference routes file**

Run: `cat packages/api/src/chunks/routes.ts | head -80`

Note: `requireSession`, `t.Object` for body/query validation, `.pipe(...)` chain, `Effect.runPromise`.

- [ ] **Step 2: Delete the parked files**

```bash
rm packages/api/src/plans/generate-from-requirements.ts \
   packages/api/src/plans/generate-from-requirements.test.ts \
   packages/api/src/plans/parse-plan-markdown.ts \
   packages/api/src/plans/parse-plan-markdown.test.ts
```

- [ ] **Step 3: Rewrite `packages/api/src/plans/service.ts`**

```typescript
import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";
import type {
    Plan,
    PlanStatus,
    PlanAnalyzeKind,
    PlanTaskChunkRelation,
    NewPlanTask,
} from "@fubbik/db/schema/plan";

import { NotFoundError, ValidationError } from "../errors";

const VALID_STATUSES: PlanStatus[] = ["draft", "analyzing", "ready", "in_progress", "completed", "archived"];
const VALID_ANALYZE_KINDS: PlanAnalyzeKind[] = ["chunk", "file", "risk", "assumption", "question"];
const VALID_TASK_RELATIONS: PlanTaskChunkRelation[] = ["context", "created", "modified"];

export interface CreatePlanInput {
    title: string;
    description?: string;
    codebaseId?: string;
    requirementIds?: string[];
    tasks?: Array<{ title: string; description?: string; acceptanceCriteria?: string[] }>;
}

export interface ListPlansInput {
    userId: string;
    codebaseId?: string;
    status?: string;
    requirementId?: string;
    includeArchived?: boolean;
}

export function listPlans(input: ListPlansInput) {
    if (input.status && !VALID_STATUSES.includes(input.status as PlanStatus)) {
        return Effect.fail(new ValidationError({ message: `Invalid status: ${input.status}` }));
    }
    return planRepo.listPlans({
        userId: input.userId,
        codebaseId: input.codebaseId,
        status: input.status as PlanStatus | undefined,
        requirementId: input.requirementId,
        includeArchived: input.includeArchived,
    });
}

export function getPlan(id: string) {
    return planRepo.getPlan(id).pipe(
        Effect.flatMap(plan =>
            plan ? Effect.succeed(plan) : Effect.fail(new NotFoundError({ message: `Plan ${id} not found` })),
        ),
    );
}

/**
 * Full plan detail including requirements, analyze items grouped by kind,
 * tasks, task-chunk links, and dependencies.
 */
export function getPlanDetail(id: string) {
    return Effect.gen(function* () {
        const plan = yield* getPlan(id);
        const requirements = yield* planRepo.listPlanRequirements(id);
        const analyzeItems = yield* planRepo.listAnalyzeItems(id);
        const tasks = yield* planRepo.listTasks(id);
        const dependencies = yield* planRepo.listTaskDependencies(id);

        // Group analyze items by kind
        const analyze: Record<PlanAnalyzeKind, typeof analyzeItems> = {
            chunk: [],
            file: [],
            risk: [],
            assumption: [],
            question: [],
        };
        for (const item of analyzeItems) {
            if (VALID_ANALYZE_KINDS.includes(item.kind as PlanAnalyzeKind)) {
                analyze[item.kind as PlanAnalyzeKind].push(item);
            }
        }

        // Fetch chunk links for each task in parallel
        const taskChunks = yield* Effect.all(
            tasks.map(t => planRepo.listTaskChunks(t.id)),
        );
        const tasksWithChunks = tasks.map((t, i) => ({ ...t, chunks: taskChunks[i] ?? [] }));

        return { plan, requirements, analyze, tasks: tasksWithChunks, dependencies };
    });
}

export function createPlan(userId: string, input: CreatePlanInput) {
    return Effect.gen(function* () {
        if (!input.title.trim()) {
            return yield* Effect.fail(new ValidationError({ message: "Title is required" }));
        }
        const created = yield* planRepo.createPlan({
            title: input.title.trim(),
            description: input.description ?? null,
            codebaseId: input.codebaseId ?? null,
            userId,
            status: "draft",
        });
        if (input.requirementIds) {
            for (const rid of input.requirementIds) {
                yield* planRepo.addPlanRequirement(created.id, rid);
            }
        }
        if (input.tasks) {
            for (const t of input.tasks) {
                yield* planRepo.createTask({
                    planId: created.id,
                    title: t.title,
                    description: t.description ?? null,
                    acceptanceCriteria: t.acceptanceCriteria ?? [],
                    status: "pending",
                });
            }
        }
        return created;
    });
}

export interface UpdatePlanInput {
    title?: string;
    description?: string | null;
    status?: string;
    codebaseId?: string | null;
}

export function updatePlan(id: string, input: UpdatePlanInput) {
    return Effect.gen(function* () {
        if (input.status && !VALID_STATUSES.includes(input.status as PlanStatus)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid status: ${input.status}` }));
        }
        const existing = yield* getPlan(id);
        const patch: Parameters<typeof planRepo.updatePlan>[1] = {};
        if (input.title !== undefined) patch.title = input.title;
        if (input.description !== undefined) patch.description = input.description;
        if (input.codebaseId !== undefined) patch.codebaseId = input.codebaseId;
        if (input.status !== undefined) {
            patch.status = input.status as PlanStatus;
            // Side effect: set/clear completedAt
            if (input.status === "completed" && existing.status !== "completed") {
                patch.completedAt = new Date();
            } else if (input.status !== "completed" && existing.status === "completed") {
                patch.completedAt = null;
            }
        }
        return yield* planRepo.updatePlan(id, patch);
    });
}

export function deletePlan(id: string) {
    return Effect.gen(function* () {
        yield* getPlan(id); // Ensures it exists (NotFoundError propagates)
        yield* planRepo.deletePlan(id);
    });
}

export { VALID_STATUSES, VALID_ANALYZE_KINDS, VALID_TASK_RELATIONS };
```

- [ ] **Step 4: Rewrite `packages/api/src/plans/routes.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../auth/middleware";
import * as planService from "./service";

export const planRoutes = new Elysia({ prefix: "/api/plans" })
    .get(
        "/",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        planService.listPlans({
                            userId: session.user.id,
                            codebaseId: ctx.query.codebaseId,
                            status: ctx.query.status,
                            requirementId: ctx.query.requirementId,
                            includeArchived: ctx.query.includeArchived === "true",
                        }),
                    ),
                ),
            );
            return result;
        },
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                status: t.Optional(t.String()),
                requirementId: t.Optional(t.String()),
                includeArchived: t.Optional(t.String()),
            }),
        },
    )
    .get("/:id", async ctx => {
        const result = await Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(() => planService.getPlanDetail(ctx.params.id))),
        );
        return result;
    })
    .post(
        "/",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => planService.createPlan(session.user.id, ctx.body)),
                ),
            );
            return result;
        },
        {
            body: t.Object({
                title: t.String(),
                description: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                requirementIds: t.Optional(t.Array(t.String())),
                tasks: t.Optional(
                    t.Array(
                        t.Object({
                            title: t.String(),
                            description: t.Optional(t.String()),
                            acceptanceCriteria: t.Optional(t.Array(t.String())),
                        }),
                    ),
                ),
            }),
        },
    )
    .patch(
        "/:id",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(() => planService.updatePlan(ctx.params.id, ctx.body))),
            );
            return result;
        },
        {
            body: t.Object({
                title: t.Optional(t.String()),
                description: t.Optional(t.Union([t.String(), t.Null()])),
                status: t.Optional(t.String()),
                codebaseId: t.Optional(t.Union([t.String(), t.Null()])),
            }),
        },
    )
    .delete("/:id", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(() => planService.deletePlan(ctx.params.id))),
        );
        return { ok: true };
    });
```

- [ ] **Step 5: Type check the API package**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "plans/(service|routes)" | head -20`

Expected: zero errors in plans/service.ts and plans/routes.ts. Errors may exist elsewhere (tasks, analyze, requirements routes aren't added yet, and sessions still exist).

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/plans/service.ts packages/api/src/plans/routes.ts packages/api/src/plans/generate-from-requirements.ts packages/api/src/plans/generate-from-requirements.test.ts packages/api/src/plans/parse-plan-markdown.ts packages/api/src/plans/parse-plan-markdown.test.ts
git commit -m "feat(api): rewrite plan CRUD service and routes, delete markdown/template helpers

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Requirements Link Routes

**Files:**
- Create: `packages/api/src/plans/requirements.ts`
- Modify: `packages/api/src/plans/routes.ts` (mount sub-routes)

- [ ] **Step 1: Create `packages/api/src/plans/requirements.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";

import { requireSession } from "../auth/middleware";
import { getPlan } from "./service";

export const planRequirementRoutes = new Elysia({ prefix: "/api/plans/:id/requirements" })
    .post(
        "/",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => planRepo.addPlanRequirement(ctx.params.id, ctx.body.requirementId)),
                ),
            );
            return result;
        },
        { body: t.Object({ requirementId: t.String() }) },
    )
    .delete("/:requirementId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() =>
                    planRepo.removePlanRequirement(ctx.params.id, ctx.params.requirementId),
                ),
            ),
        );
        return { ok: true };
    })
    .post(
        "/reorder",
        async ctx => {
            await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => planRepo.reorderPlanRequirements(ctx.params.id, ctx.body.requirementIds)),
                ),
            );
            return { ok: true };
        },
        { body: t.Object({ requirementIds: t.Array(t.String()) }) },
    );
```

- [ ] **Step 2: Mount in routes.ts**

Open `packages/api/src/plans/routes.ts` and add the import at the top:

```typescript
import { planRequirementRoutes } from "./requirements";
```

At the bottom of the file, change the final export to chain the sub-routes. Find the current export line (it should be `export const planRoutes = new Elysia(...)...` chain). Change the end of the chain to call `.use(planRequirementRoutes)`:

```typescript
export const planRoutes = new Elysia({ prefix: "/api/plans" })
    .get(...)
    // ... existing routes
    .delete("/:id", ...);

// Below the planRoutes definition:
export const planRoutesWithSubRoutes = new Elysia()
    .use(planRoutes)
    .use(planRequirementRoutes);
```

Actually, cleaner: update the root plan file to compose without renaming. Replace the file's export section with:

```typescript
const planBase = new Elysia({ prefix: "/api/plans" })
    .get(...) // keep the existing handlers
    // ...
    .delete("/:id", ...);

export const planRoutes = new Elysia()
    .use(planBase)
    .use(planRequirementRoutes);
```

This way `planRoutes` stays as the single exported symbol and the API root file doesn't change.

- [ ] **Step 3: Type check**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "plans/" | head -20`

Expected: zero errors in `plans/`.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/plans/requirements.ts packages/api/src/plans/routes.ts
git commit -m "feat(api): add plan requirement link routes

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Analyze Routes

**Files:**
- Create: `packages/api/src/plans/analyze.ts`
- Modify: `packages/api/src/plans/routes.ts` (mount)

- [ ] **Step 1: Create `packages/api/src/plans/analyze.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";
import type { PlanAnalyzeItem, PlanAnalyzeKind } from "@fubbik/db/schema/plan";

import { requireSession } from "../auth/middleware";
import { ValidationError } from "../errors";
import { VALID_ANALYZE_KINDS, getPlan } from "./service";

function groupByKind(items: PlanAnalyzeItem[]) {
    const grouped: Record<PlanAnalyzeKind, PlanAnalyzeItem[]> = {
        chunk: [],
        file: [],
        risk: [],
        assumption: [],
        question: [],
    };
    for (const item of items) {
        if (VALID_ANALYZE_KINDS.includes(item.kind as PlanAnalyzeKind)) {
            grouped[item.kind as PlanAnalyzeKind].push(item);
        }
    }
    return grouped;
}

function validateKind(kind: string): Effect.Effect<PlanAnalyzeKind, ValidationError> {
    if (!VALID_ANALYZE_KINDS.includes(kind as PlanAnalyzeKind)) {
        return Effect.fail(new ValidationError({ message: `Invalid analyze kind: ${kind}` }));
    }
    return Effect.succeed(kind as PlanAnalyzeKind);
}

export const planAnalyzeRoutes = new Elysia({ prefix: "/api/plans/:id/analyze" })
    .get("/", async ctx => {
        const result = await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.listAnalyzeItems(ctx.params.id)),
                Effect.map(groupByKind),
            ),
        );
        return result;
    })
    .post(
        "/",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => validateKind(ctx.body.kind)),
                    Effect.flatMap(kind =>
                        planRepo.createAnalyzeItem({
                            planId: ctx.params.id,
                            kind,
                            chunkId: ctx.body.chunkId ?? null,
                            filePath: ctx.body.filePath ?? null,
                            text: ctx.body.text ?? null,
                            metadata: ctx.body.metadata ?? {},
                        }),
                    ),
                ),
            );
            return result;
        },
        {
            body: t.Object({
                kind: t.String(),
                chunkId: t.Optional(t.String()),
                filePath: t.Optional(t.String()),
                text: t.Optional(t.String()),
                metadata: t.Optional(t.Any()),
            }),
        },
    )
    .patch(
        "/:itemId",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => planRepo.updateAnalyzeItem(ctx.params.itemId, ctx.body)),
                ),
            );
            return result;
        },
        {
            body: t.Object({
                text: t.Optional(t.String()),
                metadata: t.Optional(t.Any()),
                chunkId: t.Optional(t.String()),
                filePath: t.Optional(t.String()),
            }),
        },
    )
    .delete("/:itemId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.deleteAnalyzeItem(ctx.params.itemId)),
            ),
        );
        return { ok: true };
    })
    .post(
        "/reorder",
        async ctx => {
            await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => validateKind(ctx.body.kind)),
                    Effect.flatMap(kind =>
                        planRepo.reorderAnalyzeItems(ctx.params.id, kind, ctx.body.itemIds),
                    ),
                ),
            );
            return { ok: true };
        },
        { body: t.Object({ kind: t.String(), itemIds: t.Array(t.String()) }) },
    );
```

- [ ] **Step 2: Mount in routes.ts**

In `packages/api/src/plans/routes.ts`, add import:

```typescript
import { planAnalyzeRoutes } from "./analyze";
```

Update the final composed export to include it:

```typescript
export const planRoutes = new Elysia()
    .use(planBase)
    .use(planRequirementRoutes)
    .use(planAnalyzeRoutes);
```

- [ ] **Step 3: Type check**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "plans/" | head -20`

Expected: zero errors in plans/.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/plans/analyze.ts packages/api/src/plans/routes.ts
git commit -m "feat(api): add plan analyze item routes

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Tasks Routes + Auto-unblock Unit Test

**Files:**
- Create: `packages/api/src/plans/tasks.ts`
- Modify: `packages/api/src/plans/routes.ts` (mount)
- Create: `packages/db/src/__tests__/plan-unblock.test.ts`

- [ ] **Step 1: Create `packages/api/src/plans/tasks.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";
import type { PlanTaskChunkRelation, PlanTaskStatus } from "@fubbik/db/schema/plan";

import { requireSession } from "../auth/middleware";
import { ValidationError } from "../errors";
import { VALID_TASK_RELATIONS, getPlan } from "./service";

const VALID_TASK_STATUSES: PlanTaskStatus[] = ["pending", "in_progress", "done", "skipped", "blocked"];

function validateStatus(status: string): Effect.Effect<PlanTaskStatus, ValidationError> {
    if (!VALID_TASK_STATUSES.includes(status as PlanTaskStatus)) {
        return Effect.fail(new ValidationError({ message: `Invalid task status: ${status}` }));
    }
    return Effect.succeed(status as PlanTaskStatus);
}

function validateRelation(rel: string): Effect.Effect<PlanTaskChunkRelation, ValidationError> {
    if (!VALID_TASK_RELATIONS.includes(rel as PlanTaskChunkRelation)) {
        return Effect.fail(new ValidationError({ message: `Invalid task chunk relation: ${rel}` }));
    }
    return Effect.succeed(rel as PlanTaskChunkRelation);
}

export const planTaskRoutes = new Elysia({ prefix: "/api/plans/:id/tasks" })
    .post(
        "/",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() =>
                        planRepo.createTask({
                            planId: ctx.params.id,
                            title: ctx.body.title,
                            description: ctx.body.description ?? null,
                            acceptanceCriteria: ctx.body.acceptanceCriteria ?? [],
                            status: "pending",
                        }),
                    ),
                    Effect.flatMap(task =>
                        Effect.gen(function* () {
                            if (ctx.body.chunks) {
                                for (const c of ctx.body.chunks) {
                                    const rel = yield* validateRelation(c.relation);
                                    yield* planRepo.addTaskChunk(task.id, c.chunkId, rel);
                                }
                            }
                            if (ctx.body.dependsOnTaskIds) {
                                for (const depId of ctx.body.dependsOnTaskIds) {
                                    yield* planRepo.addTaskDependency(task.id, depId);
                                }
                            }
                            return task;
                        }),
                    ),
                ),
            );
            return result;
        },
        {
            body: t.Object({
                title: t.String(),
                description: t.Optional(t.String()),
                acceptanceCriteria: t.Optional(t.Array(t.String())),
                chunks: t.Optional(
                    t.Array(t.Object({ chunkId: t.String(), relation: t.String() })),
                ),
                dependsOnTaskIds: t.Optional(t.Array(t.String())),
            }),
        },
    )
    .patch(
        "/:taskId",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() =>
                        Effect.gen(function* () {
                            const patch: Record<string, unknown> = {};
                            if (ctx.body.title !== undefined) patch.title = ctx.body.title;
                            if (ctx.body.description !== undefined) patch.description = ctx.body.description;
                            if (ctx.body.acceptanceCriteria !== undefined)
                                patch.acceptanceCriteria = ctx.body.acceptanceCriteria;
                            let markedDone = false;
                            if (ctx.body.status !== undefined) {
                                const status = yield* validateStatus(ctx.body.status);
                                patch.status = status;
                                markedDone = status === "done";
                            }
                            const updated = yield* planRepo.updateTask(ctx.params.taskId, patch);
                            if (markedDone) {
                                yield* planRepo.unblockDependentsOf(ctx.params.taskId);
                            }
                            return updated;
                        }),
                    ),
                ),
            );
            return result;
        },
        {
            body: t.Object({
                title: t.Optional(t.String()),
                description: t.Optional(t.Union([t.String(), t.Null()])),
                acceptanceCriteria: t.Optional(t.Array(t.String())),
                status: t.Optional(t.String()),
            }),
        },
    )
    .delete("/:taskId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.deleteTask(ctx.params.taskId)),
            ),
        );
        return { ok: true };
    })
    .post(
        "/reorder",
        async ctx => {
            await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => planRepo.reorderTasks(ctx.params.id, ctx.body.taskIds)),
                ),
            );
            return { ok: true };
        },
        { body: t.Object({ taskIds: t.Array(t.String()) }) },
    )
    .post(
        "/:taskId/chunks",
        async ctx => {
            const result = await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => getPlan(ctx.params.id)),
                    Effect.flatMap(() => validateRelation(ctx.body.relation)),
                    Effect.flatMap(rel => planRepo.addTaskChunk(ctx.params.taskId, ctx.body.chunkId, rel)),
                ),
            );
            return result;
        },
        { body: t.Object({ chunkId: t.String(), relation: t.String() }) },
    )
    .delete("/:taskId/chunks/:linkId", async ctx => {
        await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => getPlan(ctx.params.id)),
                Effect.flatMap(() => planRepo.removeTaskChunk(ctx.params.linkId)),
            ),
        );
        return { ok: true };
    });
```

- [ ] **Step 2: Mount in routes.ts**

Add import and compose:

```typescript
import { planTaskRoutes } from "./tasks";

export const planRoutes = new Elysia()
    .use(planBase)
    .use(planRequirementRoutes)
    .use(planAnalyzeRoutes)
    .use(planTaskRoutes);
```

- [ ] **Step 3: Write auto-unblock repository test**

Create `packages/db/src/__tests__/plan-unblock.test.ts`:

```typescript
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Effect } from "effect";

import { db } from "../client";
import { plan, planTask, planTaskDependency } from "../schema/plan";
import { user } from "../schema/auth";
import * as planRepo from "../repository/plan";

describe("unblockDependentsOf", () => {
    let testUserId: string;
    let testPlanId: string;

    beforeEach(async () => {
        // Create a test user
        const [u] = await db
            .insert(user)
            .values({
                id: crypto.randomUUID(),
                email: `test-${Date.now()}@example.com`,
                name: "Test",
                emailVerified: false,
                createdAt: new Date(),
                updatedAt: new Date(),
            })
            .returning();
        if (!u) throw new Error("user insert failed");
        testUserId = u.id;

        const [p] = await db.insert(plan).values({ title: "Test", userId: testUserId }).returning();
        if (!p) throw new Error("plan insert failed");
        testPlanId = p.id;
    });

    afterEach(async () => {
        await db.delete(plan).where(eq(plan.id, testPlanId));
        await db.delete(user).where(eq(user.id, testUserId));
    });

    it("unblocks tasks whose dependency is marked done", async () => {
        const [t1] = await db.insert(planTask).values({ planId: testPlanId, title: "Dep" }).returning();
        const [t2] = await db
            .insert(planTask)
            .values({ planId: testPlanId, title: "Dependent", status: "blocked" })
            .returning();
        if (!t1 || !t2) throw new Error("task insert failed");

        await db.insert(planTaskDependency).values({ taskId: t2.id, dependsOnTaskId: t1.id });

        const unblocked = await Effect.runPromise(planRepo.unblockDependentsOf(t1.id));

        expect(unblocked).toContain(t2.id);

        const [refreshed] = await db.select().from(planTask).where(eq(planTask.id, t2.id));
        expect(refreshed?.status).toBe("pending");
    });

    it("does not touch tasks already in pending/in_progress", async () => {
        const [t1] = await db.insert(planTask).values({ planId: testPlanId, title: "Dep" }).returning();
        const [t2] = await db
            .insert(planTask)
            .values({ planId: testPlanId, title: "Dependent", status: "in_progress" })
            .returning();
        if (!t1 || !t2) throw new Error("task insert failed");

        await db.insert(planTaskDependency).values({ taskId: t2.id, dependsOnTaskId: t1.id });

        await Effect.runPromise(planRepo.unblockDependentsOf(t1.id));

        const [refreshed] = await db.select().from(planTask).where(eq(planTask.id, t2.id));
        expect(refreshed?.status).toBe("in_progress");
    });
});
```

Add the import at the top:

```typescript
import { eq } from "drizzle-orm";
```

- [ ] **Step 4: Run the test**

Run: `pnpm --filter @fubbik/db test plan-unblock`

Expected: 2 tests passing. If the test fails due to `user` table shape differences, read `packages/db/src/schema/auth.ts` and adjust the insert to match the actual user schema.

- [ ] **Step 5: Type check the API**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "plans/" | head -20`

Expected: zero errors in plans/.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/plans/tasks.ts packages/api/src/plans/routes.ts packages/db/src/__tests__/plan-unblock.test.ts
git commit -m "feat(api): add plan task routes with auto-unblock

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Delete Sessions from API

**Files:**
- Delete: `packages/api/src/sessions/routes.ts`
- Delete: `packages/api/src/sessions/service.ts`
- Delete: `packages/api/src/sessions/brief-generator.ts`
- Modify: `packages/api/src/index.ts` (unmount `sessionRoutes`)

- [ ] **Step 1: Delete the session source directory**

```bash
rm -rf packages/api/src/sessions/
```

- [ ] **Step 2: Edit `packages/api/src/index.ts`**

Read it first: `cat packages/api/src/index.ts`

Remove the line `import { sessionRoutes } from "./sessions/routes";` (or similar). Remove any `.use(sessionRoutes)` call from the Elysia chain. If there's an eden treaty type export that includes `sessionRoutes` in a union, remove it there too.

- [ ] **Step 3: Type check**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | tail -30`

Expected: no errors referencing `sessions/` or `sessionRoutes`. If there are lingering references (e.g., other modules importing from sessions), fix them by removing the imports.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/sessions packages/api/src/index.ts
git commit -m "feat(api): delete sessions subsystem

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Rewrite MCP Plan Tools + Delete Session Tools

**Files:**
- Rewrite: `packages/mcp/src/plan-tools.ts`
- Delete: `packages/mcp/src/session-tools.ts`
- Modify: `packages/mcp/src/index.ts` (unregister session tools)

- [ ] **Step 1: Read the current plan-tools.ts + index.ts to learn the registration pattern**

```bash
cat packages/mcp/src/plan-tools.ts | head -60
cat packages/mcp/src/index.ts
```

Note the `apiFetch` helper and the `server.tool(name, schema, handler)` shape.

- [ ] **Step 2: Rewrite `packages/mcp/src/plan-tools.ts`**

```typescript
import { z } from "zod";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";

import { apiFetch } from "./api-fetch";

export function registerPlanTools(server: McpServer) {
    server.tool(
        "create_plan",
        {
            title: z.string(),
            description: z.string().optional(),
            codebaseId: z.string().optional(),
            requirementIds: z.array(z.string()).optional(),
        },
        async ({ title, description, codebaseId, requirementIds }) => {
            const plan = await apiFetch("/api/plans", {
                method: "POST",
                body: { title, description, codebaseId, requirementIds },
            });
            return { content: [{ type: "text", text: JSON.stringify(plan, null, 2) }] };
        },
    );

    server.tool(
        "list_plans",
        {
            codebaseId: z.string().optional(),
            status: z.string().optional(),
            requirementId: z.string().optional(),
        },
        async args => {
            const params = new URLSearchParams();
            if (args.codebaseId) params.set("codebaseId", args.codebaseId);
            if (args.status) params.set("status", args.status);
            if (args.requirementId) params.set("requirementId", args.requirementId);
            const plans = await apiFetch(`/api/plans?${params}`);
            return { content: [{ type: "text", text: JSON.stringify(plans, null, 2) }] };
        },
    );

    server.tool("get_plan", { planId: z.string() }, async ({ planId }) => {
        const detail = await apiFetch(`/api/plans/${planId}`);
        return { content: [{ type: "text", text: JSON.stringify(detail, null, 2) }] };
    });

    server.tool(
        "update_plan",
        {
            planId: z.string(),
            title: z.string().optional(),
            description: z.string().optional(),
            status: z.string().optional(),
        },
        async ({ planId, ...patch }) => {
            const plan = await apiFetch(`/api/plans/${planId}`, { method: "PATCH", body: patch });
            return { content: [{ type: "text", text: JSON.stringify(plan, null, 2) }] };
        },
    );

    server.tool(
        "link_requirement",
        { planId: z.string(), requirementId: z.string() },
        async ({ planId, requirementId }) => {
            await apiFetch(`/api/plans/${planId}/requirements`, {
                method: "POST",
                body: { requirementId },
            });
            return { content: [{ type: "text", text: "Requirement linked" }] };
        },
    );

    server.tool(
        "unlink_requirement",
        { planId: z.string(), requirementId: z.string() },
        async ({ planId, requirementId }) => {
            await apiFetch(`/api/plans/${planId}/requirements/${requirementId}`, { method: "DELETE" });
            return { content: [{ type: "text", text: "Requirement unlinked" }] };
        },
    );

    server.tool(
        "add_analyze_item",
        {
            planId: z.string(),
            kind: z.enum(["chunk", "file", "risk", "assumption", "question"]),
            chunkId: z.string().optional(),
            filePath: z.string().optional(),
            text: z.string().optional(),
            metadata: z.record(z.unknown()).optional(),
        },
        async ({ planId, ...body }) => {
            const item = await apiFetch(`/api/plans/${planId}/analyze`, { method: "POST", body });
            return { content: [{ type: "text", text: JSON.stringify(item, null, 2) }] };
        },
    );

    server.tool(
        "update_analyze_item",
        {
            planId: z.string(),
            itemId: z.string(),
            text: z.string().optional(),
            metadata: z.record(z.unknown()).optional(),
        },
        async ({ planId, itemId, ...patch }) => {
            const item = await apiFetch(`/api/plans/${planId}/analyze/${itemId}`, {
                method: "PATCH",
                body: patch,
            });
            return { content: [{ type: "text", text: JSON.stringify(item, null, 2) }] };
        },
    );

    server.tool(
        "delete_analyze_item",
        { planId: z.string(), itemId: z.string() },
        async ({ planId, itemId }) => {
            await apiFetch(`/api/plans/${planId}/analyze/${itemId}`, { method: "DELETE" });
            return { content: [{ type: "text", text: "Analyze item deleted" }] };
        },
    );

    server.tool(
        "add_task",
        {
            planId: z.string(),
            title: z.string(),
            description: z.string().optional(),
            acceptanceCriteria: z.array(z.string()).optional(),
            chunks: z
                .array(z.object({ chunkId: z.string(), relation: z.enum(["context", "created", "modified"]) }))
                .optional(),
            dependsOnTaskIds: z.array(z.string()).optional(),
        },
        async ({ planId, ...body }) => {
            const task = await apiFetch(`/api/plans/${planId}/tasks`, { method: "POST", body });
            return { content: [{ type: "text", text: JSON.stringify(task, null, 2) }] };
        },
    );

    server.tool(
        "update_task",
        {
            planId: z.string(),
            taskId: z.string(),
            title: z.string().optional(),
            description: z.string().optional(),
            acceptanceCriteria: z.array(z.string()).optional(),
            status: z.enum(["pending", "in_progress", "done", "skipped", "blocked"]).optional(),
        },
        async ({ planId, taskId, ...patch }) => {
            const task = await apiFetch(`/api/plans/${planId}/tasks/${taskId}`, {
                method: "PATCH",
                body: patch,
            });
            return { content: [{ type: "text", text: JSON.stringify(task, null, 2) }] };
        },
    );

    server.tool(
        "delete_task",
        { planId: z.string(), taskId: z.string() },
        async ({ planId, taskId }) => {
            await apiFetch(`/api/plans/${planId}/tasks/${taskId}`, { method: "DELETE" });
            return { content: [{ type: "text", text: "Task deleted" }] };
        },
    );

    server.tool(
        "link_task_chunk",
        {
            planId: z.string(),
            taskId: z.string(),
            chunkId: z.string(),
            relation: z.enum(["context", "created", "modified"]),
        },
        async ({ planId, taskId, chunkId, relation }) => {
            const link = await apiFetch(`/api/plans/${planId}/tasks/${taskId}/chunks`, {
                method: "POST",
                body: { chunkId, relation },
            });
            return { content: [{ type: "text", text: JSON.stringify(link, null, 2) }] };
        },
    );
}
```

- [ ] **Step 3: Delete the session tools file**

```bash
rm packages/mcp/src/session-tools.ts
```

- [ ] **Step 4: Update `packages/mcp/src/index.ts`**

Remove the `import { registerSessionTools }` line and the `registerSessionTools(server)` call.

- [ ] **Step 5: Type check**

Run: `pnpm --filter @fubbik/mcp run check-types 2>&1 | tail -30`

Expected: zero errors.

- [ ] **Step 6: Commit**

```bash
git add packages/mcp/src/plan-tools.ts packages/mcp/src/session-tools.ts packages/mcp/src/index.ts
git commit -m "feat(mcp): rewrite plan tools for new API, delete session tools

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Update CLI Plan Commands

**Files:**
- Rewrite: `apps/cli/src/commands/plan.ts`

**Context:** The CLI commander registers commands against the server API. Read the current file first to learn the registration pattern and the `apiClient` helper.

- [ ] **Step 1: Read the current file**

```bash
cat apps/cli/src/commands/plan.ts
```

Take note of: how commands are registered, how API calls are made, output formatting helpers used.

- [ ] **Step 2: Rewrite `apps/cli/src/commands/plan.ts`**

Replace with a minimal command set matching the new API. Preserve the registration style from the current file (Commander.js). Example outline:

```typescript
import { Command } from "commander";

import { apiClient } from "../api-client";
import { printTable, printKeyValue } from "../output";

export function registerPlanCommands(program: Command) {
    const plan = program.command("plan").description("Manage plans");

    plan
        .command("create <title>")
        .description("Create a new plan")
        .option("-d, --description <description>", "Plan description")
        .option("-c, --codebase <codebaseId>", "Codebase ID")
        .action(async (title: string, opts) => {
            const created = await apiClient.post("/api/plans", {
                title,
                description: opts.description,
                codebaseId: opts.codebase,
            });
            printKeyValue(created);
        });

    plan
        .command("list")
        .description("List plans")
        .option("-s, --status <status>", "Filter by status")
        .option("-c, --codebase <codebaseId>", "Filter by codebase")
        .option("--archived", "Include archived plans")
        .action(async opts => {
            const params = new URLSearchParams();
            if (opts.status) params.set("status", opts.status);
            if (opts.codebase) params.set("codebaseId", opts.codebase);
            if (opts.archived) params.set("includeArchived", "true");
            const plans = await apiClient.get(`/api/plans?${params}`);
            printTable(plans, ["id", "title", "status", "createdAt"]);
        });

    plan
        .command("show <planId>")
        .description("Show plan detail")
        .action(async (planId: string) => {
            const detail = await apiClient.get(`/api/plans/${planId}`);
            console.log(JSON.stringify(detail, null, 2));
        });

    plan
        .command("status <planId> <status>")
        .description("Update plan status (draft, analyzing, ready, in_progress, completed, archived)")
        .action(async (planId: string, status: string) => {
            const updated = await apiClient.patch(`/api/plans/${planId}`, { status });
            printKeyValue(updated);
        });

    plan
        .command("add-task <planId> <title>")
        .description("Add a task to a plan")
        .option("-d, --description <description>", "Task description")
        .action(async (planId: string, title: string, opts) => {
            const task = await apiClient.post(`/api/plans/${planId}/tasks`, {
                title,
                description: opts.description,
            });
            printKeyValue(task);
        });

    plan
        .command("task-done <planId> <taskId>")
        .description("Mark a task as done")
        .action(async (planId: string, taskId: string) => {
            const updated = await apiClient.patch(`/api/plans/${planId}/tasks/${taskId}`, { status: "done" });
            printKeyValue(updated);
        });

    plan
        .command("link-requirement <planId> <requirementId>")
        .description("Link a requirement to a plan")
        .action(async (planId: string, requirementId: string) => {
            await apiClient.post(`/api/plans/${planId}/requirements`, { requirementId });
            console.log("Linked.");
        });
}
```

**Important:** If the actual `apiClient` in the repo has a different method shape (e.g., `apiClient.get<T>(...)` with a `.data` unwrap), adapt the calls to match. Read `apps/cli/src/api-client.ts` (or wherever it lives) to confirm.

- [ ] **Step 3: Type check**

Run: `pnpm --filter cli run check-types 2>&1 | tail -20`

Expected: zero errors.

- [ ] **Step 4: Smoke test the CLI help output**

Run: `pnpm --filter cli run dev -- plan --help`

Expected: subcommand list matches: `create`, `list`, `show`, `status`, `add-task`, `task-done`, `link-requirement`.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/commands/plan.ts
git commit -m "feat(cli): rewrite plan commands for new API

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Web Nav + Delete Reviews + Old Plans Components

**Files:**
- Modify: `apps/web/src/routes/__root.tsx` (nav swap, Manage dropdown)
- Delete: `apps/web/src/routes/reviews.tsx`
- Delete: `apps/web/src/routes/reviews_.queue.tsx`
- Delete: `apps/web/src/routes/reviews_.$sessionId.tsx`
- Delete: `apps/web/src/features/reviews/` (entire directory)
- Delete: `apps/web/src/features/plans/plan-progress-bar.tsx`
- Delete: `apps/web/src/features/plans/plan-step-item.tsx`
- Delete: `apps/web/src/features/plans/plan-timeline.tsx`
- Delete: `apps/web/src/features/plans/plans-list-content.tsx`

- [ ] **Step 1: Delete all /reviews route and feature files**

```bash
rm apps/web/src/routes/reviews.tsx \
   apps/web/src/routes/reviews_.queue.tsx \
   apps/web/src/routes/reviews_.$sessionId.tsx
rm -rf apps/web/src/features/reviews/
```

- [ ] **Step 2: Delete the old plans feature components**

```bash
rm apps/web/src/features/plans/plan-progress-bar.tsx \
   apps/web/src/features/plans/plan-step-item.tsx \
   apps/web/src/features/plans/plan-timeline.tsx \
   apps/web/src/features/plans/plans-list-content.tsx
```

- [ ] **Step 3: Regenerate the TanStack Router route tree**

Run: `pnpm --filter web run dev 2>&1 | head -5 &` and then kill it after a second. (The TanStack Router plugin regenerates `routeTree.gen.ts` on dev startup.) Alternatively: `pnpm --filter web run build` — the plugin regenerates as part of the build.

Verify: `grep -E "reviews" apps/web/src/routeTree.gen.ts` returns nothing.

- [ ] **Step 4: Update `apps/web/src/routes/__root.tsx`**

Read it first: `cat apps/web/src/routes/__root.tsx | head -250`

Find the primary nav. Find the `<Link to="/requirements">Requirements</Link>` element and replace it with `<Link to="/plans">Plans</Link>` (matching the same className pattern). Exact replacement:

```tsx
<Link
    to="/plans"
    className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
>
    Plans
</Link>
```

In the Manage dropdown content, the previous header-search redesign added Features / Reviews / Docs at the top with a DropdownMenuLabel "Navigate" and a separator. Now:

1. **Remove** the Reviews entry (the MessageSquare icon one)
2. **Add** a `Requirements` entry with a `ClipboardList` icon in the Navigate section:

```tsx
<DropdownMenuItem render={<Link to="/requirements" />}>
    <ClipboardList className="size-4" />
    Requirements
</DropdownMenuItem>
```

3. Update the lucide import at the top: remove `MessageSquare`, add `ClipboardList`.

- [ ] **Step 5: Update `apps/web/src/features/nav/mobile-nav.tsx`**

Read it first: `cat apps/web/src/features/nav/mobile-nav.tsx`

Expect to find `<Link to="/requirements">`, `<Link to="/reviews">`, and possibly `<Link to="/plans">` entries. Changes:
- Remove the `/reviews` link entirely
- Ensure `/plans` is a top-level entry (not buried in a collapsible)
- Demote `/requirements` below `/plans` or into a secondary group
- Remove the lucide `MessageSquare` icon import if it was only used by the reviews link

- [ ] **Step 6: Update the dashboard widget if it references sessions**

Run: `grep -rn "sessions\|reviews\|implementationSession" apps/web/src/routes/dashboard.tsx apps/web/src/features/dashboard 2>/dev/null | head -20`

If there are hits, open each file and:
- Remove any "Review Queue" widget (imports from `@/features/reviews/`, or calls `api.api.sessions.*`)
- Update any "Recent Plans" widget to use the new shape: show `title`, `status` pill (using `PlanStatusPill` from `@/features/plans/plan-status-pill` — this will be created in Task 11, so if Task 10 is dispatched before Task 11, stub the pill with plain text for now and circle back)
- Any call to `api.api.sessions.*` must be deleted

If the grep returns nothing, skip this step.

- [ ] **Step 7: Check for any remaining session/review references**

Run: `grep -rn "reviews\|sessionRoutes\|implementationSession" apps/web/src --include="*.tsx" --include="*.ts" | grep -v "node_modules\|useSession\|better-auth" | head -20`

Expected: only unrelated hits (e.g., `better-auth` session cookies, TanStack Router helper types). Remove any genuinely stale references.

- [ ] **Step 6: Type check the web package**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "(__root|reviews)" | head -20`

Expected: any errors in __root.tsx are about the old Requirements link being gone. The rest of the web package may have errors in `plans.*.tsx` files — those are rewritten in later tasks.

- [ ] **Step 8: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "(__root|mobile-nav|dashboard)" | head -20`

Expected: no new errors in the files touched above. (`plans.*.tsx` may still have errors — those are fixed in later tasks.)

- [ ] **Step 9: Commit**

```bash
git add apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx apps/web/src/routes/reviews* apps/web/src/features/reviews apps/web/src/features/plans apps/web/src/routeTree.gen.ts apps/web/src/routes/dashboard.tsx apps/web/src/features/dashboard
git commit -m "feat(web): swap Requirements→Plans in nav, delete /reviews, clean session refs

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

Note: the `git add` for dashboard paths will silently skip if those files didn't change.

---

## Task 11: Plans List Page

**Files:**
- Rewrite: `apps/web/src/routes/plans.index.tsx`
- Create: `apps/web/src/features/plans/plan-status-pill.tsx`

- [ ] **Step 1: Create `apps/web/src/features/plans/plan-status-pill.tsx`**

```typescript
import { cn } from "@/lib/utils";

export type PlanStatusValue =
    | "draft"
    | "analyzing"
    | "ready"
    | "in_progress"
    | "completed"
    | "archived";

const STATUS_STYLES: Record<PlanStatusValue, string> = {
    draft: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    analyzing: "bg-blue-500/15 border-blue-500/30 text-blue-400",
    ready: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    in_progress: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    completed: "bg-emerald-500/15 border-emerald-500/30 text-emerald-400",
    archived: "bg-zinc-500/15 border-zinc-500/30 text-zinc-500",
};

const STATUS_LABELS: Record<PlanStatusValue, string> = {
    draft: "Draft",
    analyzing: "Analyzing",
    ready: "Ready",
    in_progress: "In Progress",
    completed: "Completed",
    archived: "Archived",
};

export function PlanStatusPill({ status, className }: { status: PlanStatusValue; className?: string }) {
    return (
        <span
            className={cn(
                "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                STATUS_STYLES[status],
                className,
            )}
        >
            {STATUS_LABELS[status]}
        </span>
    );
}
```

- [ ] **Step 2: Rewrite `apps/web/src/routes/plans.index.tsx`**

```typescript
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import { PlanStatusPill, type PlanStatusValue } from "@/features/plans/plan-status-pill";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/")({ component: PlansIndexPage });

function PlansIndexPage() {
    const plansQuery = useQuery({
        queryKey: ["plans"],
        queryFn: async () => {
            const result = unwrapEden(await api.api.plans.get({ query: {} as any }));
            return (result as any[]) ?? [];
        },
    });

    return (
        <PageContainer>
            <PageHeader
                title="Plans"
                description="Plans are the home for a unit of work. Each plan holds its description, linked requirements, analyze notes, and tasks."
                actions={
                    <Button asChild>
                        <Link to="/plans/new">
                            <Plus className="size-4" />
                            New Plan
                        </Link>
                    </Button>
                }
            />
            {plansQuery.isLoading ? (
                <PageLoading />
            ) : (
                <div className="grid gap-2">
                    {(plansQuery.data ?? []).map((p: any) => (
                        <Link
                            key={p.id}
                            to="/plans/$planId"
                            params={{ planId: p.id }}
                            className="flex items-center gap-3 rounded-md border px-4 py-3 transition-colors hover:bg-muted/40"
                        >
                            <span className="flex-1">
                                <div className="font-medium">{p.title}</div>
                                {p.description && (
                                    <div className="text-xs text-muted-foreground line-clamp-1">{p.description}</div>
                                )}
                            </span>
                            <PlanStatusPill status={p.status as PlanStatusValue} />
                            <span className="text-[10px] text-muted-foreground">
                                {new Date(p.updatedAt).toLocaleDateString()}
                            </span>
                        </Link>
                    ))}
                    {(plansQuery.data ?? []).length === 0 && (
                        <div className="rounded-md border border-dashed p-8 text-center text-sm text-muted-foreground">
                            No plans yet. <Link to="/plans/new" className="underline">Create one</Link> to get started.
                        </div>
                    )}
                </div>
            )}
        </PageContainer>
    );
}
```

- [ ] **Step 3: Regenerate route tree**

Run: `pnpm --filter web run build 2>&1 | tail -5` (build triggers the router plugin to regenerate routes)

- [ ] **Step 4: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "plans\.index" | head -10`

Expected: zero errors in `plans.index.tsx`.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/plans.index.tsx apps/web/src/features/plans/plan-status-pill.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): rebuild plans list page

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Plan Create Page

**Files:**
- Rewrite: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Rewrite `apps/web/src/routes/plans.new.tsx`**

```typescript
import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageContainer, PageHeader } from "@/components/ui/page";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/new")({ component: NewPlanPage });

function NewPlanPage() {
    const navigate = useNavigate();
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [codebaseId, setCodebaseId] = useState<string>("");

    const codebasesQuery = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => (unwrapEden(await api.api.codebases.get()) as any[]) ?? [],
    });

    const createMutation = useMutation({
        mutationFn: async () => {
            const body: any = { title: title.trim() };
            if (description.trim()) body.description = description.trim();
            if (codebaseId) body.codebaseId = codebaseId;
            return unwrapEden(await api.api.plans.post(body));
        },
        onSuccess: plan => {
            navigate({ to: "/plans/$planId", params: { planId: (plan as any).id } });
        },
    });

    return (
        <PageContainer>
            <PageHeader title="New Plan" description="Start with a title and description. Link requirements, add analyze notes, and draft tasks on the detail page." />
            <form
                onSubmit={e => {
                    e.preventDefault();
                    if (!title.trim()) return;
                    createMutation.mutate();
                }}
                className="max-w-xl space-y-4"
            >
                <div className="space-y-2">
                    <Label htmlFor="title">Title</Label>
                    <Input id="title" value={title} onChange={e => setTitle(e.target.value)} required autoFocus />
                </div>
                <div className="space-y-2">
                    <Label htmlFor="description">Description (markdown)</Label>
                    <Textarea
                        id="description"
                        value={description}
                        onChange={e => setDescription(e.target.value)}
                        rows={6}
                        placeholder="What is this plan about?"
                    />
                </div>
                <div className="space-y-2">
                    <Label htmlFor="codebase">Codebase (optional)</Label>
                    <select
                        id="codebase"
                        value={codebaseId}
                        onChange={e => setCodebaseId(e.target.value)}
                        className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                    >
                        <option value="">— none —</option>
                        {(codebasesQuery.data ?? []).map((c: any) => (
                            <option key={c.id} value={c.id}>{c.name}</option>
                        ))}
                    </select>
                </div>
                <div className="flex gap-2">
                    <Button type="submit" disabled={!title.trim() || createMutation.isPending}>
                        {createMutation.isPending ? "Creating…" : "Create Plan"}
                    </Button>
                    <Button type="button" variant="ghost" onClick={() => navigate({ to: "/plans" })}>
                        Cancel
                    </Button>
                </div>
            </form>
        </PageContainer>
    );
}
```

- [ ] **Step 2: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "plans\.new" | head -10`

Expected: zero errors in `plans.new.tsx`.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/plans.new.tsx
git commit -m "feat(web): simplify plan create page

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 13: Plan Detail — Shell + Sticky Header + Description

**Files:**
- Rewrite: `apps/web/src/routes/plans.$planId.tsx`
- Create: `apps/web/src/features/plans/plan-detail-header.tsx`
- Create: `apps/web/src/features/plans/plan-description-section.tsx`

**Context:** The detail page is a single scrollable column. Subsequent tasks (14-17) add the four content sections as their own components so this shell stays short.

- [ ] **Step 1: Create `apps/web/src/features/plans/plan-detail-header.tsx`**

```typescript
import { useMutation } from "@tanstack/react-query";
import { Archive, Copy, Trash2 } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { PlanStatusPill, type PlanStatusValue } from "./plan-status-pill";

const STATUS_CYCLE: PlanStatusValue[] = ["draft", "analyzing", "ready", "in_progress", "completed"];

export interface PlanDetailHeaderProps {
    plan: { id: string; title: string; status: PlanStatusValue; updatedAt: string };
    taskCount: { done: number; total: number };
    onUpdate: () => void;
}

export function PlanDetailHeader({ plan, taskCount, onUpdate }: PlanDetailHeaderProps) {
    const [titleDraft, setTitleDraft] = useState(plan.title);
    const [editingTitle, setEditingTitle] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (patch: Record<string, unknown>) => {
            return unwrapEden(
                await (api.api.plans as any)[plan.id].patch(patch),
            );
        },
        onSuccess: () => onUpdate(),
    });

    const cycleStatus = () => {
        const idx = STATUS_CYCLE.indexOf(plan.status);
        const next = STATUS_CYCLE[(idx + 1) % STATUS_CYCLE.length];
        updateMutation.mutate({ status: next });
    };

    const progressPct = taskCount.total === 0 ? 0 : Math.round((taskCount.done / taskCount.total) * 100);

    return (
        <div className="sticky top-0 z-20 -mx-4 border-b bg-background/95 px-4 py-3 backdrop-blur">
            <div className="flex items-start gap-3">
                <div className="flex-1">
                    {editingTitle ? (
                        <input
                            autoFocus
                            value={titleDraft}
                            onChange={e => setTitleDraft(e.target.value)}
                            onBlur={() => {
                                setEditingTitle(false);
                                if (titleDraft !== plan.title) updateMutation.mutate({ title: titleDraft });
                            }}
                            onKeyDown={e => {
                                if (e.key === "Enter") e.currentTarget.blur();
                                if (e.key === "Escape") {
                                    setTitleDraft(plan.title);
                                    setEditingTitle(false);
                                }
                            }}
                            className="w-full bg-transparent text-xl font-semibold outline-none"
                        />
                    ) : (
                        <h1
                            className="cursor-text text-xl font-semibold"
                            onClick={() => setEditingTitle(true)}
                        >
                            {plan.title}
                        </h1>
                    )}
                    <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
                        <button type="button" onClick={cycleStatus} className="hover:opacity-80">
                            <PlanStatusPill status={plan.status} />
                        </button>
                        <span>•</span>
                        <span className="font-mono">{taskCount.done}/{taskCount.total} tasks</span>
                        <div className="h-1 flex-1 max-w-[120px] overflow-hidden rounded bg-muted">
                            <div className="h-full bg-emerald-500" style={{ width: `${progressPct}%` }} />
                        </div>
                        <span>•</span>
                        <span>Updated {new Date(plan.updatedAt).toLocaleDateString()}</span>
                    </div>
                </div>
                <div className="flex gap-1">
                    <Button variant="ghost" size="sm" title="Archive" onClick={() => updateMutation.mutate({ status: "archived" })}>
                        <Archive className="size-4" />
                    </Button>
                    <Button variant="ghost" size="sm" title="Duplicate">
                        <Copy className="size-4" />
                    </Button>
                    <Button variant="ghost" size="sm" title="Delete">
                        <Trash2 className="size-4" />
                    </Button>
                </div>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Create `apps/web/src/features/plans/plan-description-section.tsx`**

```typescript
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface PlanDescriptionSectionProps {
    planId: string;
    description: string | null;
    onUpdate: () => void;
}

export function PlanDescriptionSection({ planId, description, onUpdate }: PlanDescriptionSectionProps) {
    const [draft, setDraft] = useState(description ?? "");
    const [editing, setEditing] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (body: Record<string, unknown>) =>
            unwrapEden(await (api.api.plans as any)[planId].patch(body)),
        onSuccess: () => {
            setEditing(false);
            onUpdate();
        },
    });

    const handleSave = () => {
        if (draft !== (description ?? "")) {
            updateMutation.mutate({ description: draft || null });
        } else {
            setEditing(false);
        }
    };

    return (
        <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Description</h2>
            {editing ? (
                <Textarea
                    autoFocus
                    value={draft}
                    onChange={e => setDraft(e.target.value)}
                    onBlur={handleSave}
                    rows={8}
                    placeholder="Describe what this plan is about"
                />
            ) : (
                <div
                    className="cursor-text rounded-md border border-transparent p-2 hover:border-border"
                    onClick={() => setEditing(true)}
                >
                    {description ? (
                        <div className="whitespace-pre-wrap text-sm">{description}</div>
                    ) : (
                        <div className="text-sm text-muted-foreground">Describe what this plan is about</div>
                    )}
                </div>
            )}
        </section>
    );
}
```

- [ ] **Step 3: Rewrite `apps/web/src/routes/plans.$planId.tsx`**

```typescript
import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { PageContainer, PageLoading } from "@/components/ui/page";
import { PlanDescriptionSection } from "@/features/plans/plan-description-section";
import { PlanDetailHeader } from "@/features/plans/plan-detail-header";
import type { PlanStatusValue } from "@/features/plans/plan-status-pill";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/$planId")({ component: PlanDetailPage });

function PlanDetailPage() {
    const { planId } = Route.useParams();

    const detailQuery = useQuery({
        queryKey: ["plan-detail", planId],
        queryFn: async () => unwrapEden(await (api.api.plans as any)[planId].get()),
    });

    if (detailQuery.isLoading) return <PageContainer><PageLoading /></PageContainer>;
    if (!detailQuery.data) return <PageContainer>Plan not found</PageContainer>;

    const detail = detailQuery.data as any;
    const plan = detail.plan;
    const tasks = detail.tasks ?? [];
    const doneCount = tasks.filter((t: any) => t.status === "done").length;

    const refetch = () => detailQuery.refetch();

    return (
        <PageContainer>
            <PlanDetailHeader
                plan={{ id: plan.id, title: plan.title, status: plan.status as PlanStatusValue, updatedAt: plan.updatedAt }}
                taskCount={{ done: doneCount, total: tasks.length }}
                onUpdate={refetch}
            />
            <div className="space-y-8 pb-12 pt-6">
                <PlanDescriptionSection planId={plan.id} description={plan.description} onUpdate={refetch} />
                {/* Sections 2, 3, 4 added in subsequent tasks */}
            </div>
        </PageContainer>
    );
}
```

- [ ] **Step 4: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "plans\.\\\$planId|plan-detail-header|plan-description-section" | head -10`

Expected: zero errors. Note: the `(api.api.plans as any)[planId]` pattern is a deliberate Eden escape hatch since param-keyed routes need runtime indexing.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/plans.\$planId.tsx apps/web/src/features/plans/plan-detail-header.tsx apps/web/src/features/plans/plan-description-section.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): plan detail shell, sticky header, description section

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 14: Plan Detail — Requirements Section

**Files:**
- Create: `apps/web/src/features/plans/plan-requirements-section.tsx`
- Modify: `apps/web/src/routes/plans.$planId.tsx` (mount)

- [ ] **Step 1: Create `apps/web/src/features/plans/plan-requirements-section.tsx`**

```typescript
import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ChevronRight, Plus, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface PlanRequirementsSectionProps {
    planId: string;
    requirements: Array<{ id: string; requirementId: string; order: number }>;
    onUpdate: () => void;
}

export function PlanRequirementsSection({ planId, requirements, onUpdate }: PlanRequirementsSectionProps) {
    const [pickerOpen, setPickerOpen] = useState(false);
    const [pickerQuery, setPickerQuery] = useState("");

    // Fetch requirement details for each linked requirementId
    const reqIds = requirements.map(r => r.requirementId);
    const requirementDetailsQuery = useQuery({
        queryKey: ["requirements-detail", reqIds],
        queryFn: async () => {
            if (reqIds.length === 0) return [];
            const all = unwrapEden(await api.api.requirements.get({ query: {} as any })) as any[];
            return all.filter(r => reqIds.includes(r.id));
        },
    });

    const searchQuery = useQuery({
        queryKey: ["requirements-search", pickerQuery],
        queryFn: async () => {
            const all = unwrapEden(await api.api.requirements.get({ query: {} as any })) as any[];
            if (!pickerQuery) return all.slice(0, 10);
            return all
                .filter(r => r.title.toLowerCase().includes(pickerQuery.toLowerCase()))
                .slice(0, 10);
        },
        enabled: pickerOpen,
    });

    const addMutation = useMutation({
        mutationFn: async (requirementId: string) =>
            unwrapEden(
                await (api.api.plans as any)[planId].requirements.post({ requirementId }),
            ),
        onSuccess: () => {
            setPickerOpen(false);
            setPickerQuery("");
            onUpdate();
        },
    });

    const removeMutation = useMutation({
        mutationFn: async (requirementId: string) =>
            unwrapEden(
                await (api.api.plans as any)[planId].requirements[requirementId].delete(),
            ),
        onSuccess: () => onUpdate(),
    });

    const linkedIds = new Set(reqIds);
    const available = (searchQuery.data ?? []).filter((r: any) => !linkedIds.has(r.id));

    return (
        <section className="space-y-2">
            <div className="flex items-center justify-between">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Requirements <span className="ml-1 font-mono text-muted-foreground/60">({requirements.length})</span>
                </h2>
                <Button size="sm" variant="ghost" onClick={() => setPickerOpen(o => !o)}>
                    <Plus className="size-3.5" />
                    Add
                </Button>
            </div>
            {pickerOpen && (
                <div className="rounded-md border bg-card p-2 shadow-sm">
                    <Input
                        autoFocus
                        placeholder="Search requirements…"
                        value={pickerQuery}
                        onChange={e => setPickerQuery(e.target.value)}
                        className="mb-2 h-8 text-sm"
                    />
                    <div className="max-h-60 space-y-1 overflow-y-auto">
                        {available.length === 0 ? (
                            <div className="py-3 text-center text-xs text-muted-foreground">No matches</div>
                        ) : (
                            available.map((r: any) => (
                                <button
                                    key={r.id}
                                    type="button"
                                    onClick={() => addMutation.mutate(r.id)}
                                    className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-muted"
                                >
                                    <span className="flex-1 truncate">{r.title}</span>
                                    <span className="text-[9px] uppercase text-muted-foreground">{r.priority}</span>
                                </button>
                            ))
                        )}
                    </div>
                </div>
            )}
            {requirements.length === 0 ? (
                <div className="rounded-md border border-dashed p-4 text-center text-xs text-muted-foreground">
                    No requirements linked. Add one to document what this plan must satisfy.
                </div>
            ) : (
                <div className="space-y-1">
                    {(requirementDetailsQuery.data ?? []).map((r: any) => (
                        <div key={r.id} className="flex items-center gap-2 rounded-md border px-3 py-2">
                            <span className="flex-1 truncate text-sm">{r.title}</span>
                            <span className="text-[10px] uppercase text-muted-foreground">{r.status}</span>
                            <span className="text-[10px] uppercase text-muted-foreground">{r.priority}</span>
                            <button
                                type="button"
                                onClick={() => removeMutation.mutate(r.id)}
                                className="text-muted-foreground hover:text-foreground"
                                aria-label="Remove requirement"
                            >
                                <X className="size-3" />
                            </button>
                            <Link to="/requirements/$requirementId" params={{ requirementId: r.id }} className="text-muted-foreground hover:text-foreground">
                                <ChevronRight className="size-4" />
                            </Link>
                        </div>
                    ))}
                </div>
            )}
        </section>
    );
}
```

- [ ] **Step 2: Mount the section in `plans.$planId.tsx`**

Add the import:

```typescript
import { PlanRequirementsSection } from "@/features/plans/plan-requirements-section";
```

Below `PlanDescriptionSection`, render:

```tsx
<PlanRequirementsSection
    planId={plan.id}
    requirements={detail.requirements ?? []}
    onUpdate={refetch}
/>
```

- [ ] **Step 3: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "plan-requirements-section|plans\.\\\$planId" | head -10`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/plans/plan-requirements-section.tsx apps/web/src/routes/plans.\$planId.tsx
git commit -m "feat(web): plan detail requirements section

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 15: Plan Detail — Analyze Section

**Files:**
- Create: `apps/web/src/features/plans/plan-analyze-section.tsx`
- Modify: `apps/web/src/routes/plans.$planId.tsx` (mount)

**Context:** To keep this task bounded, we implement the Analyze section as a single component that handles all five kinds inline with kind-specific input fields. Drag-reorder is out of scope for v1 (YAGNI — can be added later).

- [ ] **Step 1: Create `apps/web/src/features/plans/plan-analyze-section.tsx`**

```typescript
import { useMutation } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Plus, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type AnalyzeKind = "chunk" | "file" | "risk" | "assumption" | "question";

interface AnalyzeItem {
    id: string;
    kind: AnalyzeKind;
    chunkId: string | null;
    filePath: string | null;
    text: string | null;
    metadata: Record<string, unknown>;
}

interface AnalyzeGroups {
    chunk: AnalyzeItem[];
    file: AnalyzeItem[];
    risk: AnalyzeItem[];
    assumption: AnalyzeItem[];
    question: AnalyzeItem[];
}

const KIND_LABELS: Record<AnalyzeKind, string> = {
    chunk: "Chunks",
    file: "Files",
    risk: "Risks",
    assumption: "Assumptions",
    question: "Questions",
};

const KIND_ORDER: AnalyzeKind[] = ["chunk", "file", "risk", "assumption", "question"];

export interface PlanAnalyzeSectionProps {
    planId: string;
    analyze: AnalyzeGroups;
    onUpdate: () => void;
}

export function PlanAnalyzeSection({ planId, analyze, onUpdate }: PlanAnalyzeSectionProps) {
    const [openKinds, setOpenKinds] = useState<Set<AnalyzeKind>>(new Set(KIND_ORDER));

    const toggle = (k: AnalyzeKind) =>
        setOpenKinds(prev => {
            const next = new Set(prev);
            if (next.has(k)) next.delete(k);
            else next.add(k);
            return next;
        });

    return (
        <section className="space-y-3">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Analyze</h2>
            <div className="space-y-2 rounded-md border p-2">
                {KIND_ORDER.map(kind => (
                    <AnalyzeKindBlock
                        key={kind}
                        kind={kind}
                        items={analyze[kind]}
                        planId={planId}
                        open={openKinds.has(kind)}
                        onToggle={() => toggle(kind)}
                        onUpdate={onUpdate}
                    />
                ))}
            </div>
        </section>
    );
}

function AnalyzeKindBlock({
    kind,
    items,
    planId,
    open,
    onToggle,
    onUpdate,
}: {
    kind: AnalyzeKind;
    items: AnalyzeItem[];
    planId: string;
    open: boolean;
    onToggle: () => void;
    onUpdate: () => void;
}) {
    const [adding, setAdding] = useState(false);
    const [draftText, setDraftText] = useState("");
    const [draftFilePath, setDraftFilePath] = useState("");
    const [draftSeverity, setDraftSeverity] = useState<"low" | "medium" | "high">("medium");

    const addMutation = useMutation({
        mutationFn: async () => {
            const body: Record<string, unknown> = { kind };
            if (kind === "file") {
                body.filePath = draftFilePath;
                body.text = draftText;
            } else if (kind === "risk") {
                body.text = draftText;
                body.metadata = { severity: draftSeverity };
            } else if (kind === "assumption") {
                body.text = draftText;
                body.metadata = { verified: false };
            } else if (kind === "question") {
                body.text = draftText;
                body.metadata = { answered: false };
            } else if (kind === "chunk") {
                // chunk picker would go here; v1 uses plain chunk ID input
                body.chunkId = draftText;
            }
            return unwrapEden(await (api.api.plans as any)[planId].analyze.post(body));
        },
        onSuccess: () => {
            setAdding(false);
            setDraftText("");
            setDraftFilePath("");
            onUpdate();
        },
    });

    const deleteMutation = useMutation({
        mutationFn: async (itemId: string) =>
            unwrapEden(await (api.api.plans as any)[planId].analyze[itemId].delete()),
        onSuccess: () => onUpdate(),
    });

    return (
        <div className="rounded border">
            <button
                type="button"
                onClick={onToggle}
                className="flex w-full items-center gap-2 px-2 py-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:text-foreground"
            >
                {open ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                {KIND_LABELS[kind]}
                <span className="ml-1 font-mono text-muted-foreground/60">({items.length})</span>
                <div className="ml-auto" />
                {open && (
                    <span
                        role="button"
                        tabIndex={0}
                        onClick={e => {
                            e.stopPropagation();
                            setAdding(a => !a);
                        }}
                        className="text-muted-foreground hover:text-foreground"
                    >
                        <Plus className="size-3" />
                    </span>
                )}
            </button>
            {open && (
                <div className="space-y-1 px-2 pb-2">
                    {adding && (
                        <div className="space-y-2 rounded border bg-muted/30 p-2">
                            {kind === "file" && (
                                <Input
                                    placeholder="File path, e.g. src/foo.ts"
                                    value={draftFilePath}
                                    onChange={e => setDraftFilePath(e.target.value)}
                                    className="h-7 text-xs"
                                />
                            )}
                            <Input
                                autoFocus
                                placeholder={
                                    kind === "chunk"
                                        ? "Chunk ID"
                                        : kind === "risk"
                                          ? "Describe the risk"
                                          : kind === "assumption"
                                            ? "What are you assuming?"
                                            : kind === "question"
                                              ? "What's the open question?"
                                              : "Note"
                                }
                                value={draftText}
                                onChange={e => setDraftText(e.target.value)}
                                className="h-7 text-xs"
                            />
                            {kind === "risk" && (
                                <select
                                    value={draftSeverity}
                                    onChange={e => setDraftSeverity(e.target.value as any)}
                                    className="h-7 w-full rounded border bg-background px-2 text-xs"
                                >
                                    <option value="low">Low</option>
                                    <option value="medium">Medium</option>
                                    <option value="high">High</option>
                                </select>
                            )}
                            <div className="flex gap-2">
                                <Button size="sm" onClick={() => addMutation.mutate()}>
                                    Add
                                </Button>
                                <Button size="sm" variant="ghost" onClick={() => setAdding(false)}>
                                    Cancel
                                </Button>
                            </div>
                        </div>
                    )}
                    {items.map(item => (
                        <div key={item.id} className="flex items-center gap-2 rounded px-2 py-1 text-xs hover:bg-muted/40">
                            <span className="flex-1 truncate">
                                {kind === "file" && item.filePath ? (
                                    <>
                                        <span className="font-mono">{item.filePath}</span>
                                        {item.text && <span className="ml-2 text-muted-foreground">— {item.text}</span>}
                                    </>
                                ) : (
                                    item.text ?? item.chunkId ?? "(empty)"
                                )}
                            </span>
                            {kind === "risk" && (
                                <span className="text-[9px] uppercase text-muted-foreground">
                                    {(item.metadata as any)?.severity ?? "medium"}
                                </span>
                            )}
                            <button
                                type="button"
                                onClick={() => deleteMutation.mutate(item.id)}
                                className="text-muted-foreground hover:text-foreground"
                                aria-label="Delete"
                            >
                                <X className="size-3" />
                            </button>
                        </div>
                    ))}
                    {items.length === 0 && !adding && (
                        <div className="py-2 text-center text-[10px] text-muted-foreground">
                            None yet
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Mount in `plans.$planId.tsx`**

Add import:

```typescript
import { PlanAnalyzeSection } from "@/features/plans/plan-analyze-section";
```

Below the requirements section, render:

```tsx
<PlanAnalyzeSection
    planId={plan.id}
    analyze={detail.analyze ?? { chunk: [], file: [], risk: [], assumption: [], question: [] }}
    onUpdate={refetch}
/>
```

- [ ] **Step 3: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "plan-analyze-section|plans\.\\\$planId" | head -10`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/plans/plan-analyze-section.tsx apps/web/src/routes/plans.\$planId.tsx
git commit -m "feat(web): plan detail analyze section with five kinds

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 16: Plan Detail — Tasks Section

**Files:**
- Create: `apps/web/src/features/plans/plan-tasks-section.tsx`
- Create: `apps/web/src/features/plans/plan-task-card.tsx`
- Modify: `apps/web/src/routes/plans.$planId.tsx` (mount)

- [ ] **Step 1: Create `apps/web/src/features/plans/plan-task-card.tsx`**

```typescript
import { useMutation } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, X } from "lucide-react";
import { useState } from "react";

import { Checkbox } from "@/components/ui/checkbox";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type TaskStatus = "pending" | "in_progress" | "done" | "skipped" | "blocked";

export interface Task {
    id: string;
    title: string;
    description: string | null;
    acceptanceCriteria: string[];
    status: TaskStatus;
    chunks: Array<{ id: string; chunkId: string; relation: string }>;
}

export interface PlanTaskCardProps {
    planId: string;
    task: Task;
    onUpdate: () => void;
}

export function PlanTaskCard({ planId, task, onUpdate }: PlanTaskCardProps) {
    const [expanded, setExpanded] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (patch: Record<string, unknown>) =>
            unwrapEden(
                await (api.api.plans as any)[planId].tasks[task.id].patch(patch),
            ),
        onSuccess: () => onUpdate(),
    });

    const deleteMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api.plans as any)[planId].tasks[task.id].delete()),
        onSuccess: () => onUpdate(),
    });

    const toggleDone = () => {
        updateMutation.mutate({ status: task.status === "done" ? "pending" : "done" });
    };

    return (
        <div className="rounded-md border bg-card">
            <div className="flex items-start gap-3 p-3">
                <Checkbox checked={task.status === "done"} onCheckedChange={toggleDone} className="mt-0.5" />
                <button type="button" onClick={() => setExpanded(e => !e)} className="flex-1 text-left">
                    <div className="flex items-center gap-2">
                        <span className={`font-medium ${task.status === "done" ? "line-through text-muted-foreground" : ""}`}>
                            {task.title}
                        </span>
                        {task.status === "blocked" && (
                            <span className="text-[9px] uppercase text-amber-500">blocked</span>
                        )}
                        {task.status === "in_progress" && (
                            <span className="text-[9px] uppercase text-blue-500">in progress</span>
                        )}
                    </div>
                    {task.description && !expanded && (
                        <div className="mt-1 line-clamp-1 text-xs text-muted-foreground">{task.description}</div>
                    )}
                </button>
                <button type="button" onClick={() => setExpanded(e => !e)} className="text-muted-foreground">
                    {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                </button>
                <button
                    type="button"
                    onClick={() => deleteMutation.mutate()}
                    className="text-muted-foreground hover:text-foreground"
                    aria-label="Delete task"
                >
                    <X className="size-3" />
                </button>
            </div>
            {expanded && (
                <div className="border-t px-3 py-2 text-xs">
                    {task.description && (
                        <div className="mb-2 whitespace-pre-wrap">{task.description}</div>
                    )}
                    {task.acceptanceCriteria.length > 0 && (
                        <div className="mb-2 space-y-1">
                            <div className="text-[10px] uppercase text-muted-foreground">Acceptance</div>
                            {task.acceptanceCriteria.map((ac, i) => (
                                <div key={i} className="flex items-start gap-2">
                                    <span className="mt-[2px] size-3 shrink-0 rounded border" />
                                    <span>{ac}</span>
                                </div>
                            ))}
                        </div>
                    )}
                    {task.chunks.length > 0 && (
                        <div className="mb-2">
                            <div className="text-[10px] uppercase text-muted-foreground">Chunks</div>
                            <div className="flex flex-wrap gap-1">
                                {task.chunks.map(c => (
                                    <span key={c.id} className="rounded bg-muted px-2 py-0.5 font-mono text-[10px]">
                                        {c.relation}:{c.chunkId.slice(0, 8)}
                                    </span>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Create `apps/web/src/features/plans/plan-tasks-section.tsx`**

```typescript
import { useMutation } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { PlanTaskCard, type Task } from "./plan-task-card";

export interface PlanTasksSectionProps {
    planId: string;
    tasks: Task[];
    onUpdate: () => void;
}

export function PlanTasksSection({ planId, tasks, onUpdate }: PlanTasksSectionProps) {
    const [adding, setAdding] = useState(false);
    const [draftTitle, setDraftTitle] = useState("");

    const addMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(
                await (api.api.plans as any)[planId].tasks.post({ title: draftTitle.trim() }),
            ),
        onSuccess: () => {
            setDraftTitle("");
            setAdding(false);
            onUpdate();
        },
    });

    const doneCount = tasks.filter(t => t.status === "done").length;

    return (
        <section className="space-y-2">
            <div className="flex items-center justify-between">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Tasks <span className="ml-1 font-mono text-muted-foreground/60">{doneCount}/{tasks.length} done</span>
                </h2>
                <Button size="sm" variant="ghost" onClick={() => setAdding(a => !a)}>
                    <Plus className="size-3.5" />
                    Add task
                </Button>
            </div>
            {adding && (
                <div className="flex gap-2 rounded-md border bg-card p-2">
                    <Input
                        autoFocus
                        placeholder="Task title"
                        value={draftTitle}
                        onChange={e => setDraftTitle(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter" && draftTitle.trim()) addMutation.mutate();
                            if (e.key === "Escape") {
                                setAdding(false);
                                setDraftTitle("");
                            }
                        }}
                        className="h-8 text-sm"
                    />
                    <Button size="sm" onClick={() => draftTitle.trim() && addMutation.mutate()}>
                        Add
                    </Button>
                </div>
            )}
            {tasks.length === 0 && !adding ? (
                <div className="rounded-md border border-dashed p-4 text-center text-xs text-muted-foreground">
                    No tasks yet. Add the first one to start executing.
                </div>
            ) : (
                <div className="space-y-2">
                    {tasks.map(t => (
                        <PlanTaskCard key={t.id} planId={planId} task={t} onUpdate={onUpdate} />
                    ))}
                </div>
            )}
        </section>
    );
}
```

- [ ] **Step 3: Mount in `plans.$planId.tsx`**

Add import:

```typescript
import { PlanTasksSection } from "@/features/plans/plan-tasks-section";
```

Below the analyze section:

```tsx
<PlanTasksSection planId={plan.id} tasks={tasks} onUpdate={refetch} />
```

- [ ] **Step 4: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "plan-tasks-section|plan-task-card|plans\.\\\$planId" | head -10`

Expected: zero errors.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/plans/plan-tasks-section.tsx apps/web/src/features/plans/plan-task-card.tsx apps/web/src/routes/plans.\$planId.tsx
git commit -m "feat(web): plan detail tasks section with expandable task cards

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 17: Seed Script Rewrite

**Files:**
- Modify: `packages/db/src/seed.ts`

**Context:** Only the plan-related sections of the seed script need updating. Other seed data (chunks, requirements, codebases, tags) stays unchanged.

- [ ] **Step 1: Read the current seed script**

```bash
cat packages/db/src/seed.ts | grep -n -A 5 "plan" | head -60
```

Identify where plans were previously seeded. The old pattern likely inserts into `plan` + `plan_step` + `plan_chunk_ref`.

- [ ] **Step 2: Replace the plan-seeding section**

Find the section that seeds plans. Replace it with code that creates three plans in the new shape. The exact insertion style (using `db.insert(plan).values(...)` or a helper) should match the surrounding seed code. Example:

```typescript
// Seed plans
const [planCompleted] = await db
    .insert(plan)
    .values({
        title: "Add user avatars",
        description: "Let users upload a profile picture. Shown on chunk detail pages and in the top-right nav.",
        status: "completed",
        userId: devUserId,
        codebaseId: fubbikCodebaseId,
        completedAt: new Date(),
    })
    .returning();

if (!planCompleted) throw new Error("failed to seed completed plan");

await db.insert(planTask).values([
    { planId: planCompleted.id, title: "Add avatar column to user table", status: "done", order: 0 },
    { planId: planCompleted.id, title: "Wire upload endpoint", status: "done", order: 1 },
    { planId: planCompleted.id, title: "Render avatar in nav", status: "done", order: 2 },
]);

const [planInProgress] = await db
    .insert(plan)
    .values({
        title: "Federated chunk search",
        description: "Search chunks across all linked codebases from one query. Returns results grouped by codebase.",
        status: "in_progress",
        userId: devUserId,
        codebaseId: fubbikCodebaseId,
    })
    .returning();

if (!planInProgress) throw new Error("failed to seed in_progress plan");

// Link a requirement if one exists
if (requirementIds[0]) {
    await db.insert(planRequirement).values({ planId: planInProgress.id, requirementId: requirementIds[0], order: 0 });
}

await db.insert(planAnalyzeItem).values([
    {
        planId: planInProgress.id,
        kind: "chunk",
        chunkId: chunkIds[0] ?? null,
        text: "Existing search service lives here",
        order: 0,
    },
    {
        planId: planInProgress.id,
        kind: "file",
        filePath: "packages/api/src/search/service.ts",
        text: "Main search entry point",
        metadata: { lineStart: 1, lineEnd: 50 },
        order: 0,
    },
    {
        planId: planInProgress.id,
        kind: "risk",
        text: "Cross-codebase indexes may blow up memory for 10+ codebases",
        metadata: { severity: "medium" },
        order: 0,
    },
    {
        planId: planInProgress.id,
        kind: "assumption",
        text: "All codebases share the same embedding model",
        metadata: { verified: false },
        order: 0,
    },
    {
        planId: planInProgress.id,
        kind: "question",
        text: "Should archived codebases be searchable?",
        metadata: { answered: false },
        order: 0,
    },
]);

await db.insert(planTask).values([
    { planId: planInProgress.id, title: "Extend search service to accept codebase list", status: "done", order: 0 },
    { planId: planInProgress.id, title: "Group results by codebase in response", status: "in_progress", order: 1 },
    { planId: planInProgress.id, title: "Add federated mode toggle to search page", status: "pending", order: 2 },
    { planId: planInProgress.id, title: "Integration test: 3 codebases, 1 query", status: "pending", order: 3 },
    { planId: planInProgress.id, title: "Update CLAUDE.md with federated search docs", status: "pending", order: 4 },
]);

const [planAnalyzing] = await db
    .insert(plan)
    .values({
        title: "Plans as a central entity",
        description: "Make Plan the home for a unit of work — description, linked requirements, structured analyze fields, and enriched tasks.",
        status: "analyzing",
        userId: devUserId,
        codebaseId: fubbikCodebaseId,
    })
    .returning();

if (!planAnalyzing) throw new Error("failed to seed analyzing plan");

await db.insert(planAnalyzeItem).values([
    {
        planId: planAnalyzing.id,
        kind: "risk",
        text: "Dropping session data loses review history",
        metadata: { severity: "low" },
        order: 0,
    },
    {
        planId: planAnalyzing.id,
        kind: "assumption",
        text: "Existing plan data is mostly seed/scratch, safe to wipe",
        metadata: { verified: true },
        order: 0,
    },
]);
```

Adapt the variable names (`devUserId`, `fubbikCodebaseId`, `requirementIds`, `chunkIds`) to match whatever names the existing seed file uses.

Remove the old plan-seeding code that uses `plan_step`, `plan_chunk_ref`, and `implementation_session`.

- [ ] **Step 3: Update the imports at the top of `seed.ts`**

Add:
```typescript
import { plan, planRequirement, planAnalyzeItem, planTask } from "./schema/plan";
```

Remove any imports from `./schema/implementation-session` or old plan sub-tables (`planStep`, `planChunkRef`).

- [ ] **Step 4: Run the seed**

```bash
pnpm db:push && pnpm seed
```

Expected: no errors. Verify: `psql "$DATABASE_URL" -c "SELECT count(*) FROM plan;"` returns at least 3.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/seed.ts
git commit -m "feat(db): rewrite plan seed data for new schema

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Task 18: CLAUDE.md + Final Verification

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the "Plans" core concept section in `CLAUDE.md`**

Find the "### Plans" subsection under "## Core Concepts". Replace its body with:

```markdown
### Plans

The central unit of work. Each plan holds a description, linked requirements, structured analyze fields, and enriched tasks.

- `plan` table: `title`, `description` (markdown), `status` (`draft | analyzing | ready | in_progress | completed | archived` — labels only, ungated), `userId`, `codebaseId`, `completedAt`
- `plan_requirement` — many-to-many link to existing `requirement` entities at the plan level
- `plan_analyze_item` — discriminated table holding five kinds: `chunk`, `file`, `risk`, `assumption`, `question`, each with kind-specific metadata (severity for risks, verified flag for assumptions, answer for questions, line range for files)
- `plan_task` — enriched tasks with `title`, `description`, `acceptanceCriteria` (JSONB string array), `status`
- `plan_task_chunk` — many-to-many linking tasks to multiple chunks with a relation (`context | created | modified`)
- `plan_task_dependency` — task dependencies; marking a task `done` auto-unblocks dependents in `blocked` state
- Web UI: `/plans` list, `/plans/new` (simple form), `/plans/:id` (sticky header + four sections: description, requirements, analyze, tasks)
- CLI: `fubbik plan create/list/show/status/add-task/task-done/link-requirement`
```

- [ ] **Step 2: Remove the "### Implementation Sessions (Reviews)" section entirely**

Delete the entire subsection from CLAUDE.md.

- [ ] **Step 3: Update the "## API Endpoints" section**

Replace the `### Plans` subsection with the new route list:

```markdown
### Plans
- `GET /api/plans` — list (filters: `codebaseId`, `status`, `requirementId`, `includeArchived`)
- `POST /api/plans` — create (body: `title`, `description?`, `codebaseId?`, `requirementIds?`, `tasks?`)
- `GET /api/plans/:id` — detail (plan + requirements + analyze grouped by kind + tasks with chunks + dependencies)
- `PATCH /api/plans/:id` — update title/description/status/codebaseId
- `DELETE /api/plans/:id`
- `POST /api/plans/:id/requirements` / `DELETE /api/plans/:id/requirements/:requirementId` / `POST /api/plans/:id/requirements/reorder`
- `GET /api/plans/:id/analyze` / `POST /api/plans/:id/analyze` / `PATCH /api/plans/:id/analyze/:itemId` / `DELETE /api/plans/:id/analyze/:itemId` / `POST /api/plans/:id/analyze/reorder`
- `POST /api/plans/:id/tasks` / `PATCH /api/plans/:id/tasks/:taskId` / `DELETE /api/plans/:id/tasks/:taskId` / `POST /api/plans/:id/tasks/reorder`
- `POST /api/plans/:id/tasks/:taskId/chunks` / `DELETE /api/plans/:id/tasks/:taskId/chunks/:linkId`
```

Delete the entire `### Sessions` subsection.

- [ ] **Step 4: Update the "## Web Pages" section**

- Remove the `/reviews` and `/reviews/:sessionId` entries
- Replace the `/plans/*` entries with:

```markdown
- `/plans` — list of plans with status pills and task progress
- `/plans/new` — simple form (title + description + optional codebase)
- `/plans/:id` — sticky header + four sections (Description, Requirements, Analyze, Tasks)
```

- [ ] **Step 5: Update the "## CLI Commands" section**

Replace any `fubbik plan *` lines with the new command list:

```markdown
- `fubbik plan create <title>` — create a plan
- `fubbik plan list` — list plans
- `fubbik plan show <id>` — show plan detail
- `fubbik plan status <id> <status>` — update plan status
- `fubbik plan add-task <planId> <title>` — add a task
- `fubbik plan task-done <planId> <taskId>` — mark a task done
- `fubbik plan link-requirement <planId> <requirementId>` — link a requirement
```

Remove any `fubbik session*` lines.

- [ ] **Step 6: Run the full CI pipeline**

```bash
pnpm ci
```

Expected: type-check, lint, test, build, format-check, sherif — all pass. Pre-existing unrelated type errors (e.g., in `broken-link-checker`, `dashboard`, `import`) are out of scope and may still exist. Fail the task only on NEW errors in files touched by this plan.

If any fail, fix them in this task (not a new commit) until clean.

- [ ] **Step 7: Manual smoke test**

Start dev:

```bash
pnpm dev
```

Navigate through these flows:

1. Open `/plans` — shows the three seeded plans with status pills
2. Click the in-progress plan — detail page shows sticky header, description, 1 requirement, analyze with items in all 5 kinds, 5 tasks with 1 done + 1 in_progress
3. Click the status pill — cycles to `completed`; click again — cycles back through the labels
4. Click the title — inline edit mode, edit, blur — title updates
5. In the Tasks section, click a task checkbox — toggles `done` state, visual line-through
6. In the Analyze section, expand Risks, click `+`, add a new risk with severity `high` — appears in the list
7. In Requirements, click `+ Add`, search for a requirement, click one — pill appears
8. In the header nav, verify: `Dashboard · Chunks · Graph · Plans` (primary), and Manage dropdown has `Requirements` in the top "Navigate" group (no Reviews entry)
9. `/reviews` should 404 (route deleted)

- [ ] **Step 8: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for plans-as-central-entity rewrite

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Deferred from spec (known follow-ups)

These items from the spec are intentionally deferred to a follow-up PR to keep this rewrite bounded:

- **Right rail** (Section 5) — the desktop-only `≥1280px` summary rail with status / progress ring / analyze counts / "jump to" anchor links. Not essential to functionality; the main column works standalone. Add in a follow-up.
- **Drag-to-reorder** in Requirements, Analyze, and Tasks sections — the reorder API routes exist (`POST .../reorder`) but no drag UI is wired. Add `@dnd-kit` integration in a follow-up.
- **Chunk picker integration** in analyze chunks + task chunks — v1 asks for a raw chunk ID. Wire the existing chunk picker component in a follow-up.
- **Acceptance criteria checklist UI** — v1 renders as plain bullets in the expanded task card. Make them toggleable (local/ephemeral state) in a follow-up.
- **Status transition via keyboard / command palette** — currently requires clicking the pill.
- **Keyboard shortcuts in Tasks section** (`j/k`, `space`, `n`, `enter`) — specced but not implemented in Task 16. Add as a follow-up.
