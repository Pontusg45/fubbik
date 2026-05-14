# Behavioral Specification Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-class Behavioral Specification Matrix system — abstract spec layer above BDD requirements, with two matrix layers (invariants × entities, contracts × actors), computed cell statuses, and a grid UI.

**Architecture:** Five new database tables (matrix, dimension, rule, cell, cell_requirement), a repository/service/route stack following the existing Effect-based patterns, a grid-based frontend with slide-over cell panel, MCP tools, and CLI commands.

**Tech Stack:** Drizzle (schema), Effect (service), Elysia (routes), TanStack Router + React Query (frontend), Commander.js (CLI), MCP SDK (tools)

---

### Task 1: Database Schema

**Files:**
- Create: `packages/db/src/schema/behavior-matrix.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Write schema test**

Create `packages/db/src/schema/__tests__/behavior-matrix.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { getTableColumns } from "drizzle-orm";
import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement
} from "../behavior-matrix";

describe("behavior-matrix schema", () => {
    it("behaviorMatrix has expected columns", () => {
        const cols = getTableColumns(behaviorMatrix);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("name");
        expect(cols).toHaveProperty("layer");
        expect(cols).toHaveProperty("description");
        expect(cols).toHaveProperty("codebaseId");
        expect(cols).toHaveProperty("userId");
        expect(cols).toHaveProperty("createdAt");
        expect(cols).toHaveProperty("updatedAt");
    });

    it("behaviorDimension has expected columns", () => {
        const cols = getTableColumns(behaviorDimension);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("matrixId");
        expect(cols).toHaveProperty("name");
        expect(cols).toHaveProperty("order");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorRule has expected columns", () => {
        const cols = getTableColumns(behaviorRule);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("matrixId");
        expect(cols).toHaveProperty("title");
        expect(cols).toHaveProperty("description");
        expect(cols).toHaveProperty("category");
        expect(cols).toHaveProperty("order");
        expect(cols).toHaveProperty("createdAt");
        expect(cols).toHaveProperty("updatedAt");
    });

    it("behaviorCell has expected columns", () => {
        const cols = getTableColumns(behaviorCell);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("ruleId");
        expect(cols).toHaveProperty("dimensionId");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorCellRequirement has expected columns", () => {
        const cols = getTableColumns(behaviorCellRequirement);
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("requirementId");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/schema/__tests__/behavior-matrix.test.ts`
Expected: FAIL — cannot resolve `"../behavior-matrix"`

- [ ] **Step 3: Create the schema file**

Create `packages/db/src/schema/behavior-matrix.ts`:

```typescript
import { relations } from "drizzle-orm";
import { index, integer, pgTable, primaryKey, text, timestamp, unique } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { codebase } from "./codebase";
import { requirement } from "./requirement";

export const behaviorMatrix = pgTable(
    "behavior_matrix",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        layer: text("layer").notNull(),
        description: text("description"),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("behavior_matrix_userId_idx").on(table.userId),
        index("behavior_matrix_layer_idx").on(table.layer)
    ]
);

export const behaviorDimension = pgTable(
    "behavior_dimension",
    {
        id: text("id").primaryKey(),
        matrixId: text("matrix_id")
            .notNull()
            .references(() => behaviorMatrix.id, { onDelete: "cascade" }),
        name: text("name").notNull(),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        unique("behavior_dimension_matrix_name").on(table.matrixId, table.name),
        index("behavior_dimension_matrixId_idx").on(table.matrixId)
    ]
);

export const behaviorRule = pgTable(
    "behavior_rule",
    {
        id: text("id").primaryKey(),
        matrixId: text("matrix_id")
            .notNull()
            .references(() => behaviorMatrix.id, { onDelete: "cascade" }),
        title: text("title").notNull(),
        description: text("description"),
        category: text("category"),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("behavior_rule_matrixId_idx").on(table.matrixId)
    ]
);

export const behaviorCell = pgTable(
    "behavior_cell",
    {
        id: text("id").primaryKey(),
        ruleId: text("rule_id")
            .notNull()
            .references(() => behaviorRule.id, { onDelete: "cascade" }),
        dimensionId: text("dimension_id")
            .notNull()
            .references(() => behaviorDimension.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        unique("behavior_cell_rule_dimension").on(table.ruleId, table.dimensionId),
        index("behavior_cell_ruleId_idx").on(table.ruleId),
        index("behavior_cell_dimensionId_idx").on(table.dimensionId)
    ]
);

export const behaviorCellRequirement = pgTable(
    "behavior_cell_requirement",
    {
        cellId: text("cell_id")
            .notNull()
            .references(() => behaviorCell.id, { onDelete: "cascade" }),
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.cellId, table.requirementId] })]
);

export const behaviorMatrixRelations = relations(behaviorMatrix, ({ one, many }) => ({
    user: one(user, { fields: [behaviorMatrix.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [behaviorMatrix.codebaseId], references: [codebase.id] }),
    dimensions: many(behaviorDimension),
    rules: many(behaviorRule)
}));

export const behaviorDimensionRelations = relations(behaviorDimension, ({ one, many }) => ({
    matrix: one(behaviorMatrix, { fields: [behaviorDimension.matrixId], references: [behaviorMatrix.id] }),
    cells: many(behaviorCell)
}));

export const behaviorRuleRelations = relations(behaviorRule, ({ one, many }) => ({
    matrix: one(behaviorMatrix, { fields: [behaviorRule.matrixId], references: [behaviorMatrix.id] }),
    cells: many(behaviorCell)
}));

export const behaviorCellRelations = relations(behaviorCell, ({ one, many }) => ({
    rule: one(behaviorRule, { fields: [behaviorCell.ruleId], references: [behaviorRule.id] }),
    dimension: one(behaviorDimension, { fields: [behaviorCell.dimensionId], references: [behaviorDimension.id] }),
    cellRequirements: many(behaviorCellRequirement)
}));

export const behaviorCellRequirementRelations = relations(behaviorCellRequirement, ({ one }) => ({
    cell: one(behaviorCell, { fields: [behaviorCellRequirement.cellId], references: [behaviorCell.id] }),
    requirement: one(requirement, { fields: [behaviorCellRequirement.requirementId], references: [requirement.id] })
}));
```

- [ ] **Step 4: Export from schema index**

Add to `packages/db/src/schema/index.ts`:

```typescript
export * from "./behavior-matrix";
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/schema/__tests__/behavior-matrix.test.ts`
Expected: PASS — all 5 assertions green

- [ ] **Step 6: Push schema to database**

Run: `pnpm db:push`
Expected: Tables `behavior_matrix`, `behavior_dimension`, `behavior_rule`, `behavior_cell`, `behavior_cell_requirement` created

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/schema/behavior-matrix.ts packages/db/src/schema/index.ts packages/db/src/schema/__tests__/behavior-matrix.test.ts
git commit -m "feat: add behavioral matrix database schema"
```

---

### Task 2: Repository

**Files:**
- Create: `packages/db/src/repository/behavior-matrix.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the repository file**

Create `packages/db/src/repository/behavior-matrix.ts`:

```typescript
import { and, eq, sql, inArray, asc } from "drizzle-orm";

import { db, dbEffect } from "../index";
import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement
} from "../schema/behavior-matrix";
import { requirement } from "../schema/requirement";

// --- Matrix CRUD ---

export function createMatrix(params: {
    id: string;
    name: string;
    layer: string;
    description?: string;
    codebaseId?: string;
    userId: string;
}) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorMatrix).values(params).returning();
        return created!;
    });
}

export function getMatrixById(id: string, userId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(behaviorMatrix)
            .where(and(eq(behaviorMatrix.id, id), eq(behaviorMatrix.userId, userId)));
        return found ?? null;
    });
}

export function listMatrices(userId: string, filters?: { codebaseId?: string; layer?: string }) {
    return dbEffect(async () => {
        const conditions = [eq(behaviorMatrix.userId, userId)];
        if (filters?.codebaseId) conditions.push(eq(behaviorMatrix.codebaseId, filters.codebaseId));
        if (filters?.layer) conditions.push(eq(behaviorMatrix.layer, filters.layer));

        return db
            .select()
            .from(behaviorMatrix)
            .where(and(...conditions))
            .orderBy(asc(behaviorMatrix.name));
    });
}

export function updateMatrix(id: string, userId: string, data: { name?: string; description?: string | null }) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorMatrix)
            .set(data)
            .where(and(eq(behaviorMatrix.id, id), eq(behaviorMatrix.userId, userId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteMatrix(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorMatrix)
            .where(and(eq(behaviorMatrix.id, id), eq(behaviorMatrix.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

// --- Dimension CRUD ---

export function createDimension(params: { id: string; matrixId: string; name: string; order: number }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorDimension).values(params).returning();
        return created!;
    });
}

export function updateDimension(id: string, matrixId: string, data: { name?: string }) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorDimension)
            .set(data)
            .where(and(eq(behaviorDimension.id, id), eq(behaviorDimension.matrixId, matrixId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteDimension(id: string, matrixId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorDimension)
            .where(and(eq(behaviorDimension.id, id), eq(behaviorDimension.matrixId, matrixId)))
            .returning();
        return deleted ?? null;
    });
}

export function getDimensionsForMatrix(matrixId: string) {
    return dbEffect(() =>
        db
            .select()
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId))
            .orderBy(asc(behaviorDimension.order))
    );
}

export function getMaxDimensionOrder(matrixId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ max: sql<number>`coalesce(max(${behaviorDimension.order}), -1)::int` })
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId));
        return result?.max ?? -1;
    });
}

export function reorderDimensions(dimensionIds: string[]) {
    return dbEffect(async () => {
        for (let i = 0; i < dimensionIds.length; i++) {
            await db
                .update(behaviorDimension)
                .set({ order: i })
                .where(eq(behaviorDimension.id, dimensionIds[i]!));
        }
    });
}

// --- Rule CRUD ---

export function createRule(params: {
    id: string;
    matrixId: string;
    title: string;
    description?: string;
    category?: string;
    order: number;
}) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorRule).values(params).returning();
        return created!;
    });
}

export function updateRule(id: string, matrixId: string, data: { title?: string; description?: string | null; category?: string | null }) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorRule)
            .set(data)
            .where(and(eq(behaviorRule.id, id), eq(behaviorRule.matrixId, matrixId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteRule(id: string, matrixId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorRule)
            .where(and(eq(behaviorRule.id, id), eq(behaviorRule.matrixId, matrixId)))
            .returning();
        return deleted ?? null;
    });
}

export function getRulesForMatrix(matrixId: string) {
    return dbEffect(() =>
        db
            .select()
            .from(behaviorRule)
            .where(eq(behaviorRule.matrixId, matrixId))
            .orderBy(asc(behaviorRule.order))
    );
}

export function getMaxRuleOrder(matrixId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ max: sql<number>`coalesce(max(${behaviorRule.order}), -1)::int` })
            .from(behaviorRule)
            .where(eq(behaviorRule.matrixId, matrixId));
        return result?.max ?? -1;
    });
}

export function reorderRules(ruleIds: string[]) {
    return dbEffect(async () => {
        for (let i = 0; i < ruleIds.length; i++) {
            await db
                .update(behaviorRule)
                .set({ order: i })
                .where(eq(behaviorRule.id, ruleIds[i]!));
        }
    });
}

// --- Cell CRUD ---

export function getCellByRuleDimension(ruleId: string, dimensionId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(behaviorCell)
            .where(and(eq(behaviorCell.ruleId, ruleId), eq(behaviorCell.dimensionId, dimensionId)));
        return found ?? null;
    });
}

export function createCell(params: { id: string; ruleId: string; dimensionId: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorCell).values(params).returning();
        return created!;
    });
}

export function deleteCell(id: string) {
    return dbEffect(async () => {
        const [deleted] = await db.delete(behaviorCell).where(eq(behaviorCell.id, id)).returning();
        return deleted ?? null;
    });
}

export function getCellRequirementCount(cellId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ count: sql<number>`count(*)::int` })
            .from(behaviorCellRequirement)
            .where(eq(behaviorCellRequirement.cellId, cellId));
        return result?.count ?? 0;
    });
}

// --- Cell Requirement Links ---

export function linkCellRequirement(cellId: string, requirementId: string) {
    return dbEffect(async () => {
        const [created] = await db
            .insert(behaviorCellRequirement)
            .values({ cellId, requirementId })
            .onConflictDoNothing()
            .returning();
        return created ?? null;
    });
}

export function unlinkCellRequirement(cellId: string, requirementId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorCellRequirement)
            .where(and(
                eq(behaviorCellRequirement.cellId, cellId),
                eq(behaviorCellRequirement.requirementId, requirementId)
            ))
            .returning();
        return deleted ?? null;
    });
}

export function getRequirementsForCell(cellId: string) {
    return dbEffect(() =>
        db
            .select({
                requirementId: behaviorCellRequirement.requirementId,
                title: requirement.title,
                status: requirement.status
            })
            .from(behaviorCellRequirement)
            .innerJoin(requirement, eq(behaviorCellRequirement.requirementId, requirement.id))
            .where(eq(behaviorCellRequirement.cellId, cellId))
    );
}

// --- Matrix View (full grid query) ---

export function getMatrixView(matrixId: string) {
    return dbEffect(async () => {
        const dimensions = await db
            .select()
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId))
            .orderBy(asc(behaviorDimension.order));

        const rules = await db
            .select()
            .from(behaviorRule)
            .where(eq(behaviorRule.matrixId, matrixId))
            .orderBy(asc(behaviorRule.order));

        const cells = await db
            .select({
                id: behaviorCell.id,
                ruleId: behaviorCell.ruleId,
                dimensionId: behaviorCell.dimensionId,
                requirementCount: sql<number>`count(${behaviorCellRequirement.requirementId})::int`.as("requirement_count"),
                failingCount: sql<number>`count(case when ${requirement.status} = 'failing' then 1 end)::int`.as("failing_count")
            })
            .from(behaviorCell)
            .leftJoin(behaviorCellRequirement, eq(behaviorCellRequirement.cellId, behaviorCell.id))
            .leftJoin(requirement, eq(behaviorCellRequirement.requirementId, requirement.id))
            .where(
                inArray(
                    behaviorCell.ruleId,
                    rules.map(r => r.id)
                )
            )
            .groupBy(behaviorCell.id, behaviorCell.ruleId, behaviorCell.dimensionId);

        return { dimensions, rules, cells };
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:

```typescript
export * from "./behavior-matrix";
```

- [ ] **Step 3: Run the schema test to verify nothing broke**

Run: `cd packages/db && pnpm vitest run src/schema/__tests__/behavior-matrix.test.ts`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/behavior-matrix.ts packages/db/src/repository/index.ts
git commit -m "feat: add behavioral matrix repository"
```

---

### Task 3: Service Layer

**Files:**
- Create: `packages/api/src/matrices/service.ts`

- [ ] **Step 1: Write service tests**

Create `packages/api/src/matrices/service.test.ts`:

```typescript
import { describe, expect, it, vi, beforeEach } from "vitest";
import { Effect } from "effect";

vi.mock("@fubbik/db/repository", () => ({
    createMatrix: vi.fn(),
    getMatrixById: vi.fn(),
    listMatrices: vi.fn(),
    updateMatrix: vi.fn(),
    deleteMatrix: vi.fn(),
    createDimension: vi.fn(),
    updateDimension: vi.fn(),
    deleteDimension: vi.fn(),
    getDimensionsForMatrix: vi.fn(),
    getMaxDimensionOrder: vi.fn(),
    reorderDimensions: vi.fn(),
    createRule: vi.fn(),
    updateRule: vi.fn(),
    deleteRule: vi.fn(),
    getRulesForMatrix: vi.fn(),
    getMaxRuleOrder: vi.fn(),
    reorderRules: vi.fn(),
    getCellByRuleDimension: vi.fn(),
    createCell: vi.fn(),
    deleteCell: vi.fn(),
    getCellRequirementCount: vi.fn(),
    linkCellRequirement: vi.fn(),
    unlinkCellRequirement: vi.fn(),
    getRequirementsForCell: vi.fn(),
    getMatrixView: vi.fn()
}));

import * as repo from "@fubbik/db/repository";
import * as service from "./service";

const mockRepo = repo as unknown as Record<string, ReturnType<typeof vi.fn>>;

function mockMatrix(overrides?: Record<string, unknown>) {
    return {
        id: "mat-1",
        name: "Domain Invariants",
        layer: "invariant",
        description: null,
        codebaseId: null,
        userId: "user-1",
        createdAt: new Date(),
        updatedAt: new Date(),
        ...overrides
    };
}

beforeEach(() => vi.clearAllMocks());

describe("createMatrix", () => {
    it("creates a matrix and returns it", async () => {
        const mat = mockMatrix();
        mockRepo.createMatrix.mockReturnValue(Effect.succeed(mat));

        const result = await Effect.runPromise(service.createMatrix("user-1", {
            name: "Domain Invariants",
            layer: "invariant"
        }));

        expect(result).toMatchObject({ name: "Domain Invariants", layer: "invariant" });
        expect(mockRepo.createMatrix).toHaveBeenCalledOnce();
    });
});

describe("getMatrixDetail", () => {
    it("returns matrix with dimensions and rules", async () => {
        mockRepo.getMatrixById.mockReturnValue(Effect.succeed(mockMatrix()));
        mockRepo.getDimensionsForMatrix.mockReturnValue(Effect.succeed([{ id: "dim-1", name: "Chunk" }]));
        mockRepo.getRulesForMatrix.mockReturnValue(Effect.succeed([{ id: "rule-1", title: "Cascade deletes" }]));

        const result = await Effect.runPromise(service.getMatrixDetail("mat-1", "user-1"));

        expect(result.matrix.name).toBe("Domain Invariants");
        expect(result.dimensions).toHaveLength(1);
        expect(result.rules).toHaveLength(1);
    });

    it("fails with NotFoundError for missing matrix", async () => {
        mockRepo.getMatrixById.mockReturnValue(Effect.succeed(null));

        await expect(Effect.runPromise(service.getMatrixDetail("nope", "user-1"))).rejects.toThrow();
    });
});

describe("toggleCell", () => {
    it("creates cell when none exists", async () => {
        mockRepo.getCellByRuleDimension.mockReturnValue(Effect.succeed(null));
        mockRepo.createCell.mockReturnValue(Effect.succeed({ id: "cell-1", ruleId: "rule-1", dimensionId: "dim-1" }));

        const result = await Effect.runPromise(service.toggleCell("rule-1", "dim-1"));

        expect(result).toMatchObject({ action: "created" });
        expect(mockRepo.createCell).toHaveBeenCalledOnce();
    });

    it("deletes cell with no linked requirements", async () => {
        mockRepo.getCellByRuleDimension.mockReturnValue(Effect.succeed({ id: "cell-1" }));
        mockRepo.getCellRequirementCount.mockReturnValue(Effect.succeed(0));
        mockRepo.deleteCell.mockReturnValue(Effect.succeed({ id: "cell-1" }));

        const result = await Effect.runPromise(service.toggleCell("rule-1", "dim-1"));

        expect(result).toMatchObject({ action: "deleted" });
    });

    it("fails when cell has linked requirements", async () => {
        mockRepo.getCellByRuleDimension.mockReturnValue(Effect.succeed({ id: "cell-1" }));
        mockRepo.getCellRequirementCount.mockReturnValue(Effect.succeed(2));

        await expect(Effect.runPromise(service.toggleCell("rule-1", "dim-1"))).rejects.toThrow();
    });
});

describe("getMatrixView", () => {
    it("computes cell statuses correctly", async () => {
        mockRepo.getMatrixById.mockReturnValue(Effect.succeed(mockMatrix()));
        mockRepo.getMatrixView.mockReturnValue(Effect.succeed({
            dimensions: [{ id: "dim-1", name: "Chunk", order: 0 }],
            rules: [{ id: "rule-1", title: "Cascade", category: null, order: 0 }],
            cells: [
                { id: "c1", ruleId: "rule-1", dimensionId: "dim-1", requirementCount: 2, failingCount: 0 },
                { id: "c2", ruleId: "rule-1", dimensionId: "dim-2", requirementCount: 0, failingCount: 0 },
                { id: "c3", ruleId: "rule-1", dimensionId: "dim-3", requirementCount: 3, failingCount: 1 }
            ]
        }));

        const result = await Effect.runPromise(service.getMatrixViewService("mat-1", "user-1"));

        const cells = result.cells;
        expect(cells["rule-1:dim-1"]?.status).toBe("specified");
        expect(cells["rule-1:dim-2"]?.status).toBe("unspecified");
        expect(cells["rule-1:dim-3"]?.status).toBe("violated");
        expect(result.summary.specified).toBe(1);
        expect(result.summary.unspecified).toBe(1);
        expect(result.summary.violated).toBe(1);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/matrices/service.test.ts`
Expected: FAIL — cannot resolve `"./service"`

- [ ] **Step 3: Create the service file**

Create `packages/api/src/matrices/service.ts`:

```typescript
import {
    createMatrix as createMatrixRepo,
    getMatrixById,
    listMatrices as listMatricesRepo,
    updateMatrix as updateMatrixRepo,
    deleteMatrix as deleteMatrixRepo,
    createDimension as createDimensionRepo,
    updateDimension as updateDimensionRepo,
    deleteDimension as deleteDimensionRepo,
    getDimensionsForMatrix,
    getMaxDimensionOrder,
    reorderDimensions as reorderDimensionsRepo,
    createRule as createRuleRepo,
    updateRule as updateRuleRepo,
    deleteRule as deleteRuleRepo,
    getRulesForMatrix,
    getMaxRuleOrder,
    reorderRules as reorderRulesRepo,
    getCellByRuleDimension,
    createCell as createCellRepo,
    deleteCell as deleteCellRepo,
    getCellRequirementCount,
    linkCellRequirement as linkCellRequirementRepo,
    unlinkCellRequirement as unlinkCellRequirementRepo,
    getRequirementsForCell as getRequirementsForCellRepo,
    getMatrixView
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

// --- Matrix ---

export function createMatrix(userId: string, body: {
    name: string;
    layer: string;
    description?: string;
    codebaseId?: string;
}) {
    if (body.layer !== "invariant" && body.layer !== "contract") {
        return Effect.fail(new ValidationError({ message: "Layer must be 'invariant' or 'contract'" }));
    }
    const id = crypto.randomUUID();
    return createMatrixRepo({ id, ...body, userId });
}

export function getMatrixDetail(matrixId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(matrix =>
            Effect.all({
                matrix: Effect.succeed(matrix),
                dimensions: getDimensionsForMatrix(matrixId),
                rules: getRulesForMatrix(matrixId)
            })
        )
    );
}

export function listMatrices(userId: string, filters?: { codebaseId?: string; layer?: string }) {
    return listMatricesRepo(userId, filters);
}

export function updateMatrix(matrixId: string, userId: string, body: { name?: string; description?: string | null }) {
    return updateMatrixRepo(matrixId, userId, body).pipe(
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Matrix" }))))
    );
}

export function deleteMatrixService(matrixId: string, userId: string) {
    return deleteMatrixRepo(matrixId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Matrix" }))))
    );
}

// --- Dimensions ---

export function addDimension(matrixId: string, userId: string, body: { name: string }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getMaxDimensionOrder(matrixId)),
        Effect.flatMap(maxOrder => createDimensionRepo({
            id: crypto.randomUUID(),
            matrixId,
            name: body.name,
            order: maxOrder + 1
        }))
    );
}

export function renameDimension(matrixId: string, dimId: string, userId: string, body: { name: string }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => updateDimensionRepo(dimId, matrixId, body)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Dimension" }))))
    );
}

export function removeDimension(matrixId: string, dimId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => deleteDimensionRepo(dimId, matrixId)),
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Dimension" }))))
    );
}

export function reorderDimensions(matrixId: string, userId: string, dimensionIds: string[]) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => reorderDimensionsRepo(dimensionIds)),
        Effect.map(() => ({ message: "Reordered" }))
    );
}

// --- Rules ---

export function addRule(matrixId: string, userId: string, body: { title: string; description?: string; category?: string }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getMaxRuleOrder(matrixId)),
        Effect.flatMap(maxOrder => createRuleRepo({
            id: crypto.randomUUID(),
            matrixId,
            title: body.title,
            description: body.description,
            category: body.category,
            order: maxOrder + 1
        }))
    );
}

export function updateRule(matrixId: string, ruleId: string, userId: string, body: { title?: string; description?: string | null; category?: string | null }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => updateRuleRepo(ruleId, matrixId, body)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Rule" }))))
    );
}

export function removeRule(matrixId: string, ruleId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => deleteRuleRepo(ruleId, matrixId)),
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Rule" }))))
    );
}

export function reorderRules(matrixId: string, userId: string, ruleIds: string[]) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => reorderRulesRepo(ruleIds)),
        Effect.map(() => ({ message: "Reordered" }))
    );
}

// --- Cells ---

export function toggleCell(ruleId: string, dimensionId: string) {
    return getCellByRuleDimension(ruleId, dimensionId).pipe(
        Effect.flatMap(existing => {
            if (!existing) {
                return createCellRepo({ id: crypto.randomUUID(), ruleId, dimensionId }).pipe(
                    Effect.map(cell => ({ action: "created" as const, cell }))
                );
            }
            return getCellRequirementCount(existing.id).pipe(
                Effect.flatMap(count => {
                    if (count > 0) {
                        return Effect.fail(new ValidationError({
                            message: `Cell has ${count} linked requirement(s). Unlink them first.`
                        }));
                    }
                    return deleteCellRepo(existing.id).pipe(
                        Effect.map(() => ({ action: "deleted" as const, cell: existing }))
                    );
                })
            );
        })
    );
}

export function linkRequirementToCell(cellId: string, requirementId: string) {
    return linkCellRequirementRepo(cellId, requirementId);
}

export function unlinkRequirementFromCell(cellId: string, requirementId: string) {
    return unlinkCellRequirementRepo(cellId, requirementId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Cell-Requirement link" }))))
    );
}

export function getRequirementsForCell(cellId: string) {
    return getRequirementsForCellRepo(cellId);
}

// --- Matrix View ---

type CellStatus = "specified" | "unspecified" | "violated";

interface ViewCell {
    id: string;
    status: CellStatus;
    requirementCount: number;
}

export function getMatrixViewService(matrixId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(matrix =>
            getMatrixView(matrixId).pipe(
                Effect.map(({ dimensions, rules, cells }) => {
                    const cellMap: Record<string, ViewCell | null> = {};
                    let specified = 0;
                    let unspecified = 0;
                    let violated = 0;

                    for (const cell of cells) {
                        const key = `${cell.ruleId}:${cell.dimensionId}`;
                        let status: CellStatus;
                        if (cell.failingCount > 0) {
                            status = "violated";
                            violated++;
                        } else if (cell.requirementCount > 0) {
                            status = "specified";
                            specified++;
                        } else {
                            status = "unspecified";
                            unspecified++;
                        }
                        cellMap[key] = { id: cell.id, status, requirementCount: cell.requirementCount };
                    }

                    return {
                        matrix,
                        dimensions,
                        rules,
                        cells: cellMap,
                        summary: {
                            specified,
                            unspecified,
                            violated,
                            total: specified + unspecified + violated
                        }
                    };
                })
            )
        )
    );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/api && pnpm vitest run src/matrices/service.test.ts`
Expected: PASS — all 5 tests green

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/matrices/service.ts packages/api/src/matrices/service.test.ts
git commit -m "feat: add behavioral matrix service with tests"
```

---

### Task 4: API Routes

**Files:**
- Create: `packages/api/src/matrices/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create the routes file**

Create `packages/api/src/matrices/routes.ts`:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as matrixService from "./service";

export const matrixRoutes = new Elysia()
    // --- Matrix CRUD ---
    .get(
        "/matrices",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        matrixService.listMatrices(session.user.id, {
                            codebaseId: ctx.query.codebaseId,
                            layer: ctx.query.layer
                        })
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                layer: t.Optional(t.String())
            })
        }
    )
    .post(
        "/matrices",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.createMatrix(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 200 }),
                layer: t.Union([t.Literal("invariant"), t.Literal("contract")]),
                description: t.Optional(t.String({ maxLength: 1000 })),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get("/matrices/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.getMatrixDetail(ctx.params.id, session.user.id))
            )
        )
    )
    .get("/matrices/:id/view", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.getMatrixViewService(ctx.params.id, session.user.id))
            )
        )
    )
    .patch(
        "/matrices/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.updateMatrix(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()]))
            })
        }
    )
    .delete("/matrices/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.deleteMatrixService(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    // --- Dimensions ---
    .post(
        "/matrices/:id/dimensions",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.addDimension(ctx.params.id, session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 })
            })
        }
    )
    .patch(
        "/matrices/:id/dimensions/:dimId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.renameDimension(ctx.params.id, ctx.params.dimId, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 })
            })
        }
    )
    .delete("/matrices/:id/dimensions/:dimId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.removeDimension(ctx.params.id, ctx.params.dimId, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post(
        "/matrices/:id/dimensions/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.reorderDimensions(ctx.params.id, session.user.id, ctx.body.dimensionIds))
                )
            ),
        {
            body: t.Object({
                dimensionIds: t.Array(t.String())
            })
        }
    )
    // --- Rules ---
    .post(
        "/matrices/:id/rules",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.addRule(ctx.params.id, session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                title: t.String({ maxLength: 200 }),
                description: t.Optional(t.String({ maxLength: 1000 })),
                category: t.Optional(t.String({ maxLength: 100 }))
            })
        }
    )
    .patch(
        "/matrices/:id/rules/:ruleId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.updateRule(ctx.params.id, ctx.params.ruleId, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                title: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
                category: t.Optional(t.Union([t.String({ maxLength: 100 }), t.Null()]))
            })
        }
    )
    .delete("/matrices/:id/rules/:ruleId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => matrixService.removeRule(ctx.params.id, ctx.params.ruleId, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post(
        "/matrices/:id/rules/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => matrixService.reorderRules(ctx.params.id, session.user.id, ctx.body.ruleIds))
                )
            ),
        {
            body: t.Object({
                ruleIds: t.Array(t.String())
            })
        }
    )
    // --- Cells ---
    .put(
        "/matrices/:id/cells",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => matrixService.toggleCell(ctx.body.ruleId, ctx.body.dimensionId))
                )
            ),
        {
            body: t.Object({
                ruleId: t.String(),
                dimensionId: t.String()
            })
        }
    )
    .post(
        "/matrices/:id/cells/:cellId/requirements",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => matrixService.linkRequirementToCell(ctx.params.cellId, ctx.body.requirementId)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                requirementId: t.String()
            })
        }
    )
    .delete("/matrices/:id/cells/:cellId/requirements/:reqId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => matrixService.unlinkRequirementFromCell(ctx.params.cellId, ctx.params.reqId)),
                Effect.map(() => ({ message: "Unlinked" }))
            )
        )
    )
    .get("/matrices/:id/cells/:cellId/requirements", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => matrixService.getRequirementsForCell(ctx.params.cellId))
            )
        )
    );
```

- [ ] **Step 2: Register routes in API index**

In `packages/api/src/index.ts`, add the import alongside other imports:

```typescript
import { matrixRoutes } from "./matrices/routes";
```

Add `.use(matrixRoutes)` to the `extendedRoutes` group:

```typescript
const extendedRoutes = new Elysia()
    // ... existing routes ...
    .use(matrixRoutes)
    // ... rest
```

- [ ] **Step 3: Run type check**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/matrices/routes.ts packages/api/src/index.ts
git commit -m "feat: add behavioral matrix API routes"
```

---

### Task 5: Frontend — List Page

**Files:**
- Create: `apps/web/src/routes/matrices.tsx`

- [ ] **Step 1: Create the matrices list page**

Create `apps/web/src/routes/matrices.tsx`:

```tsx
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Grid3X3, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { PageContainer, PageEmpty, PageHeader, PageLoading } from "~/components/ui/page";
import { Badge } from "~/components/ui/badge";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "~/components/ui/dialog";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "~/components/ui/select";
import { useApiQuery } from "~/hooks/use-api-query";
import { api } from "~/utils/api";
import { unwrapEden } from "~/utils/eden";

export const Route = createFileRoute("/matrices")({
    component: MatricesPage,
    beforeLoad: ({ context }) => {
        if (!context.session) throw new Error("Not authenticated");
    }
});

function MatricesPage() {
    const queryClient = useQueryClient();
    const navigate = useNavigate();
    const [createOpen, setCreateOpen] = useState(false);
    const [name, setName] = useState("");
    const [layer, setLayer] = useState<"invariant" | "contract">("invariant");
    const [description, setDescription] = useState("");

    const matricesQuery = useApiQuery<Array<{
        id: string;
        name: string;
        layer: string;
        description: string | null;
        codebaseId: string | null;
        createdAt: string;
        updatedAt: string;
    }>>({
        queryKey: ["matrices"],
        queryFn: () => api.api.matrices.get({ query: {} }),
        fallback: []
    });

    const createMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.matrices.post({
                name,
                layer,
                description: description || undefined
            }));
        },
        onSuccess: (data) => {
            queryClient.invalidateQueries({ queryKey: ["matrices"] });
            setCreateOpen(false);
            setName("");
            setDescription("");
            toast.success("Matrix created");
            navigate({ to: "/matrices/$matrixId", params: { matrixId: data.id } });
        },
        onError: () => toast.error("Failed to create matrix")
    });

    const matrices = matricesQuery.data ?? [];

    return (
        <PageContainer maxWidth="4xl">
            <PageHeader
                icon={Grid3X3}
                title="Behavioral Matrices"
                count={matrices.length}
                actions={
                    <Button size="sm" onClick={() => setCreateOpen(true)}>
                        <Plus className="mr-1 h-4 w-4" />
                        New Matrix
                    </Button>
                }
            />

            {matricesQuery.isLoading ? (
                <PageLoading count={3} />
            ) : matrices.length === 0 ? (
                <PageEmpty
                    icon={Grid3X3}
                    title="No matrices yet"
                    description="Create a behavioral matrix to define what your system should do."
                    action={
                        <Button onClick={() => setCreateOpen(true)}>
                            <Plus className="mr-1 h-4 w-4" />
                            Create Matrix
                        </Button>
                    }
                />
            ) : (
                <div className="space-y-3">
                    {matrices.map(matrix => (
                        <Link
                            key={matrix.id}
                            to="/matrices/$matrixId"
                            params={{ matrixId: matrix.id }}
                            className="border-border hover:border-primary/30 bg-card block rounded-lg border p-4 transition-colors"
                        >
                            <div className="flex items-center justify-between">
                                <div className="flex items-center gap-3">
                                    <h3 className="font-medium">{matrix.name}</h3>
                                    <Badge variant={matrix.layer === "invariant" ? "secondary" : "outline"}>
                                        {matrix.layer}
                                    </Badge>
                                </div>
                            </div>
                            {matrix.description && (
                                <p className="text-muted-foreground mt-1 text-sm">{matrix.description}</p>
                            )}
                        </Link>
                    ))}
                </div>
            )}

            <Dialog open={createOpen} onOpenChange={setCreateOpen}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>New Behavioral Matrix</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div>
                            <Label htmlFor="matrix-name">Name</Label>
                            <Input
                                id="matrix-name"
                                value={name}
                                onChange={e => setName(e.target.value)}
                                placeholder="e.g. Domain Invariants"
                            />
                        </div>
                        <div>
                            <Label htmlFor="matrix-layer">Layer</Label>
                            <Select value={layer} onValueChange={v => setLayer(v as "invariant" | "contract")}>
                                <SelectTrigger id="matrix-layer">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="invariant">Invariant (Rules x Entities)</SelectItem>
                                    <SelectItem value="contract">Contract (Capabilities x Actors)</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>
                        <div>
                            <Label htmlFor="matrix-desc">Description (optional)</Label>
                            <Input
                                id="matrix-desc"
                                value={description}
                                onChange={e => setDescription(e.target.value)}
                                placeholder="Brief description"
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setCreateOpen(false)}>Cancel</Button>
                        <Button
                            onClick={() => createMutation.mutate()}
                            disabled={!name.trim() || createMutation.isPending}
                        >
                            Create
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </PageContainer>
    );
}
```

- [ ] **Step 2: Regenerate route tree**

Run: `cd apps/web && pnpm tsr generate`
Expected: `routeTree.gen.ts` updated with `/matrices` route

- [ ] **Step 3: Add nav link**

In `apps/web/src/routes/__root.tsx`, add a link to Matrices in the "Manage" dropdown under the "Navigate" section, alongside Features and Requirements:

```tsx
<Link to="/matrices" className="...">
    <Grid3X3 className="h-4 w-4" />
    Matrices
</Link>
```

Import `Grid3X3` from `lucide-react` if not already imported.

- [ ] **Step 4: Start dev server and verify the list page renders**

Run: `pnpm dev`
Navigate to `http://localhost:3001/matrices`
Expected: Empty state with "No matrices yet" message and "Create Matrix" button. Creating a matrix should redirect to the detail page (which will 404 for now — that's expected).

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/matrices.tsx apps/web/src/routes/__root.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat: add matrices list page with create dialog"
```

---

### Task 6: Frontend — Matrix Grid View

**Files:**
- Create: `apps/web/src/routes/matrices_.$matrixId.tsx`
- Create: `apps/web/src/features/matrices/matrix-grid.tsx`
- Create: `apps/web/src/features/matrices/cell-panel.tsx`

- [ ] **Step 1: Create the matrix grid component**

Create `apps/web/src/features/matrices/matrix-grid.tsx`:

```tsx
import { cn } from "~/lib/utils";

export type CellStatus = "specified" | "unspecified" | "violated";

export interface ViewCell {
    id: string;
    status: CellStatus;
    requirementCount: number;
}

interface Dimension {
    id: string;
    name: string;
    order: number;
}

interface Rule {
    id: string;
    title: string;
    description: string | null;
    category: string | null;
    order: number;
}

interface MatrixGridProps {
    dimensions: Dimension[];
    rules: Rule[];
    cells: Record<string, ViewCell | null>;
    onCellClick: (ruleId: string, dimensionId: string, cell: ViewCell | null) => void;
    onToggleCell: (ruleId: string, dimensionId: string) => void;
}

const statusColors: Record<CellStatus, string> = {
    specified: "bg-green-500/20 border-green-500/40 hover:bg-green-500/30",
    unspecified: "bg-yellow-500/20 border-yellow-500/40 hover:bg-yellow-500/30",
    violated: "bg-red-500/20 border-red-500/40 hover:bg-red-500/30"
};

export function MatrixGrid({ dimensions, rules, cells, onCellClick, onToggleCell }: MatrixGridProps) {
    const categories = Array.from(new Set(rules.map(r => r.category ?? "__uncategorized")));

    return (
        <div className="overflow-x-auto">
            <table className="w-full border-collapse">
                <thead>
                    <tr>
                        <th className="text-muted-foreground sticky left-0 z-10 bg-background border-border border-b border-r px-3 py-2 text-left text-sm font-medium">
                            Rules / Dimensions
                        </th>
                        {dimensions.map(dim => (
                            <th
                                key={dim.id}
                                className="border-border border-b px-3 py-2 text-center text-sm font-medium"
                            >
                                {dim.name}
                            </th>
                        ))}
                    </tr>
                </thead>
                <tbody>
                    {categories.map(category => {
                        const categoryRules = rules.filter(r => (r.category ?? "__uncategorized") === category);
                        return (
                            <CategoryGroup
                                key={category}
                                category={category === "__uncategorized" ? null : category}
                                rules={categoryRules}
                                dimensions={dimensions}
                                cells={cells}
                                onCellClick={onCellClick}
                                onToggleCell={onToggleCell}
                            />
                        );
                    })}
                </tbody>
            </table>
        </div>
    );
}

function CategoryGroup({
    category,
    rules,
    dimensions,
    cells,
    onCellClick,
    onToggleCell
}: {
    category: string | null;
    rules: Rule[];
    dimensions: Dimension[];
    cells: Record<string, ViewCell | null>;
    onCellClick: (ruleId: string, dimensionId: string, cell: ViewCell | null) => void;
    onToggleCell: (ruleId: string, dimensionId: string) => void;
}) {
    return (
        <>
            {category && (
                <tr>
                    <td
                        colSpan={dimensions.length + 1}
                        className="bg-muted/50 border-border border-b px-3 py-1.5 text-xs font-semibold uppercase tracking-wider"
                    >
                        {category}
                    </td>
                </tr>
            )}
            {rules.map(rule => (
                <tr key={rule.id} className="hover:bg-muted/30">
                    <td className="sticky left-0 z-10 bg-background border-border border-b border-r px-3 py-2 text-sm" title={rule.description ?? undefined}>
                        {rule.title}
                    </td>
                    {dimensions.map(dim => {
                        const key = `${rule.id}:${dim.id}`;
                        const cell = cells[key] ?? null;
                        return (
                            <td key={dim.id} className="border-border border-b px-1 py-1 text-center">
                                <button
                                    className={cn(
                                        "mx-auto h-8 w-8 rounded border transition-colors",
                                        cell
                                            ? statusColors[cell.status]
                                            : "border-border/50 bg-muted/20 hover:bg-muted/40"
                                    )}
                                    onClick={() => cell ? onCellClick(rule.id, dim.id, cell) : onToggleCell(rule.id, dim.id)}
                                    onContextMenu={e => {
                                        e.preventDefault();
                                        if (cell) onToggleCell(rule.id, dim.id);
                                    }}
                                    title={cell ? `${cell.status} (${cell.requirementCount} req)` : "Click to mark as relevant"}
                                >
                                    {cell && cell.requirementCount > 0 && (
                                        <span className="text-xs font-medium">{cell.requirementCount}</span>
                                    )}
                                </button>
                            </td>
                        );
                    })}
                </tr>
            ))}
        </>
    );
}
```

- [ ] **Step 2: Create the cell panel component**

Create `apps/web/src/features/matrices/cell-panel.tsx`:

```tsx
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "~/components/ui/badge";
import { Button } from "~/components/ui/button";
import { Input } from "~/components/ui/input";
import { api } from "~/utils/api";
import { unwrapEden } from "~/utils/eden";

interface CellPanelProps {
    matrixId: string;
    cellId: string;
    ruleTitle: string;
    dimensionName: string;
    onClose: () => void;
}

const statusVariant: Record<string, "success" | "secondary" | "destructive"> = {
    passing: "success",
    untested: "secondary",
    failing: "destructive"
};

export function CellPanel({ matrixId, cellId, ruleTitle, dimensionName, onClose }: CellPanelProps) {
    const queryClient = useQueryClient();
    const [searchReqId, setSearchReqId] = useState("");

    const requirementsQuery = useQuery({
        queryKey: ["matrix-cell-requirements", cellId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.matrices({ id: matrixId }).cells({ cellId }).requirements.get()
            );
        }
    });

    const linkMutation = useMutation({
        mutationFn: async (requirementId: string) => {
            return unwrapEden(
                await api.api.matrices({ id: matrixId }).cells({ cellId }).requirements.post({ requirementId })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-requirements", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setSearchReqId("");
            toast.success("Requirement linked");
        },
        onError: () => toast.error("Failed to link requirement")
    });

    const unlinkMutation = useMutation({
        mutationFn: async (requirementId: string) => {
            return unwrapEden(
                await api.api.matrices({ id: matrixId }).cells({ cellId }).requirements({ reqId: requirementId }).delete()
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-requirements", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            toast.success("Requirement unlinked");
        }
    });

    const reqs = (requirementsQuery.data ?? []) as Array<{ requirementId: string; title: string; status: string }>;

    return (
        <div className="border-border bg-card fixed right-0 top-0 z-50 flex h-full w-96 flex-col border-l shadow-lg">
            <div className="border-border flex items-center justify-between border-b px-4 py-3">
                <div>
                    <p className="text-sm font-medium">{ruleTitle}</p>
                    <p className="text-muted-foreground text-xs">{dimensionName}</p>
                </div>
                <Button variant="ghost" size="icon" onClick={onClose}>
                    <X className="h-4 w-4" />
                </Button>
            </div>

            <div className="flex-1 overflow-y-auto p-4">
                <h4 className="mb-2 text-sm font-medium">Linked Requirements ({reqs.length})</h4>
                {reqs.length === 0 ? (
                    <p className="text-muted-foreground text-sm">No requirements linked yet.</p>
                ) : (
                    <ul className="space-y-2">
                        {reqs.map(req => (
                            <li key={req.requirementId} className="flex items-center justify-between rounded border p-2 text-sm">
                                <div className="flex items-center gap-2">
                                    <Badge variant={statusVariant[req.status] ?? "secondary"} className="text-xs">
                                        {req.status}
                                    </Badge>
                                    <span>{req.title}</span>
                                </div>
                                <Button
                                    variant="ghost"
                                    size="icon"
                                    className="h-6 w-6"
                                    onClick={() => unlinkMutation.mutate(req.requirementId)}
                                >
                                    <X className="h-3 w-3" />
                                </Button>
                            </li>
                        ))}
                    </ul>
                )}

                <div className="mt-4">
                    <h4 className="mb-2 text-sm font-medium">Link Requirement</h4>
                    <div className="flex gap-2">
                        <Input
                            placeholder="Requirement ID"
                            value={searchReqId}
                            onChange={e => setSearchReqId(e.target.value)}
                            className="text-sm"
                        />
                        <Button
                            size="sm"
                            disabled={!searchReqId.trim() || linkMutation.isPending}
                            onClick={() => linkMutation.mutate(searchReqId.trim())}
                        >
                            Link
                        </Button>
                    </div>
                </div>
            </div>
        </div>
    );
}
```

- [ ] **Step 3: Create the matrix detail route page**

Create `apps/web/src/routes/matrices_.$matrixId.tsx`:

```tsx
import { createFileRoute, Link } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Grid3X3, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { PageContainer, PageLoading } from "~/components/ui/page";
import { Badge } from "~/components/ui/badge";
import { Button } from "~/components/ui/button";
import { Input } from "~/components/ui/input";
import { useApiQuery } from "~/hooks/use-api-query";
import { MatrixGrid, type ViewCell } from "~/features/matrices/matrix-grid";
import { CellPanel } from "~/features/matrices/cell-panel";
import { api } from "~/utils/api";
import { unwrapEden } from "~/utils/eden";

export const Route = createFileRoute("/matrices_/$matrixId")({
    component: MatrixDetailPage,
    beforeLoad: ({ context }) => {
        if (!context.session) throw new Error("Not authenticated");
    }
});

function MatrixDetailPage() {
    const { matrixId } = Route.useParams();
    const queryClient = useQueryClient();

    const [newDimName, setNewDimName] = useState("");
    const [newRuleTitle, setNewRuleTitle] = useState("");
    const [newRuleCategory, setNewRuleCategory] = useState("");

    const [selectedCell, setSelectedCell] = useState<{
        ruleId: string;
        dimensionId: string;
        cellId: string;
        ruleTitle: string;
        dimensionName: string;
    } | null>(null);

    const viewQuery = useApiQuery<{
        matrix: { id: string; name: string; layer: string; description: string | null };
        dimensions: Array<{ id: string; name: string; order: number }>;
        rules: Array<{ id: string; title: string; description: string | null; category: string | null; order: number }>;
        cells: Record<string, ViewCell | null>;
        summary: { specified: number; unspecified: number; violated: number; total: number };
    }>({
        queryKey: ["matrix-view", matrixId],
        queryFn: () => api.api.matrices({ id: matrixId }).view.get()
    });

    const addDimensionMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.matrices({ id: matrixId }).dimensions.post({ name: newDimName.trim() }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setNewDimName("");
            toast.success("Dimension added");
        },
        onError: () => toast.error("Failed to add dimension")
    });

    const addRuleMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.matrices({ id: matrixId }).rules.post({
                title: newRuleTitle.trim(),
                category: newRuleCategory.trim() || undefined
            }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setNewRuleTitle("");
            toast.success("Rule added");
        },
        onError: () => toast.error("Failed to add rule")
    });

    const toggleCellMutation = useMutation({
        mutationFn: async ({ ruleId, dimensionId }: { ruleId: string; dimensionId: string }) => {
            return unwrapEden(await api.api.matrices({ id: matrixId }).cells.put({ ruleId, dimensionId }));
        },
        onSuccess: (data) => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            const action = (data as { action: string }).action;
            toast.success(action === "created" ? "Cell marked as relevant" : "Cell removed");
        },
        onError: (err) => toast.error(err.message || "Failed to toggle cell")
    });

    if (viewQuery.isLoading) return <PageContainer maxWidth="6xl"><PageLoading count={5} /></PageContainer>;
    if (!viewQuery.data) return null;

    const { matrix, dimensions, rules, cells, summary } = viewQuery.data;

    function handleCellClick(ruleId: string, dimensionId: string, cell: ViewCell | null) {
        if (!cell) return;
        const rule = rules.find(r => r.id === ruleId);
        const dim = dimensions.find(d => d.id === dimensionId);
        setSelectedCell({
            ruleId,
            dimensionId,
            cellId: cell.id,
            ruleTitle: rule?.title ?? "",
            dimensionName: dim?.name ?? ""
        });
    }

    function handleToggleCell(ruleId: string, dimensionId: string) {
        toggleCellMutation.mutate({ ruleId, dimensionId });
    }

    return (
        <PageContainer maxWidth="6xl">
            {/* Header */}
            <div className="mb-6">
                <Link to="/matrices" className="text-muted-foreground hover:text-foreground mb-2 inline-flex items-center gap-1 text-sm">
                    <ArrowLeft className="h-4 w-4" />
                    Matrices
                </Link>
                <div className="flex items-center gap-3">
                    <Grid3X3 className="text-muted-foreground h-6 w-6" />
                    <h1 className="text-2xl font-bold">{matrix.name}</h1>
                    <Badge variant={matrix.layer === "invariant" ? "secondary" : "outline"}>
                        {matrix.layer}
                    </Badge>
                </div>
                {matrix.description && (
                    <p className="text-muted-foreground mt-1">{matrix.description}</p>
                )}
            </div>

            {/* Coverage Summary */}
            <div className="border-border mb-4 flex items-center gap-4 rounded-lg border p-3">
                <span className="text-sm font-medium">Coverage:</span>
                <span className="text-sm">
                    <span className="text-green-500 font-medium">{summary.specified}</span> specified
                </span>
                <span className="text-sm">
                    <span className="text-yellow-500 font-medium">{summary.unspecified}</span> unspecified
                </span>
                <span className="text-sm">
                    <span className="text-red-500 font-medium">{summary.violated}</span> violated
                </span>
                <span className="text-muted-foreground text-sm">/ {summary.total} total</span>
            </div>

            {/* Grid */}
            <MatrixGrid
                dimensions={dimensions}
                rules={rules}
                cells={cells}
                onCellClick={handleCellClick}
                onToggleCell={handleToggleCell}
            />

            {/* Add controls */}
            <div className="mt-4 flex gap-4">
                <div className="flex items-center gap-2">
                    <Input
                        placeholder="New dimension name"
                        value={newDimName}
                        onChange={e => setNewDimName(e.target.value)}
                        className="w-48 text-sm"
                        onKeyDown={e => { if (e.key === "Enter" && newDimName.trim()) addDimensionMutation.mutate(); }}
                    />
                    <Button size="sm" variant="outline" disabled={!newDimName.trim()} onClick={() => addDimensionMutation.mutate()}>
                        <Plus className="mr-1 h-3 w-3" />
                        Column
                    </Button>
                </div>
                <div className="flex items-center gap-2">
                    <Input
                        placeholder="New rule title"
                        value={newRuleTitle}
                        onChange={e => setNewRuleTitle(e.target.value)}
                        className="w-48 text-sm"
                        onKeyDown={e => { if (e.key === "Enter" && newRuleTitle.trim()) addRuleMutation.mutate(); }}
                    />
                    <Input
                        placeholder="Category (optional)"
                        value={newRuleCategory}
                        onChange={e => setNewRuleCategory(e.target.value)}
                        className="w-36 text-sm"
                    />
                    <Button size="sm" variant="outline" disabled={!newRuleTitle.trim()} onClick={() => addRuleMutation.mutate()}>
                        <Plus className="mr-1 h-3 w-3" />
                        Row
                    </Button>
                </div>
            </div>

            {/* Cell Panel (slide-over) */}
            {selectedCell && (
                <CellPanel
                    matrixId={matrixId}
                    cellId={selectedCell.cellId}
                    ruleTitle={selectedCell.ruleTitle}
                    dimensionName={selectedCell.dimensionName}
                    onClose={() => setSelectedCell(null)}
                />
            )}
        </PageContainer>
    );
}
```

- [ ] **Step 4: Regenerate route tree**

Run: `cd apps/web && pnpm tsr generate`
Expected: `routeTree.gen.ts` updated with `/matrices/$matrixId` route

- [ ] **Step 5: Test in browser**

Run: `pnpm dev`
Navigate to `http://localhost:3001/matrices`, create a matrix, add dimensions and rules, click cells to toggle them, click colored cells to open the panel.

Expected:
- Grid renders with dimensions as columns, rules as rows
- Clicking gray cells creates them (turns yellow)
- Right-clicking colored cells removes them (with confirmation if linked requirements exist)
- Clicking colored cells opens the slide-over panel
- Coverage summary updates after each toggle

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/routes/matrices_.\$matrixId.tsx apps/web/src/features/matrices/matrix-grid.tsx apps/web/src/features/matrices/cell-panel.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat: add matrix grid view and cell panel"
```

---

### Task 7: MCP Tools

**Files:**
- Create: `packages/mcp/src/matrix-tools.ts`
- Modify: `packages/mcp/src/index.ts`

- [ ] **Step 1: Create the MCP tools file**

Create `packages/mcp/src/matrix-tools.ts`:

```typescript
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

import { apiFetch } from "./api-client.js";

export const matrixPlugin = {
    name: "matrix",
    description: "Behavioral specification matrix tools",
    register(server: McpServer) {
        server.tool(
            "list_matrices",
            "List behavioral specification matrices",
            { codebaseId: z.string().optional(), layer: z.enum(["invariant", "contract"]).optional() },
            async (params) => {
                const query = new URLSearchParams();
                if (params.codebaseId) query.set("codebaseId", params.codebaseId);
                if (params.layer) query.set("layer", params.layer);
                const result = await apiFetch(`/matrices?${query}`);
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "get_matrix_view",
            "Get the full behavioral matrix grid with computed cell statuses (specified/unspecified/violated)",
            { matrixId: z.string().describe("Matrix ID") },
            async (params) => {
                const result = await apiFetch(`/matrices/${params.matrixId}/view`);
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "create_matrix",
            "Create a new behavioral specification matrix",
            {
                name: z.string().describe("Matrix name"),
                layer: z.enum(["invariant", "contract"]).describe("invariant (rules x entities) or contract (capabilities x actors)"),
                description: z.string().optional(),
                codebaseId: z.string().optional()
            },
            async (params) => {
                const result = await apiFetch("/matrices", {
                    method: "POST",
                    body: JSON.stringify(params)
                });
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "add_dimension",
            "Add a column (dimension) to a matrix",
            {
                matrixId: z.string().describe("Matrix ID"),
                name: z.string().describe("Dimension name, e.g. 'Chunk' or 'AI Agent'")
            },
            async (params) => {
                const result = await apiFetch(`/matrices/${params.matrixId}/dimensions`, {
                    method: "POST",
                    body: JSON.stringify({ name: params.name })
                });
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "add_rule",
            "Add a row (rule) to a matrix",
            {
                matrixId: z.string().describe("Matrix ID"),
                title: z.string().describe("Rule title, e.g. 'Cascade deletes to children'"),
                description: z.string().optional(),
                category: z.string().optional().describe("Category for grouping rules")
            },
            async (params) => {
                const result = await apiFetch(`/matrices/${params.matrixId}/rules`, {
                    method: "POST",
                    body: JSON.stringify({ title: params.title, description: params.description, category: params.category })
                });
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "toggle_cell",
            "Toggle a cell (intersection of rule and dimension). Creates cell if absent, deletes if present and has no linked requirements.",
            {
                matrixId: z.string().describe("Matrix ID"),
                ruleId: z.string().describe("Rule ID"),
                dimensionId: z.string().describe("Dimension ID")
            },
            async (params) => {
                const result = await apiFetch(`/matrices/${params.matrixId}/cells`, {
                    method: "PUT",
                    body: JSON.stringify({ ruleId: params.ruleId, dimensionId: params.dimensionId })
                });
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "link_cell_requirement",
            "Link a BDD requirement to a matrix cell",
            {
                matrixId: z.string().describe("Matrix ID"),
                cellId: z.string().describe("Cell ID"),
                requirementId: z.string().describe("Requirement ID to link")
            },
            async (params) => {
                const result = await apiFetch(`/matrices/${params.matrixId}/cells/${params.cellId}/requirements`, {
                    method: "POST",
                    body: JSON.stringify({ requirementId: params.requirementId })
                });
                return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
            }
        );

        server.tool(
            "get_matrix_gaps",
            "Get only unspecified and violated cells — shows what is missing or broken",
            { matrixId: z.string().describe("Matrix ID") },
            async (params) => {
                const view = (await apiFetch(`/matrices/${params.matrixId}/view`)) as {
                    matrix: { name: string };
                    dimensions: Array<{ id: string; name: string }>;
                    rules: Array<{ id: string; title: string }>;
                    cells: Record<string, { status: string; requirementCount: number } | null>;
                    summary: { specified: number; unspecified: number; violated: number; total: number };
                };

                const dimMap = new Map(view.dimensions.map(d => [d.id, d.name]));
                const ruleMap = new Map(view.rules.map(r => [r.id, r.title]));

                const gaps: Array<{ rule: string; dimension: string; status: string }> = [];
                for (const [key, cell] of Object.entries(view.cells)) {
                    if (cell && (cell.status === "unspecified" || cell.status === "violated")) {
                        const [ruleId, dimId] = key.split(":");
                        gaps.push({
                            rule: ruleMap.get(ruleId!) ?? ruleId!,
                            dimension: dimMap.get(dimId!) ?? dimId!,
                            status: cell.status
                        });
                    }
                }

                return {
                    content: [{
                        type: "text",
                        text: JSON.stringify({ matrix: view.matrix.name, summary: view.summary, gaps }, null, 2)
                    }]
                };
            }
        );
    }
};
```

- [ ] **Step 2: Register the plugin in MCP index**

In `packages/mcp/src/index.ts`, import and register the plugin:

```typescript
import { matrixPlugin } from "./matrix-tools.js";
```

Add to the plugin registration:

```typescript
registerPlugin(matrixPlugin);
```

- [ ] **Step 3: Verify MCP builds**

Run: `cd packages/mcp && pnpm build`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add packages/mcp/src/matrix-tools.ts packages/mcp/src/index.ts
git commit -m "feat: add behavioral matrix MCP tools"
```

---

### Task 8: CLI Commands

**Files:**
- Create: `apps/cli/src/commands/matrix.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Create the CLI command file**

Create `apps/cli/src/commands/matrix.ts`:

```typescript
import { Command } from "commander";

import { fetchApiJson, requireServer } from "../lib/api";
import { formatBold, formatSuccess, output } from "../lib/output";

interface Matrix {
    id: string;
    name: string;
    layer: string;
    description: string | null;
}

interface ViewResponse {
    matrix: Matrix;
    dimensions: Array<{ id: string; name: string; order: number }>;
    rules: Array<{ id: string; title: string; category: string | null; order: number }>;
    cells: Record<string, { id: string; status: string; requirementCount: number } | null>;
    summary: { specified: number; unspecified: number; violated: number; total: number };
}

export const matrixCommand = new Command("matrix").description("Manage behavioral specification matrices");

matrixCommand
    .command("list")
    .description("List matrices")
    .option("--layer <layer>", "Filter by layer (invariant|contract)")
    .action(async (opts) => {
        await requireServer();
        const query = new URLSearchParams();
        if (opts.layer) query.set("layer", opts.layer);
        const matrices = await fetchApiJson<Matrix[]>(`/matrices?${query}`);
        if (matrices.length === 0) {
            output("No matrices found.");
            return;
        }
        for (const m of matrices) {
            output(`${formatBold(m.name)} [${m.layer}] (${m.id})`);
            if (m.description) output(`  ${m.description}`);
        }
    });

matrixCommand
    .command("create <name>")
    .description("Create a new matrix")
    .requiredOption("--layer <layer>", "Layer: invariant or contract")
    .option("--description <desc>", "Description")
    .option("--codebase <id>", "Codebase ID")
    .action(async (name, opts) => {
        await requireServer();
        const matrix = await fetchApiJson<Matrix>("/matrices", {
            method: "POST",
            body: JSON.stringify({
                name,
                layer: opts.layer,
                description: opts.description,
                codebaseId: opts.codebase
            })
        });
        output(formatSuccess(`Created matrix "${matrix.name}" (${matrix.id})`));
    });

matrixCommand
    .command("show <id>")
    .description("Show matrix as ASCII grid")
    .action(async (id) => {
        await requireServer();
        const view = await fetchApiJson<ViewResponse>(`/matrices/${id}/view`);
        const { matrix, dimensions, rules, cells, summary } = view;

        output(`\n${formatBold(matrix.name)} [${matrix.layer}]`);
        output(`Coverage: ${summary.specified} specified, ${summary.unspecified} unspecified, ${summary.violated} violated / ${summary.total} total\n`);

        if (dimensions.length === 0 || rules.length === 0) {
            output("Matrix is empty. Add dimensions and rules first.");
            return;
        }

        const maxRuleLen = Math.max(...rules.map(r => r.title.length), 10);
        const colWidth = Math.max(...dimensions.map(d => d.name.length), 5);

        // Header
        const header = "".padEnd(maxRuleLen + 2) + dimensions.map(d => d.name.padStart(colWidth)).join(" ");
        output(header);
        output("-".repeat(header.length));

        // Rows
        for (const rule of rules) {
            const row = rule.title.padEnd(maxRuleLen + 2) + dimensions.map(dim => {
                const key = `${rule.id}:${dim.id}`;
                const cell = cells[key];
                if (!cell) return ".".padStart(colWidth);
                const symbol = cell.status === "specified" ? "✓" : cell.status === "violated" ? "✗" : "?";
                return symbol.padStart(colWidth);
            }).join(" ");
            output(row);
        }
    });

matrixCommand
    .command("add-dimension <matrixId> <name>")
    .description("Add a dimension (column)")
    .action(async (matrixId, name) => {
        await requireServer();
        await fetchApiJson(`/matrices/${matrixId}/dimensions`, {
            method: "POST",
            body: JSON.stringify({ name })
        });
        output(formatSuccess(`Added dimension "${name}"`));
    });

matrixCommand
    .command("add-rule <matrixId> <title>")
    .description("Add a rule (row)")
    .option("--category <category>", "Category for grouping")
    .action(async (matrixId, title, opts) => {
        await requireServer();
        await fetchApiJson(`/matrices/${matrixId}/rules`, {
            method: "POST",
            body: JSON.stringify({ title, category: opts.category })
        });
        output(formatSuccess(`Added rule "${title}"`));
    });

matrixCommand
    .command("cell <matrixId> <ruleId> <dimensionId>")
    .description("Toggle a cell (mark as relevant or remove)")
    .action(async (matrixId, ruleId, dimensionId) => {
        await requireServer();
        const result = await fetchApiJson<{ action: string }>(`/matrices/${matrixId}/cells`, {
            method: "PUT",
            body: JSON.stringify({ ruleId, dimensionId })
        });
        output(formatSuccess(`Cell ${result.action}`));
    });

matrixCommand
    .command("gaps <id>")
    .description("List unspecified and violated cells")
    .action(async (id) => {
        await requireServer();
        const view = await fetchApiJson<ViewResponse>(`/matrices/${id}/view`);
        const { dimensions, rules, cells, summary } = view;

        const dimMap = new Map(dimensions.map(d => [d.id, d.name]));
        const ruleMap = new Map(rules.map(r => [r.id, r.title]));

        let count = 0;
        for (const [key, cell] of Object.entries(cells)) {
            if (cell && (cell.status === "unspecified" || cell.status === "violated")) {
                const [ruleId, dimId] = key.split(":");
                const status = cell.status === "violated" ? "VIOLATED" : "GAP";
                output(`[${status}] "${ruleMap.get(ruleId!)}" × "${dimMap.get(dimId!)}"`);
                count++;
            }
        }
        if (count === 0) output(formatSuccess("No gaps or violations found."));
        else output(`\n${count} issue(s) found.`);
    });

matrixCommand
    .command("link <cellId> <requirementId>")
    .description("Link a requirement to a cell")
    .requiredOption("--matrix <matrixId>", "Matrix ID")
    .action(async (cellId, requirementId, opts) => {
        await requireServer();
        await fetchApiJson(`/matrices/${opts.matrix}/cells/${cellId}/requirements`, {
            method: "POST",
            body: JSON.stringify({ requirementId })
        });
        output(formatSuccess("Requirement linked to cell"));
    });
```

- [ ] **Step 2: Register the command in CLI index**

In `apps/cli/src/index.ts`, import and add the command:

```typescript
import { matrixCommand } from "./commands/matrix";
```

Add:

```typescript
program.addCommand(matrixCommand);
```

- [ ] **Step 3: Verify CLI builds**

Run: `cd apps/cli && pnpm build`
Expected: No errors

- [ ] **Step 4: Test CLI commands**

Run (with server running):
```bash
node apps/cli/dist/index.js matrix list
node apps/cli/dist/index.js matrix create "Test Invariants" --layer invariant
node apps/cli/dist/index.js matrix show <id-from-previous>
```
Expected: List shows the created matrix, show renders an ASCII grid (empty at first)

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/commands/matrix.ts apps/cli/src/index.ts
git commit -m "feat: add behavioral matrix CLI commands"
```

---

### Task 9: Integration and Polish

**Files:**
- Modify: `apps/web/src/routes/__root.tsx` (if not already done in Task 5)

- [ ] **Step 1: Run full test suite**

Run: `pnpm test`
Expected: All tests pass, including the new schema and service tests

- [ ] **Step 2: Run type check**

Run: `pnpm run check-types`
Expected: No type errors

- [ ] **Step 3: Test end-to-end flow in browser**

1. Navigate to `/matrices`
2. Create an "invariant" matrix
3. Add 3 dimensions (e.g., "Chunk", "Plan", "Feature")
4. Add 3 rules (e.g., "Title required", "Cascade deletes", "User ownership")
5. Click gray cells to create them (should turn yellow)
6. Open cell panel by clicking a yellow cell
7. Link a requirement (if one exists)
8. Verify cell turns green after linking a passing requirement
9. Verify coverage summary updates

- [ ] **Step 4: Test CLI flow**

```bash
node apps/cli/dist/index.js matrix list
node apps/cli/dist/index.js matrix show <id>
node apps/cli/dist/index.js matrix gaps <id>
```
Expected: ASCII grid renders correctly, gaps command shows unspecified cells

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: behavioral specification matrix — full stack"
```
