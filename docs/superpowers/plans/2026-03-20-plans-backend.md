# Plans Feature — Backend Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "plans" entity with ordered steps for AI agents to track implementation progress, connected to the knowledge graph.

**Architecture:** New `plan` and `plan_step` tables following the existing Repository → Service → Route pattern. Plans have a status (draft/active/completed/archived), steps have individual status (pending/in_progress/done/skipped/blocked) with ordering and optional chunk links. A `plan_chunk_ref` join table connects plans to chunks with a relation type (context/created/modified).

**Tech Stack:** Drizzle ORM (PostgreSQL), Effect (typed errors), Elysia (routes), Arktype (future validation)

**Codebase patterns to follow:**
- IDs: `crypto.randomUUID()` in service layer
- Timestamps: `timestamp().defaultNow().notNull()` + `.$onUpdate(() => new Date())`
- Repo functions return `Effect<T, DatabaseError>`
- Services use `Effect.gen(function* () { ... })` or `.pipe(Effect.flatMap(...))`
- Routes: `Effect.runPromise(requireSession(ctx).pipe(...))`
- Errors: `NotFoundError({ resource })`, `ValidationError({ message })`
- Schema exports: `packages/db/src/schema/index.ts`
- Repo exports: `packages/db/src/repository/index.ts`
- Route mounting: `.use(planRoutes)` in `packages/api/src/index.ts`

---

## File Structure

### New files:
- `packages/db/src/schema/plan.ts` — Plan + PlanStep + PlanChunkRef tables + relations
- `packages/db/src/repository/plan.ts` — CRUD for plans and steps
- `packages/api/src/plans/service.ts` — Business logic, validation
- `packages/api/src/plans/routes.ts` — Elysia HTTP handlers

### Files to modify:
- `packages/db/src/schema/index.ts` — Export plan schema
- `packages/db/src/repository/index.ts` — Export plan repository
- `packages/api/src/index.ts` — Mount plan routes

---

## Task 1: Database Schema

**Files:**
- Create: `packages/db/src/schema/plan.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create plan schema**

Read `packages/db/src/schema/requirement.ts` and `packages/db/src/schema/use-case.ts` first to match patterns exactly. Then create:

```ts
// packages/db/src/schema/plan.ts
import { pgTable, text, timestamp, integer, index, type AnyPgColumn } from "drizzle-orm/pg-core";
import { relations } from "drizzle-orm";
import { user } from "./auth";
import { codebase } from "./codebase";
import { chunk } from "./chunk";

export const plan = pgTable(
    "plan",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        description: text("description"),
        status: text("status").notNull().default("draft"), // draft | active | completed | archived
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull(),
    },
    (table) => [
        index("plan_userId_idx").on(table.userId),
        index("plan_status_idx").on(table.status),
    ]
);

export const planStep = pgTable(
    "plan_step",
    {
        id: text("id").primaryKey(),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        description: text("description").notNull(),
        status: text("status").notNull().default("pending"), // pending | in_progress | done | skipped | blocked
        order: integer("order").notNull().default(0),
        parentStepId: text("parent_step_id").references((): AnyPgColumn => planStep.id, { onDelete: "cascade" }),
        note: text("note"), // AI can add implementation notes
        chunkId: text("chunk_id").references(() => chunk.id, { onDelete: "set null" }), // optional link to a relevant chunk
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull(),
    },
    (table) => [
        index("plan_step_planId_idx").on(table.planId),
        index("plan_step_order_idx").on(table.planId, table.order),
    ]
);

export const planChunkRef = pgTable(
    "plan_chunk_ref",
    {
        id: text("id").primaryKey(),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        relation: text("relation").notNull().default("context"), // context | created | modified
        createdAt: timestamp("created_at").defaultNow().notNull(),
    },
    (table) => [
        index("plan_chunk_ref_planId_idx").on(table.planId),
        index("plan_chunk_ref_chunkId_idx").on(table.chunkId),
    ]
);

// Relations
export const planRelations = relations(plan, ({ one, many }) => ({
    user: one(user, { fields: [plan.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [plan.codebaseId], references: [codebase.id] }),
    steps: many(planStep),
    chunkRefs: many(planChunkRef),
}));

// NOTE: Self-referential relations require `relationName` on both sides to avoid Drizzle ambiguity errors.
// See use-case.ts for the correct pattern.
export const planStepRelations = relations(planStep, ({ one, many }) => ({
    plan: one(plan, { fields: [planStep.planId], references: [plan.id] }),
    parentStep: one(planStep, { fields: [planStep.parentStepId], references: [planStep.id], relationName: "children" }),
    children: many(planStep, { relationName: "children" }),
    chunk: one(chunk, { fields: [planStep.chunkId], references: [chunk.id] }),
}));

export const planChunkRefRelations = relations(planChunkRef, ({ one }) => ({
    plan: one(plan, { fields: [planChunkRef.planId], references: [plan.id] }),
    chunk: one(chunk, { fields: [planChunkRef.chunkId], references: [chunk.id] }),
}));
```

- [ ] **Step 2: Export from schema index**

In `packages/db/src/schema/index.ts`, add:
```ts
export * from "./plan";
```

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/plan.ts packages/db/src/schema/index.ts
git commit -m "feat: add plan, plan_step, plan_chunk_ref database schema"
```

---

## Task 2: Repository Layer

**Files:**
- Create: `packages/db/src/repository/plan.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create plan repository**

Read `packages/db/src/repository/use-case.ts` for the pattern. Create:

```ts
// packages/db/src/repository/plan.ts
import { Effect } from "effect";
import { db } from "../index";
import { plan, planStep, planChunkRef } from "../schema/plan";
import { eq, and, asc, desc } from "drizzle-orm";
import { DatabaseError } from "../errors";

// ─── Plan CRUD ───

export function createPlan(params: {
    id: string;
    title: string;
    description?: string;
    status?: string;
    userId: string;
    codebaseId?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(plan).values(params).returning();
            return created!;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function getPlanById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.id, id)];
            if (userId) conditions.push(eq(plan.userId, userId));
            const [found] = await db.select().from(plan).where(and(...conditions));
            return found ?? null;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function listPlans(userId: string, params?: { codebaseId?: string; status?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(plan.userId, userId)];
            if (params?.codebaseId) conditions.push(eq(plan.codebaseId, params.codebaseId));
            if (params?.status) conditions.push(eq(plan.status, params.status));
            return db.select().from(plan).where(and(...conditions)).orderBy(desc(plan.updatedAt));
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function updatePlan(id: string, userId: string, params: {
    title?: string;
    description?: string | null;
    status?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.title !== undefined) setClause.title = params.title;
            if (params.description !== undefined) setClause.description = params.description;
            if (params.status !== undefined) setClause.status = params.status;
            if (Object.keys(setClause).length === 0) {
                const [found] = await db.select().from(plan).where(and(eq(plan.id, id), eq(plan.userId, userId)));
                return found ?? null;
            }
            const [updated] = await db.update(plan).set(setClause).where(and(eq(plan.id, id), eq(plan.userId, userId))).returning();
            return updated ?? null;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function deletePlan(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db.delete(plan).where(and(eq(plan.id, id), eq(plan.userId, userId))).returning();
            return deleted ?? null;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

// ─── Steps CRUD ───

export function getStepsForPlan(planId: string) {
    return Effect.tryPromise({
        try: async () =>
            db.select().from(planStep).where(eq(planStep.planId, planId)).orderBy(asc(planStep.order)),
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function createStep(params: {
    id: string;
    planId: string;
    description: string;
    order: number;
    parentStepId?: string;
    chunkId?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(planStep).values(params).returning();
            return created!;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function updateStep(id: string, params: {
    description?: string;
    status?: string;
    order?: number;
    note?: string | null;
    chunkId?: string | null;
}) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.description !== undefined) setClause.description = params.description;
            if (params.status !== undefined) setClause.status = params.status;
            if (params.order !== undefined) setClause.order = params.order;
            if (params.note !== undefined) setClause.note = params.note;
            if (params.chunkId !== undefined) setClause.chunkId = params.chunkId;
            if (Object.keys(setClause).length === 0) return null;
            const [updated] = await db.update(planStep).set(setClause).where(eq(planStep.id, id)).returning();
            return updated ?? null;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function deleteStep(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db.delete(planStep).where(eq(planStep.id, id)).returning();
            return deleted ?? null;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

// ─── Chunk Refs ───

export function getChunkRefsForPlan(planId: string) {
    return Effect.tryPromise({
        try: async () =>
            db.select().from(planChunkRef).where(eq(planChunkRef.planId, planId)),
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function addChunkRef(params: { id: string; planId: string; chunkId: string; relation: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(planChunkRef).values(params).returning();
            return created!;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}

export function removeChunkRef(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db.delete(planChunkRef).where(eq(planChunkRef.id, id)).returning();
            return deleted ?? null;
        },
        catch: (cause) => new DatabaseError({ cause }),
    });
}
```

- [ ] **Step 2: Export from repository index**

In `packages/db/src/repository/index.ts`, add:
```ts
export * from "./plan";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/plan.ts packages/db/src/repository/index.ts
git commit -m "feat: add plan repository with CRUD for plans, steps, and chunk refs"
```

---

## Task 3: Service Layer

**Files:**
- Create: `packages/api/src/plans/service.ts`

- [ ] **Step 1: Create plan service**

Read `packages/api/src/use-cases/service.ts` for the pattern. Create:

```ts
// packages/api/src/plans/service.ts
import { Effect } from "effect";
import {
    createPlan as createPlanRepo,
    getPlanById,
    listPlans as listPlansRepo,
    updatePlan as updatePlanRepo,
    deletePlan as deletePlanRepo,
    getStepsForPlan,
    createStep as createStepRepo,
    updateStep as updateStepRepo,
    deleteStep as deleteStepRepo,
    getChunkRefsForPlan,
    addChunkRef as addChunkRefRepo,
    removeChunkRef as removeChunkRefRepo,
} from "@fubbik/db/repository";
import { NotFoundError, ValidationError } from "../errors";

const VALID_PLAN_STATUSES = ["draft", "active", "completed", "archived"];
const VALID_STEP_STATUSES = ["pending", "in_progress", "done", "skipped", "blocked"];
const VALID_REF_RELATIONS = ["context", "created", "modified"];

// ─── Plans ───

export function createPlan(userId: string, body: {
    title: string;
    description?: string;
    codebaseId?: string;
    steps?: Array<{ description: string; order?: number; parentStepId?: string; chunkId?: string }>;
}) {
    return Effect.gen(function* () {
        const planData = yield* createPlanRepo({
            id: crypto.randomUUID(),
            title: body.title,
            description: body.description,
            status: "draft",
            userId,
            codebaseId: body.codebaseId,
        });

        // Create initial steps if provided
        if (body.steps?.length) {
            for (let i = 0; i < body.steps.length; i++) {
                const step = body.steps[i]!;
                yield* createStepRepo({
                    id: crypto.randomUUID(),
                    planId: planData.id,
                    description: step.description,
                    order: step.order ?? i,
                    parentStepId: step.parentStepId,
                    chunkId: step.chunkId,
                });
            }
        }

        return yield* getPlanDetail(planData.id, userId);
    });
}

export function getPlanDetail(id: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const [steps, chunkRefs] = yield* Effect.all([
            getStepsForPlan(id),
            getChunkRefsForPlan(id),
        ]);

        // Compute progress
        const totalSteps = steps.length;
        const doneSteps = steps.filter(s => s.status === "done").length;
        const progress = totalSteps > 0 ? Math.round((doneSteps / totalSteps) * 100) : 0;

        return { ...found, steps, chunkRefs, progress };
    });
}

export function listPlans(userId: string, params?: { codebaseId?: string; status?: string }) {
    return Effect.gen(function* () {
        const plans = yield* listPlansRepo(userId, params);

        // Fetch step counts for each plan
        const plansWithProgress = yield* Effect.all(
            plans.map(p =>
                getStepsForPlan(p.id).pipe(
                    Effect.map(steps => {
                        const total = steps.length;
                        const done = steps.filter(s => s.status === "done").length;
                        return { ...p, stepCount: total, doneCount: done, progress: total > 0 ? Math.round((done / total) * 100) : 0 };
                    })
                )
            )
        );

        return plansWithProgress;
    });
}

export function updatePlan(id: string, userId: string, body: {
    title?: string;
    description?: string | null;
    status?: string;
}) {
    return Effect.gen(function* () {
        if (body.status && !VALID_PLAN_STATUSES.includes(body.status)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid status. Must be: ${VALID_PLAN_STATUSES.join(", ")}` }));
        }
        const updated = yield* updatePlanRepo(id, userId, body);
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));
        return updated;
    });
}

export function deletePlan(id: string, userId: string) {
    return deletePlanRepo(id, userId).pipe(
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed({ message: "Deleted" }) : Effect.fail(new NotFoundError({ resource: "Plan" }))
        )
    );
}

// ─── Steps ───

export function addStep(planId: string, userId: string, body: {
    description: string;
    order?: number;
    parentStepId?: string;
    chunkId?: string;
}) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        // Auto-assign order if not provided
        const steps = yield* getStepsForPlan(planId);
        const order = body.order ?? steps.length;

        return yield* createStepRepo({
            id: crypto.randomUUID(),
            planId,
            description: body.description,
            order,
            parentStepId: body.parentStepId,
            chunkId: body.chunkId,
        });
    });
}

export function updateStep(planId: string, stepId: string, userId: string, body: {
    description?: string;
    status?: string;
    order?: number;
    note?: string | null;
    chunkId?: string | null;
}) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        if (body.status && !VALID_STEP_STATUSES.includes(body.status)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid step status. Must be: ${VALID_STEP_STATUSES.join(", ")}` }));
        }

        const updated = yield* updateStepRepo(stepId, body);
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "Step" }));
        return updated;
    });
}

export function deleteStep(planId: string, stepId: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const deleted = yield* deleteStepRepo(stepId);
        if (!deleted) return yield* Effect.fail(new NotFoundError({ resource: "Step" }));
        return { message: "Deleted" };
    });
}

// ─── Chunk Refs ───

export function addPlanChunkRef(planId: string, userId: string, body: {
    chunkId: string;
    relation: string;
}) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        if (!VALID_REF_RELATIONS.includes(body.relation)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid relation. Must be: ${VALID_REF_RELATIONS.join(", ")}` }));
        }

        return yield* addChunkRefRepo({
            id: crypto.randomUUID(),
            planId,
            chunkId: body.chunkId,
            relation: body.relation,
        });
    });
}

export function removePlanChunkRef(planId: string, refId: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const deleted = yield* removeChunkRefRepo(refId);
        if (!deleted) return yield* Effect.fail(new NotFoundError({ resource: "ChunkRef" }));
        return { message: "Deleted" };
    });
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/plans/service.ts
git commit -m "feat: add plan service with CRUD, step management, and chunk refs"
```

---

## Task 4: API Routes

**Files:**
- Create: `packages/api/src/plans/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create plan routes**

Read `packages/api/src/use-cases/routes.ts` for the pattern. Create:

```ts
// packages/api/src/plans/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as planService from "./service";

export const planRoutes = new Elysia()
    // ─── Plans ───
    .get(
        "/plans",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.listPlans(session.user.id, ctx.query))
            )
        ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                status: t.Optional(t.String()),
            }),
        }
    )
    .post(
        "/plans",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.createPlan(session.user.id, ctx.body)),
                Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
            )
        ),
        {
            body: t.Object({
                title: t.String({ maxLength: 500 }),
                description: t.Optional(t.String({ maxLength: 5000 })),
                codebaseId: t.Optional(t.String()),
                steps: t.Optional(t.Array(t.Object({
                    description: t.String({ maxLength: 1000 }),
                    order: t.Optional(t.Number()),
                    parentStepId: t.Optional(t.String()),
                    chunkId: t.Optional(t.String()),
                }))),
            }),
        }
    )
    .get(
        "/plans/:id",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.getPlanDetail(ctx.params.id, session.user.id))
            )
        )
    )
    .patch(
        "/plans/:id",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.updatePlan(ctx.params.id, session.user.id, ctx.body))
            )
        ),
        {
            body: t.Object({
                title: t.Optional(t.String({ maxLength: 500 })),
                description: t.Optional(t.Union([t.String({ maxLength: 5000 }), t.Null()])),
                status: t.Optional(t.String()),
            }),
        }
    )
    .delete(
        "/plans/:id",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.deletePlan(ctx.params.id, session.user.id))
            )
        )
    )
    // ─── Steps ───
    .post(
        "/plans/:id/steps",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.addStep(ctx.params.id, session.user.id, ctx.body)),
                Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
            )
        ),
        {
            body: t.Object({
                description: t.String({ maxLength: 1000 }),
                order: t.Optional(t.Number()),
                parentStepId: t.Optional(t.String()),
                chunkId: t.Optional(t.String()),
            }),
        }
    )
    .patch(
        "/plans/:id/steps/:stepId",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.updateStep(ctx.params.id, ctx.params.stepId, session.user.id, ctx.body))
            )
        ),
        {
            body: t.Object({
                description: t.Optional(t.String({ maxLength: 1000 })),
                status: t.Optional(t.String()),
                order: t.Optional(t.Number()),
                note: t.Optional(t.Union([t.String({ maxLength: 2000 }), t.Null()])),
                chunkId: t.Optional(t.Union([t.String(), t.Null()])),
            }),
        }
    )
    .delete(
        "/plans/:id/steps/:stepId",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.deleteStep(ctx.params.id, ctx.params.stepId, session.user.id))
            )
        )
    )
    // ─── Chunk Refs ───
    .post(
        "/plans/:id/chunks",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.addPlanChunkRef(ctx.params.id, session.user.id, ctx.body)),
                Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
            )
        ),
        {
            body: t.Object({
                chunkId: t.String(),
                relation: t.String(),
            }),
        }
    )
    .delete(
        "/plans/:id/chunks/:refId",
        (ctx) => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session) => planService.removePlanChunkRef(ctx.params.id, ctx.params.refId, session.user.id))
            )
        )
    );
```

- [ ] **Step 2: Mount routes in API index**

In `packages/api/src/index.ts`, add:
```ts
import { planRoutes } from "./plans/routes";
// ... in the Elysia chain:
.use(planRoutes)
```

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/plans/routes.ts packages/api/src/index.ts
git commit -m "feat: add plan API routes (CRUD for plans, steps, chunk refs)"
```
