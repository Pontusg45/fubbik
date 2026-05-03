# Feature Flag Knowledge Overlays Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a delta/overlay system that associates chunk modifications with named features, enabling feature-scoped knowledge that can be toggled, prioritized, and merged.

**Architecture:** New `feature`, `feature_codebase`, `chunk_feature_delta`, and `user_active_feature` tables in Drizzle. Repository → Service → Route layers following existing patterns. A resolution utility applies deltas to base chunks at the service layer. Frontend gets a feature switcher in the nav and a management page.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router/Query, React

**Spec:** `docs/superpowers/specs/2026-05-03-feature-flag-knowledge-overlays-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `packages/db/src/schema/feature.ts` | `feature`, `feature_codebase`, `chunk_feature_delta`, `user_active_feature` table + relation definitions |
| `packages/db/src/repository/feature.ts` | Feature CRUD, codebase association, active feature reads/writes |
| `packages/db/src/repository/chunk-feature-delta.ts` | Delta CRUD, batch fetch for resolution |
| `packages/api/src/features/service.ts` | Feature business logic, merge flow, priority reordering |
| `packages/api/src/features/resolve.ts` | `resolveChunk()` and `resolveChunks()` pure utilities |
| `packages/api/src/features/resolve.test.ts` | Tests for resolution logic |
| `packages/api/src/features/routes.ts` | Elysia routes for feature CRUD, activation, delta endpoints |
| `apps/web/src/features/feature-flags/use-active-features.ts` | Hook for reading/writing active features (server-persisted, fetched via API) |
| `apps/web/src/features/feature-flags/feature-switcher.tsx` | Nav dropdown to toggle features on/off |
| `apps/web/src/routes/features.tsx` | Feature management page |

### Modified files

| File | Change |
|------|--------|
| `packages/db/src/schema/index.ts` | Add `export * from "./feature"` |
| `packages/db/src/repository/index.ts` | Add exports for feature + delta repos |
| `packages/api/src/index.ts` | Mount `featureRoutes`, add active feature resolution to `.resolve()` |
| `packages/api/src/chunks/service.ts` | Call `resolveChunks()` in `listChunks` and `getChunkDetail` |
| `apps/web/src/routes/__root.tsx` | Add `<FeatureSwitcher />` to nav |

---

### Task 1: Schema definitions

**Files:**
- Create: `packages/db/src/schema/feature.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create the feature schema file**

Create `packages/db/src/schema/feature.ts`:

```typescript
import { relations } from "drizzle-orm";
import { index, integer, jsonb, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";

export const feature = pgTable(
    "feature",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        priority: integer("priority").notNull(),
        status: text("status").notNull().default("inactive"),
        color: text("color"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().notNull().$onUpdate(() => new Date())
    },
    table => [
        uniqueIndex("feature_user_name_idx").on(table.userId, table.name),
        uniqueIndex("feature_user_priority_idx").on(table.userId, table.priority)
    ]
);

export const featureCodebase = pgTable(
    "feature_codebase",
    {
        featureId: text("feature_id")
            .notNull()
            .references(() => feature.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.featureId, table.codebaseId] })]
);

export const chunkFeatureDelta = pgTable(
    "chunk_feature_delta",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        featureId: text("feature_id")
            .notNull()
            .references(() => feature.id, { onDelete: "cascade" }),
        delta: jsonb("delta").notNull().$type<Record<string, unknown>>(),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().notNull().$onUpdate(() => new Date())
    },
    table => [
        uniqueIndex("chunk_feature_delta_chunk_feature_idx").on(table.chunkId, table.featureId),
        index("chunk_feature_delta_feature_idx").on(table.featureId)
    ]
);

export const userActiveFeature = pgTable(
    "user_active_feature",
    {
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        featureId: text("feature_id")
            .notNull()
            .references(() => feature.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.userId, table.featureId] })]
);

export const featureRelations = relations(feature, ({ one, many }) => ({
    user: one(user, { fields: [feature.userId], references: [user.id] }),
    codebases: many(featureCodebase),
    deltas: many(chunkFeatureDelta),
    activeUsers: many(userActiveFeature)
}));

export const featureCodebaseRelations = relations(featureCodebase, ({ one }) => ({
    feature: one(feature, { fields: [featureCodebase.featureId], references: [feature.id] }),
    codebase: one(codebase, { fields: [featureCodebase.codebaseId], references: [codebase.id] })
}));

export const chunkFeatureDeltaRelations = relations(chunkFeatureDelta, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkFeatureDelta.chunkId], references: [chunk.id] }),
    feature: one(feature, { fields: [chunkFeatureDelta.featureId], references: [feature.id] })
}));

export const userActiveFeatureRelations = relations(userActiveFeature, ({ one }) => ({
    user: one(user, { fields: [userActiveFeature.userId], references: [user.id] }),
    feature: one(feature, { fields: [userActiveFeature.featureId], references: [feature.id] })
}));
```

- [ ] **Step 2: Export from schema index**

Add to `packages/db/src/schema/index.ts`:

```typescript
export * from "./feature";
```

- [ ] **Step 3: Push schema to database**

Run: `pnpm db:push`

Expected: Tables `feature`, `feature_codebase`, `chunk_feature_delta`, `user_active_feature` created successfully.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/feature.ts packages/db/src/schema/index.ts
git commit -m "feat: add feature flag knowledge overlay schema"
```

---

### Task 2: Feature repository

**Files:**
- Create: `packages/db/src/repository/feature.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the feature repository**

Create `packages/db/src/repository/feature.ts`:

```typescript
import { and, eq, gt, gte, ilike, inArray, lt, lte, sql } from "drizzle-orm";
import { Effect } from "effect";

import { db, dbEffect } from "../index";
import { feature, featureCodebase, userActiveFeature, chunkFeatureDelta } from "../schema/feature";
import { codebase } from "../schema/codebase";

export function createFeature(params: {
    id: string;
    name: string;
    description?: string;
    priority: number;
    color?: string;
    userId: string;
}) {
    return dbEffect(async () => {
        const [created] = await db.insert(feature).values(params).returning();
        return created!;
    });
}

export function getFeatureById(id: string, userId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(feature)
            .where(and(eq(feature.id, id), eq(feature.userId, userId)));
        return found ?? null;
    });
}

export function listFeatures(userId: string, filters?: { codebaseId?: string; status?: string; search?: string }) {
    return dbEffect(async () => {
        const conditions = [eq(feature.userId, userId)];

        if (filters?.status) {
            conditions.push(eq(feature.status, filters.status));
        }
        if (filters?.search) {
            conditions.push(ilike(feature.name, `%${filters.search}%`));
        }

        const features = await db
            .select({
                id: feature.id,
                name: feature.name,
                description: feature.description,
                priority: feature.priority,
                status: feature.status,
                color: feature.color,
                createdAt: feature.createdAt,
                updatedAt: feature.updatedAt,
                deltaCount: sql<number>`count(${chunkFeatureDelta.id})::int`.as("delta_count")
            })
            .from(feature)
            .leftJoin(chunkFeatureDelta, eq(chunkFeatureDelta.featureId, feature.id))
            .where(and(...conditions))
            .groupBy(feature.id)
            .orderBy(feature.priority);

        if (!filters?.codebaseId) return features;

        // Filter by codebase association
        const featureIdsInCodebase = await db
            .select({ featureId: featureCodebase.featureId })
            .from(featureCodebase)
            .where(eq(featureCodebase.codebaseId, filters.codebaseId));
        const idSet = new Set(featureIdsInCodebase.map(r => r.featureId));

        // Include features linked to this codebase OR features with no codebase links (global)
        const allLinked = await db
            .select({ featureId: featureCodebase.featureId })
            .from(featureCodebase);
        const linkedSet = new Set(allLinked.map(r => r.featureId));

        return features.filter(f => idSet.has(f.id) || !linkedSet.has(f.id));
    });
}

export function updateFeature(id: string, userId: string, data: {
    name?: string;
    description?: string | null;
    priority?: number;
    status?: string;
    color?: string | null;
}) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(feature)
            .set(data)
            .where(and(eq(feature.id, id), eq(feature.userId, userId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteFeature(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(feature)
            .where(and(eq(feature.id, id), eq(feature.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

export function setFeatureCodebases(featureId: string, codebaseIds: string[]) {
    return dbEffect(async () => {
        await db.delete(featureCodebase).where(eq(featureCodebase.featureId, featureId));
        if (codebaseIds.length === 0) return [];
        return db
            .insert(featureCodebase)
            .values(codebaseIds.map(codebaseId => ({ featureId, codebaseId })))
            .returning();
    });
}

export function getCodebasesForFeature(featureId: string) {
    return dbEffect(() =>
        db
            .select({ id: codebase.id, name: codebase.name })
            .from(featureCodebase)
            .innerJoin(codebase, eq(featureCodebase.codebaseId, codebase.id))
            .where(eq(featureCodebase.featureId, featureId))
    );
}

export function shiftPriorities(userId: string, newPriority: number, direction: "up" | "down") {
    return dbEffect(async () => {
        if (direction === "up") {
            await db
                .update(feature)
                .set({ priority: sql`${feature.priority} + 1` })
                .where(and(eq(feature.userId, userId), gte(feature.priority, newPriority)));
        } else {
            await db
                .update(feature)
                .set({ priority: sql`${feature.priority} - 1` })
                .where(and(eq(feature.userId, userId), lte(feature.priority, newPriority)));
        }
    });
}

export function getMaxPriority(userId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ max: sql<number>`coalesce(max(${feature.priority}), 0)::int` })
            .from(feature)
            .where(eq(feature.userId, userId));
        return result?.max ?? 0;
    });
}

// --- Active feature management ---

export function getActiveFeatureIds(userId: string) {
    return dbEffect(() =>
        db
            .select({ featureId: userActiveFeature.featureId })
            .from(userActiveFeature)
            .where(eq(userActiveFeature.userId, userId))
    );
}

export function setActiveFeatures(userId: string, featureIds: string[]) {
    return dbEffect(async () => {
        await db.delete(userActiveFeature).where(eq(userActiveFeature.userId, userId));
        if (featureIds.length === 0) return [];
        return db
            .insert(userActiveFeature)
            .values(featureIds.map(featureId => ({ userId, featureId })))
            .returning();
    });
}

export function featureNameConflict(id: string, userId: string, name: string) {
    return dbEffect(async () => {
        const [hit] = await db
            .select({ id: feature.id })
            .from(feature)
            .where(and(eq(feature.userId, userId), eq(feature.name, name), sql`${feature.id} != ${id}`))
            .limit(1);
        return !!hit;
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:

```typescript
export * from "./feature";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/feature.ts packages/db/src/repository/index.ts
git commit -m "feat: add feature repository with CRUD and active feature management"
```

---

### Task 3: Delta repository

**Files:**
- Create: `packages/db/src/repository/chunk-feature-delta.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the delta repository**

Create `packages/db/src/repository/chunk-feature-delta.ts`:

```typescript
import { and, eq, inArray, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunkFeatureDelta } from "../schema/feature";
import { feature } from "../schema/feature";
import { chunk } from "../schema/chunk";

export function upsertDelta(params: { id: string; chunkId: string; featureId: string; delta: Record<string, unknown> }) {
    return dbEffect(async () => {
        const [result] = await db
            .insert(chunkFeatureDelta)
            .values(params)
            .onConflictDoUpdate({
                target: [chunkFeatureDelta.chunkId, chunkFeatureDelta.featureId],
                set: { delta: params.delta, updatedAt: new Date() }
            })
            .returning();
        return result!;
    });
}

export function getDeltasForChunk(chunkId: string) {
    return dbEffect(() =>
        db
            .select({
                id: chunkFeatureDelta.id,
                chunkId: chunkFeatureDelta.chunkId,
                featureId: chunkFeatureDelta.featureId,
                delta: chunkFeatureDelta.delta,
                featureName: feature.name,
                featurePriority: feature.priority,
                featureColor: feature.color,
                featureStatus: feature.status,
                createdAt: chunkFeatureDelta.createdAt,
                updatedAt: chunkFeatureDelta.updatedAt
            })
            .from(chunkFeatureDelta)
            .innerJoin(feature, eq(chunkFeatureDelta.featureId, feature.id))
            .where(eq(chunkFeatureDelta.chunkId, chunkId))
            .orderBy(feature.priority)
    );
}

export function getDeltasForFeature(featureId: string) {
    return dbEffect(() =>
        db
            .select({
                id: chunkFeatureDelta.id,
                chunkId: chunkFeatureDelta.chunkId,
                featureId: chunkFeatureDelta.featureId,
                delta: chunkFeatureDelta.delta,
                chunkTitle: chunk.title,
                createdAt: chunkFeatureDelta.createdAt,
                updatedAt: chunkFeatureDelta.updatedAt
            })
            .from(chunkFeatureDelta)
            .innerJoin(chunk, eq(chunkFeatureDelta.chunkId, chunk.id))
            .where(eq(chunkFeatureDelta.featureId, featureId))
    );
}

export function batchFetchDeltas(chunkIds: string[], featureIds: string[]) {
    if (chunkIds.length === 0 || featureIds.length === 0) {
        return dbEffect(async () => []);
    }
    return dbEffect(() =>
        db
            .select({
                id: chunkFeatureDelta.id,
                chunkId: chunkFeatureDelta.chunkId,
                featureId: chunkFeatureDelta.featureId,
                delta: chunkFeatureDelta.delta,
                featurePriority: feature.priority
            })
            .from(chunkFeatureDelta)
            .innerJoin(feature, eq(chunkFeatureDelta.featureId, feature.id))
            .where(
                and(
                    inArray(chunkFeatureDelta.chunkId, chunkIds),
                    inArray(chunkFeatureDelta.featureId, featureIds)
                )
            )
            .orderBy(feature.priority)
    );
}

export function deleteDelta(chunkId: string, featureId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(chunkFeatureDelta)
            .where(
                and(
                    eq(chunkFeatureDelta.chunkId, chunkId),
                    eq(chunkFeatureDelta.featureId, featureId)
                )
            )
            .returning();
        return deleted ?? null;
    });
}

export function deleteDeltasForFeature(featureId: string) {
    return dbEffect(async () => {
        await db
            .delete(chunkFeatureDelta)
            .where(eq(chunkFeatureDelta.featureId, featureId));
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:

```typescript
export * from "./chunk-feature-delta";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/chunk-feature-delta.ts packages/db/src/repository/index.ts
git commit -m "feat: add chunk feature delta repository"
```

---

### Task 4: Resolution utility with tests

**Files:**
- Create: `packages/api/src/features/resolve.ts`
- Create: `packages/api/src/features/resolve.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `packages/api/src/features/resolve.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { resolveChunk, resolveChunks } from "./resolve";

describe("resolveChunk", () => {
    const baseChunk = {
        id: "chunk-1",
        title: "Base Title",
        content: "Base content",
        type: "note",
        rationale: null,
        summary: null
    };

    it("returns base chunk unchanged when no deltas", () => {
        const result = resolveChunk(baseChunk, []);
        expect(result.title).toBe("Base Title");
        expect(result.content).toBe("Base content");
        expect(result._appliedFeatures).toEqual([]);
        expect(result._hasDeltas).toBe(false);
    });

    it("applies a single delta", () => {
        const deltas = [
            { featureId: "f1", delta: { content: "Feature content" }, priority: 1 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.title).toBe("Base Title");
        expect(result.content).toBe("Feature content");
        expect(result._appliedFeatures).toEqual(["f1"]);
        expect(result._hasDeltas).toBe(true);
    });

    it("applies multiple deltas in priority order (higher priority wins)", () => {
        const deltas = [
            { featureId: "f2", delta: { content: "High priority content" }, priority: 10 },
            { featureId: "f1", delta: { content: "Low priority content" }, priority: 1 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.content).toBe("High priority content");
        expect(result._appliedFeatures).toEqual(["f1", "f2"]);
    });

    it("composes non-overlapping field deltas from multiple features", () => {
        const deltas = [
            { featureId: "f1", delta: { content: "New content" }, priority: 1 },
            { featureId: "f2", delta: { title: "New Title" }, priority: 2 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.title).toBe("New Title");
        expect(result.content).toBe("New content");
    });

    it("higher priority overwrites lower priority on same field", () => {
        const deltas = [
            { featureId: "f1", delta: { title: "Low" }, priority: 1 },
            { featureId: "f2", delta: { title: "High" }, priority: 5 }
        ];
        const result = resolveChunk(baseChunk, deltas);
        expect(result.title).toBe("High");
    });
});

describe("resolveChunks", () => {
    it("returns chunks unchanged when no active feature IDs", () => {
        const chunks = [{ id: "c1", title: "T", content: "C" }];
        const result = resolveChunks(chunks, [], []);
        expect(result).toEqual(chunks);
    });

    it("applies deltas to matching chunks only", () => {
        const chunks = [
            { id: "c1", title: "Chunk 1", content: "Content 1" },
            { id: "c2", title: "Chunk 2", content: "Content 2" }
        ];
        const deltas = [
            { chunkId: "c1", featureId: "f1", delta: { title: "Modified 1" }, priority: 1 }
        ];
        const result = resolveChunks(chunks, ["f1"], deltas);
        expect(result[0].title).toBe("Modified 1");
        expect(result[0]._hasDeltas).toBe(true);
        expect(result[1].title).toBe("Chunk 2");
        expect(result[1]._hasDeltas).toBe(false);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && npx vitest run src/features/resolve.test.ts`

Expected: FAIL — `Cannot find module './resolve'`

- [ ] **Step 3: Implement the resolution utility**

Create `packages/api/src/features/resolve.ts`:

```typescript
interface Delta {
    featureId: string;
    delta: Record<string, unknown>;
    priority: number;
}

interface DeltaWithChunk extends Delta {
    chunkId: string;
}

export interface ResolvedMeta {
    _appliedFeatures: string[];
    _hasDeltas: boolean;
}

export function resolveChunk<T extends Record<string, unknown>>(chunk: T, deltas: Delta[]): T & ResolvedMeta {
    if (deltas.length === 0) {
        return { ...chunk, _appliedFeatures: [], _hasDeltas: false };
    }

    const sorted = [...deltas].sort((a, b) => a.priority - b.priority);
    const resolved = { ...chunk };
    const appliedFeatures: string[] = [];

    for (const d of sorted) {
        Object.assign(resolved, d.delta);
        appliedFeatures.push(d.featureId);
    }

    return { ...resolved, _appliedFeatures: appliedFeatures, _hasDeltas: true };
}

export function resolveChunks<T extends Record<string, unknown>>(
    chunks: T[],
    activeFeatureIds: string[],
    allDeltas: DeltaWithChunk[]
): (T & ResolvedMeta)[] {
    if (activeFeatureIds.length === 0 || allDeltas.length === 0) {
        return chunks.map(c => ({ ...c, _appliedFeatures: [] as string[], _hasDeltas: false }));
    }

    const deltasByChunk = new Map<string, Delta[]>();
    for (const d of allDeltas) {
        const existing = deltasByChunk.get(d.chunkId) ?? [];
        existing.push({ featureId: d.featureId, delta: d.delta, priority: d.priority });
        deltasByChunk.set(d.chunkId, existing);
    }

    return chunks.map(chunk => {
        const chunkId = (chunk as Record<string, unknown>).id as string;
        const deltas = deltasByChunk.get(chunkId) ?? [];
        return resolveChunk(chunk, deltas);
    });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/api && npx vitest run src/features/resolve.test.ts`

Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/features/resolve.ts packages/api/src/features/resolve.test.ts
git commit -m "feat: add chunk feature delta resolution utility with tests"
```

---

### Task 5: Feature service

**Files:**
- Create: `packages/api/src/features/service.ts`

- [ ] **Step 1: Create the feature service**

Create `packages/api/src/features/service.ts`:

```typescript
import {
    createFeature as createFeatureRepo,
    deleteFeature as deleteFeatureRepo,
    featureNameConflict,
    getActiveFeatureIds as getActiveFeatureIdsRepo,
    getCodebasesForFeature,
    getFeatureById,
    getMaxPriority,
    listFeatures as listFeaturesRepo,
    setActiveFeatures as setActiveFeaturesRepo,
    setFeatureCodebases,
    shiftPriorities,
    updateFeature as updateFeatureRepo
} from "@fubbik/db/repository";
import {
    batchFetchDeltas,
    deleteDeltasForFeature,
    getDeltasForChunk as getDeltasForChunkRepo,
    getDeltasForFeature as getDeltasForFeatureRepo,
    upsertDelta as upsertDeltaRepo,
    deleteDelta as deleteDeltaRepo
} from "@fubbik/db/repository";
import {
    getChunkById,
    getNextVersionNumber,
    createVersion,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";
import { enrichChunk } from "../enrich/service";
import { logger } from "../logger";

const DELTA_ALLOWED_FIELDS = new Set(["title", "content", "type", "rationale", "alternatives", "consequences", "summary"]);

function validateDelta(delta: Record<string, unknown>): Effect.Effect<Record<string, unknown>, ValidationError> {
    const invalid = Object.keys(delta).filter(k => !DELTA_ALLOWED_FIELDS.has(k));
    if (invalid.length > 0) {
        return Effect.fail(new ValidationError({ message: `Invalid delta fields: ${invalid.join(", ")}. Allowed: ${[...DELTA_ALLOWED_FIELDS].join(", ")}` }));
    }
    if (Object.keys(delta).length === 0) {
        return Effect.fail(new ValidationError({ message: "Delta must contain at least one field" }));
    }
    return Effect.succeed(delta);
}

export function createFeature(userId: string, body: {
    name: string;
    description?: string;
    priority?: number;
    color?: string;
    codebaseIds?: string[];
}) {
    const id = crypto.randomUUID();
    return (body.priority !== undefined ? Effect.succeed(body.priority) : getMaxPriority(userId).pipe(Effect.map(max => max + 1))).pipe(
        Effect.flatMap(priority =>
            createFeatureRepo({ id, name: body.name, description: body.description, priority, color: body.color, userId })
        ),
        Effect.tap(() => {
            if (body.codebaseIds && body.codebaseIds.length > 0) {
                return setFeatureCodebases(id, body.codebaseIds);
            }
            return Effect.void;
        })
    );
}

export function getFeatureDetail(featureId: string, userId: string) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(found =>
            Effect.all({
                feature: Effect.succeed(found),
                codebases: getCodebasesForFeature(featureId),
                deltas: getDeltasForFeatureRepo(featureId)
            })
        )
    );
}

export function listFeatures(userId: string, filters?: { codebaseId?: string; status?: string; search?: string }) {
    return listFeaturesRepo(userId, filters);
}

export function updateFeature(featureId: string, userId: string, body: {
    name?: string;
    description?: string | null;
    priority?: number;
    status?: string;
    color?: string | null;
    codebaseIds?: string[];
}) {
    const guard = body.name !== undefined
        ? featureNameConflict(featureId, userId, body.name).pipe(
            Effect.flatMap(conflict =>
                conflict
                    ? Effect.fail(new ValidationError({ message: `Feature "${body.name}" already exists` }))
                    : Effect.succeed(undefined)
            )
        )
        : Effect.succeed(undefined);

    const { codebaseIds, ...repoBody } = body;

    return guard.pipe(
        Effect.flatMap(() => updateFeatureRepo(featureId, userId, repoBody)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.tap(() => {
            if (codebaseIds !== undefined) {
                return setFeatureCodebases(featureId, codebaseIds);
            }
            return Effect.void;
        })
    );
}

export function deleteFeatureService(featureId: string, userId: string) {
    return deleteFeatureRepo(featureId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Feature" }))))
    );
}

export function reorderFeature(featureId: string, userId: string, newPriority: number) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(existing => {
            if (existing.priority === newPriority) return Effect.succeed(existing);
            return shiftPriorities(userId, newPriority, "up").pipe(
                Effect.flatMap(() => updateFeatureRepo(featureId, userId, { priority: newPriority })),
                Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Feature" }))))
            );
        })
    );
}

// --- Active features ---

export function getActiveFeatures(userId: string) {
    return getActiveFeatureIdsRepo(userId).pipe(
        Effect.map(rows => rows.map(r => r.featureId))
    );
}

export function setActiveFeatures(userId: string, featureIds: string[]) {
    return setActiveFeaturesRepo(userId, featureIds);
}

// --- Deltas ---

export function getDeltasForChunk(chunkId: string) {
    return getDeltasForChunkRepo(chunkId);
}

export function getDeltasForFeature(featureId: string) {
    return getDeltasForFeatureRepo(featureId);
}

export function upsertDelta(chunkId: string, featureId: string, userId: string, delta: Record<string, unknown>) {
    return validateDelta(delta).pipe(
        Effect.flatMap(() => getFeatureById(featureId, userId)),
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(() => getChunkById(chunkId, userId)),
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => upsertDeltaRepo({ id: crypto.randomUUID(), chunkId, featureId, delta }))
    );
}

export function deleteDelta(chunkId: string, featureId: string, userId: string) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(() => deleteDeltaRepo(chunkId, featureId)),
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Delta" }))))
    );
}

// --- Merge ---

export function mergeFeature(featureId: string, userId: string) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(found => {
            if (found.status === "merged") {
                return Effect.fail(new ValidationError({ message: "Feature is already merged" }));
            }
            return getDeltasForFeatureRepo(featureId);
        }),
        Effect.flatMap(deltas => {
            if (deltas.length === 0) {
                return updateFeatureRepo(featureId, userId, { status: "merged" }).pipe(Effect.asVoid);
            }

            // Apply each delta to its base chunk, creating version snapshots
            const applyAll = deltas.map(deltaRow =>
                getChunkById(deltaRow.chunkId, userId).pipe(
                    Effect.flatMap(existing => {
                        if (!existing) return Effect.void;
                        return getNextVersionNumber(deltaRow.chunkId).pipe(
                            Effect.flatMap(version =>
                                createVersion({
                                    id: crypto.randomUUID(),
                                    chunkId: deltaRow.chunkId,
                                    version,
                                    title: existing.title,
                                    content: existing.content,
                                    type: existing.type,
                                    tags: []
                                })
                            ),
                            Effect.flatMap(() => {
                                const updateData = deltaRow.delta as Record<string, unknown>;
                                return updateChunkRepo(deltaRow.chunkId, updateData as Parameters<typeof updateChunkRepo>[1]);
                            })
                        );
                    })
                )
            );

            return Effect.all(applyAll, { concurrency: 3 }).pipe(
                Effect.flatMap(() => deleteDeltasForFeature(featureId)),
                Effect.flatMap(() => updateFeatureRepo(featureId, userId, { status: "merged" })),
                Effect.tap(() => {
                    // Fire-and-forget re-enrichment for affected chunks
                    for (const deltaRow of deltas) {
                        Effect.runPromise(enrichChunk(deltaRow.chunkId)).catch(err => {
                            logger.error(`[merge] Failed to re-enrich chunk ${deltaRow.chunkId}:`, { err });
                        });
                    }
                    return Effect.void;
                })
            );
        })
    );
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/features/service.ts
git commit -m "feat: add feature service with CRUD, delta management, and merge flow"
```

---

### Task 6: Feature routes

**Files:**
- Create: `packages/api/src/features/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create the feature routes**

Create `packages/api/src/features/routes.ts`:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as featureService from "./service";

export const featureRoutes = new Elysia()
    .get(
        "/features",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        featureService.listFeatures(session.user.id, {
                            codebaseId: ctx.query.codebaseId,
                            status: ctx.query.status,
                            search: ctx.query.search
                        })
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                status: t.Optional(t.String()),
                search: t.Optional(t.String())
            })
        }
    )
    .post(
        "/features",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.createFeature(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 }),
                description: t.Optional(t.String({ maxLength: 1000 })),
                priority: t.Optional(t.Number()),
                color: t.Optional(t.String({ maxLength: 7 })),
                codebaseIds: t.Optional(t.Array(t.String()))
            })
        }
    )
    .get("/features/active", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.getActiveFeatures(session.user.id))
            )
        )
    )
    .put(
        "/features/active",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.setActiveFeatures(session.user.id, ctx.body.featureIds)),
                    Effect.map(() => ({ message: "Active features updated" }))
                )
            ),
        {
            body: t.Object({
                featureIds: t.Array(t.String())
            })
        }
    )
    .get("/features/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.getFeatureDetail(ctx.params.id, session.user.id))
            )
        )
    )
    .patch(
        "/features/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.updateFeature(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
                priority: t.Optional(t.Number()),
                status: t.Optional(t.Union([t.Literal("active"), t.Literal("inactive"), t.Literal("archived")])),
                color: t.Optional(t.Union([t.String({ maxLength: 7 }), t.Null()])),
                codebaseIds: t.Optional(t.Array(t.String()))
            })
        }
    )
    .delete("/features/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.deleteFeatureService(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    )
    .post("/features/:id/merge", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => featureService.mergeFeature(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Feature merged" }))
            )
        )
    )
    .post(
        "/features/:id/reorder",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => featureService.reorderFeature(ctx.params.id, session.user.id, ctx.body.priority))
                )
            ),
        {
            body: t.Object({
                priority: t.Number()
            })
        }
    )
    // --- Delta endpoints mounted under chunks ---
    .get("/chunks/:id/deltas", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => featureService.getDeltasForChunk(ctx.params.id))
            )
        )
    )
    .put(
        "/chunks/:id/deltas/:featureId",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        featureService.upsertDelta(ctx.params.id, ctx.params.featureId, session.user.id, ctx.body.delta)
                    )
                )
            ),
        {
            body: t.Object({
                delta: t.Record(t.String(), t.Unknown())
            })
        }
    )
    .delete("/chunks/:id/deltas/:featureId", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    featureService.deleteDelta(ctx.params.id, ctx.params.featureId, session.user.id)
                ),
                Effect.map(() => ({ message: "Delta deleted" }))
            )
        )
    )
    .get("/features/:id/deltas", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => {
                    // Verify ownership by fetching the feature first
                    return featureService.getFeatureDetail(ctx.params.id, session.user.id).pipe(
                        Effect.map(detail => detail.deltas)
                    );
                })
            )
        )
    );
```

- [ ] **Step 2: Mount routes in API index**

In `packages/api/src/index.ts`, add the import at the top with the other route imports:

```typescript
import { featureRoutes } from "./features/routes";
```

Add `.use(featureRoutes)` to the chain, after `.use(workspaceRoutes)`:

```typescript
    .use(workspaceRoutes)
    .use(featureRoutes)
    .use(stalenessRoutes)
```

- [ ] **Step 3: Verify compilation**

Run: `pnpm run check-types`

Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/features/routes.ts packages/api/src/index.ts
git commit -m "feat: add feature routes and mount in API"
```

---

### Task 7: Active feature context injection

**Files:**
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Add active feature resolution to the `.resolve()` block**

In `packages/api/src/index.ts`, add the import:

```typescript
import { getActiveFeatureIds } from "@fubbik/db/repository";
import { Effect } from "effect";
```

Note: `Effect` is already imported. Only add the `getActiveFeatureIds` import.

Modify the existing `.resolve()` block (lines 133-136) to also fetch active features:

```typescript
    .resolve(async ({ headers }) => {
        const session = await getSession(new Headers(headers as Record<string, string>));
        let activeFeatureIds: string[] = [];
        if (session) {
            try {
                const rows = await Effect.runPromise(getActiveFeatureIds(session.user.id));
                activeFeatureIds = rows.map(r => r.featureId);
            } catch {
                // Non-critical — proceed without feature resolution
            }
        }
        return { session: session ?? undefined, activeFeatureIds };
    })
```

- [ ] **Step 2: Verify compilation**

Run: `pnpm run check-types`

Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/index.ts
git commit -m "feat: inject active feature IDs into request context"
```

---

### Task 8: Chunk service feature resolution integration

**Files:**
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Add feature resolution to `listChunks`**

In `packages/api/src/chunks/service.ts`, add imports at the top:

```typescript
import { batchFetchDeltas, getDeltasForChunk as getDeltasForChunkRepo } from "@fubbik/db/repository";
import { resolveChunks } from "../features/resolve";
```

Modify the `listChunks` function. After the final `.pipe(Effect.flatMap(...))` that returns the result, add resolution. Change the return to wrap with resolution. The function currently returns:

```typescript
    return listChunksRepo({ ... }).pipe(
        Effect.flatMap(result => {
            ...
            return Effect.succeed({ ...result, chunks, limit, offset });
        })
    );
```

Add a second `Effect.flatMap` to resolve features. Add `activeFeatureIds: string[] = []` as a third parameter to `listChunks`:

```typescript
export function listChunks(
    userId: string | undefined,
    query: { ... },
    activeFeatureIds: string[] = []
) {
```

After the existing `.pipe(Effect.flatMap(...))`, chain another:

```typescript
    ).pipe(
        Effect.flatMap(result => {
            if (activeFeatureIds.length === 0 || result.chunks.length === 0) {
                return Effect.succeed(result);
            }
            const chunkIds = result.chunks.map((c: { id: string }) => c.id);
            return batchFetchDeltas(chunkIds, activeFeatureIds).pipe(
                Effect.map(deltas => ({
                    ...result,
                    chunks: resolveChunks(result.chunks, activeFeatureIds, deltas as any)
                }))
            );
        })
    );
```

- [ ] **Step 2: Add feature resolution to `getChunkDetail`**

Add `activeFeatureIds: string[] = []` as a third parameter to `getChunkDetail`:

```typescript
export function getChunkDetail(chunkId: string, userId?: string, activeFeatureIds: string[] = []) {
```

At the end of the `getChunkDetail` function, before the final `Effect.map(result => { ... return { ...result, healthScore }; })`, add delta fetching. Modify the `Effect.all` block to include deltas:

```typescript
        Effect.flatMap(found =>
            Effect.all({
                chunk: Effect.succeed(found),
                connections: getChunkConnections(chunkId),
                codebases: getCodebasesForChunk(chunkId),
                appliesTo: getAppliesToForChunk(chunkId),
                fileReferences: getFileRefsForChunk(chunkId),
                tags: getTagsForChunk(chunkId),
                requirements: getRequirementsForChunks([chunkId]),
                allDeltas: getDeltasForChunkRepo(chunkId)
            })
        ),
```

Then in the final `Effect.map`, apply resolution and include deltas in the response:

```typescript
        Effect.map(result => {
            const chunkRequirements = result.requirements.filter(r => r.chunkId === chunkId);
            const requirementCount = chunkRequirements.length;
            const allRequirementsPassing = requirementCount > 0 && chunkRequirements.every(r => r.status === "passing");
            const healthScore = computeHealthScore({ ... });

            // Resolve chunk through active features
            let resolvedChunk = result.chunk;
            let _appliedFeatures: string[] = [];
            let _hasDeltas = result.allDeltas.length > 0;
            if (activeFeatureIds.length > 0 && result.allDeltas.length > 0) {
                const activeDeltas = result.allDeltas
                    .filter(d => activeFeatureIds.includes(d.featureId))
                    .sort((a, b) => a.featurePriority - b.featurePriority);
                for (const d of activeDeltas) {
                    resolvedChunk = { ...resolvedChunk, ...(d.delta as Record<string, unknown>) };
                    _appliedFeatures.push(d.featureId);
                }
            }

            return {
                ...result,
                chunk: resolvedChunk,
                healthScore,
                _appliedFeatures,
                _hasDeltas,
                deltas: result.allDeltas
            };
        })
```

- [ ] **Step 3: Thread `activeFeatureIds` from chunk routes**

In `packages/api/src/chunks/routes.ts`, update the `GET /chunks` handler to pass `activeFeatureIds` from context:

```typescript
    .get(
        "/chunks",
        ctx => Effect.runPromise(requireSession(ctx).pipe(
            Effect.flatMap(session => chunkService.listChunks(session.user.id, ctx.query, (ctx as any).activeFeatureIds ?? []))
        )),
```

Update the `GET /chunks/:id` handler similarly:

```typescript
        Effect.flatMap(session => chunkService.getChunkDetail(ctx.params.id, session.user.id, (ctx as any).activeFeatureIds ?? []))
```

- [ ] **Step 4: Verify compilation**

Run: `pnpm run check-types`

Expected: No type errors.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/service.ts packages/api/src/chunks/routes.ts
git commit -m "feat: integrate feature resolution into chunk list and detail endpoints"
```

---

### Task 9: Frontend — active features hook

**Files:**
- Create: `apps/web/src/features/feature-flags/use-active-features.ts`

- [ ] **Step 1: Create the hook**

Create `apps/web/src/features/feature-flags/use-active-features.ts`:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function useActiveFeatures() {
    const queryClient = useQueryClient();

    const { data: activeFeatureIds = [] } = useQuery({
        queryKey: ["features", "active"],
        queryFn: async () => unwrapEden(await api.api.features.active.get()),
        staleTime: 60_000
    });

    const toggleMutation = useMutation({
        mutationFn: async (featureIds: string[]) => {
            unwrapEden(await api.api.features.active.put({ featureIds }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features", "active"] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
        }
    });

    const toggleFeature = (featureId: string) => {
        const current = activeFeatureIds as string[];
        const next = current.includes(featureId)
            ? current.filter(id => id !== featureId)
            : [...current, featureId];
        toggleMutation.mutate(next);
    };

    const isActive = (featureId: string) => (activeFeatureIds as string[]).includes(featureId);

    return { activeFeatureIds: activeFeatureIds as string[], toggleFeature, isActive, isUpdating: toggleMutation.isPending };
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/feature-flags/use-active-features.ts
git commit -m "feat: add useActiveFeatures hook for toggling feature overlays"
```

---

### Task 10: Frontend — feature switcher component

**Files:**
- Create: `apps/web/src/features/feature-flags/feature-switcher.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Create the feature switcher**

Create `apps/web/src/features/feature-flags/feature-switcher.tsx`:

```typescript
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Flag } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { useActiveFeatures } from "./use-active-features";

export function FeatureSwitcher() {
    const { activeFeatureIds, toggleFeature, isActive } = useActiveFeatures();

    const { data: features } = useQuery({
        queryKey: ["features"],
        queryFn: async () => unwrapEden(await api.api.features.get({ query: {} })),
        staleTime: 60_000
    });

    if (!features || features.length === 0) return null;

    const activeCount = activeFeatureIds.length;

    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" size="sm" className="relative gap-1.5" />}>
                <Flag className="size-3.5" />
                <span className="hidden sm:inline">Features</span>
                {activeCount > 0 && (
                    <span className="flex size-4 items-center justify-center rounded-full bg-blue-500 text-[10px] font-bold text-white">
                        {activeCount}
                    </span>
                )}
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
                <DropdownMenuLabel>Feature Overlays</DropdownMenuLabel>
                {(features as Array<{ id: string; name: string; color: string | null; priority: number; status: string; deltaCount: number }>).map(f => (
                    <DropdownMenuItem
                        key={f.id}
                        onClick={() => toggleFeature(f.id)}
                        className="flex items-center justify-between"
                    >
                        <span className="flex items-center gap-2">
                            <span
                                className="size-2 rounded-full"
                                style={{ backgroundColor: f.color ?? "#8b5cf6" }}
                            />
                            <span>{f.name}</span>
                            <span className="text-muted-foreground text-xs">({f.deltaCount})</span>
                        </span>
                        <span className={`size-3 rounded-sm border ${isActive(f.id) ? "border-blue-500 bg-blue-500" : "border-muted-foreground"}`} />
                    </DropdownMenuItem>
                ))}
                <DropdownMenuSeparator />
                <DropdownMenuItem render={<Link to="/features" />}>
                    Manage Features
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
```

- [ ] **Step 2: Add the switcher to the nav**

In `apps/web/src/routes/__root.tsx`, add the import:

```typescript
import { FeatureSwitcher } from "@/features/feature-flags/feature-switcher";
```

Add `<FeatureSwitcher />` right after `<CodebaseSwitcher />` (line 91):

```typescript
                                    <CodebaseSwitcher />
                                    <FeatureSwitcher />
```

- [ ] **Step 3: Verify the app builds**

Run: `pnpm build`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/feature-flags/feature-switcher.tsx apps/web/src/routes/__root.tsx
git commit -m "feat: add feature switcher dropdown in nav bar"
```

---

### Task 11: Frontend — features management page

**Files:**
- Create: `apps/web/src/routes/features.tsx`

- [ ] **Step 1: Create the features page**

Create `apps/web/src/routes/features.tsx`:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Flag, Plus, Trash2, Merge, Archive } from "lucide-react";
import { useState } from "react";

import { PageContainer, PageHeader, PageLoading, PageEmpty } from "@/components/ui/page";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { useActiveFeatures } from "@/features/feature-flags/use-active-features";
import { toast } from "sonner";
import { getUser } from "@/utils/auth";

export const Route = createFileRoute("/features")({
    component: FeaturesPage,
    beforeLoad: async () => {
        let session = null;
        try { session = await getUser(); } catch {}
        return { session };
    }
});

function FeaturesPage() {
    const queryClient = useQueryClient();
    const { isActive, toggleFeature } = useActiveFeatures();
    const [createOpen, setCreateOpen] = useState(false);
    const [mergeTarget, setMergeTarget] = useState<{ id: string; name: string } | null>(null);

    const { data: features, isLoading } = useQuery({
        queryKey: ["features"],
        queryFn: async () => unwrapEden(await api.api.features.get({ query: {} })),
        staleTime: 60_000
    });

    const createMutation = useMutation({
        mutationFn: async (body: { name: string; description?: string; color?: string }) =>
            unwrapEden(await api.api.features.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            setCreateOpen(false);
            toast.success("Feature created");
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api.features({ id }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            toast.success("Feature deleted");
        }
    });

    const mergeMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api.features({ id }).merge.post({})),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            setMergeTarget(null);
            toast.success("Feature merged into base chunks");
        }
    });

    const archiveMutation = useMutation({
        mutationFn: async (id: string) =>
            unwrapEden(await api.api.features({ id }).patch({ status: "archived" })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            toast.success("Feature archived");
        }
    });

    if (isLoading) return <PageLoading />;

    const featureList = (features ?? []) as Array<{
        id: string;
        name: string;
        description: string | null;
        priority: number;
        status: string;
        color: string | null;
        deltaCount: number;
        createdAt: string;
    }>;

    return (
        <PageContainer>
            <PageHeader icon={Flag} title="Features" count={featureList.length}>
                <Button onClick={() => setCreateOpen(true)} size="sm">
                    <Plus className="mr-1 size-4" />
                    New Feature
                </Button>
            </PageHeader>

            {featureList.length === 0 ? (
                <PageEmpty message="No features yet. Create one to start tracking knowledge overlays." />
            ) : (
                <div className="space-y-3">
                    {featureList.map(f => (
                        <div
                            key={f.id}
                            className="border-border flex items-center justify-between rounded-lg border p-4"
                        >
                            <div className="flex items-center gap-3">
                                <span
                                    className="size-3 rounded-full"
                                    style={{ backgroundColor: f.color ?? "#8b5cf6" }}
                                />
                                <div>
                                    <div className="flex items-center gap-2">
                                        <span className="font-medium">{f.name}</span>
                                        <span className="bg-muted text-muted-foreground rounded px-1.5 py-0.5 text-xs">
                                            P{f.priority}
                                        </span>
                                        <span className={`rounded px-1.5 py-0.5 text-xs ${
                                            f.status === "merged" ? "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200"
                                            : f.status === "archived" ? "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400"
                                            : isActive(f.id) ? "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200"
                                            : "bg-muted text-muted-foreground"
                                        }`}>
                                            {isActive(f.id) && f.status !== "merged" && f.status !== "archived" ? "active" : f.status}
                                        </span>
                                    </div>
                                    {f.description && (
                                        <p className="text-muted-foreground mt-1 text-sm">{f.description}</p>
                                    )}
                                    <p className="text-muted-foreground mt-1 text-xs">
                                        {f.deltaCount} chunk {f.deltaCount === 1 ? "delta" : "deltas"}
                                    </p>
                                </div>
                            </div>
                            <div className="flex items-center gap-1">
                                {f.status !== "merged" && f.status !== "archived" && (
                                    <>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => toggleFeature(f.id)}
                                            className={isActive(f.id) ? "text-blue-600" : ""}
                                        >
                                            {isActive(f.id) ? "Deactivate" : "Activate"}
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => setMergeTarget({ id: f.id, name: f.name })}
                                            disabled={f.deltaCount === 0}
                                        >
                                            <Merge className="size-4" />
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => archiveMutation.mutate(f.id)}
                                        >
                                            <Archive className="size-4" />
                                        </Button>
                                    </>
                                )}
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => deleteMutation.mutate(f.id)}
                                    className="text-destructive"
                                >
                                    <Trash2 className="size-4" />
                                </Button>
                            </div>
                        </div>
                    ))}
                </div>
            )}

            {/* Create Dialog */}
            <CreateFeatureDialog
                open={createOpen}
                onOpenChange={setCreateOpen}
                onSubmit={(body) => createMutation.mutate(body)}
                isPending={createMutation.isPending}
            />

            {/* Merge Confirmation Dialog */}
            <Dialog open={!!mergeTarget} onOpenChange={() => setMergeTarget(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Merge Feature</DialogTitle>
                        <DialogDescription>
                            This will permanently apply all deltas from "{mergeTarget?.name}" to their base chunks.
                            This cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setMergeTarget(null)}>Cancel</Button>
                        <Button
                            onClick={() => mergeTarget && mergeMutation.mutate(mergeTarget.id)}
                            disabled={mergeMutation.isPending}
                        >
                            {mergeMutation.isPending ? "Merging..." : "Merge"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </PageContainer>
    );
}

function CreateFeatureDialog({
    open,
    onOpenChange,
    onSubmit,
    isPending
}: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSubmit: (body: { name: string; description?: string; color?: string }) => void;
    isPending: boolean;
}) {
    const [name, setName] = useState("");
    const [description, setDescription] = useState("");
    const [color, setColor] = useState("#8b5cf6");

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        onSubmit({ name, description: description || undefined, color });
        setName("");
        setDescription("");
        setColor("#8b5cf6");
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <form onSubmit={handleSubmit}>
                    <DialogHeader>
                        <DialogTitle>Create Feature</DialogTitle>
                        <DialogDescription>
                            Create a named feature to track knowledge overlays.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="mt-4 space-y-4">
                        <div>
                            <Label htmlFor="feature-name">Name</Label>
                            <Input
                                id="feature-name"
                                value={name}
                                onChange={e => setName(e.target.value)}
                                placeholder="e.g. OAuth Migration"
                                required
                            />
                        </div>
                        <div>
                            <Label htmlFor="feature-description">Description</Label>
                            <Textarea
                                id="feature-description"
                                value={description}
                                onChange={e => setDescription(e.target.value)}
                                placeholder="Optional description"
                                rows={3}
                            />
                        </div>
                        <div>
                            <Label htmlFor="feature-color">Color</Label>
                            <div className="flex items-center gap-2">
                                <input
                                    id="feature-color"
                                    type="color"
                                    value={color}
                                    onChange={e => setColor(e.target.value)}
                                    className="h-8 w-12 cursor-pointer rounded border-0"
                                />
                                <span className="text-muted-foreground text-sm">{color}</span>
                            </div>
                        </div>
                    </div>
                    <DialogFooter className="mt-4">
                        <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
                        <Button type="submit" disabled={!name.trim() || isPending}>
                            {isPending ? "Creating..." : "Create"}
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}
```

- [ ] **Step 2: Verify the app builds**

Run: `pnpm build`

Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/features.tsx
git commit -m "feat: add features management page"
```

---

### Task 12: Frontend — chunk detail delta indicators

**Files:**
- Modify: `apps/web/src/routes/chunks.$id.tsx` (or wherever the chunk detail component lives)

- [ ] **Step 1: Find the chunk detail page**

Run: `ls apps/web/src/routes/chunks*` to locate the exact file path.

- [ ] **Step 2: Add delta overlay section to chunk detail**

At the bottom of the chunk detail page (after existing sections like connections, appliesTo, etc.), add a "Feature Overlays" section. The data is already available in the API response via the `deltas` and `_appliedFeatures` fields added in Task 8.

Add the following section component:

```typescript
function FeatureOverlaysSection({ deltas, appliedFeatures }: {
    deltas: Array<{
        id: string;
        featureId: string;
        featureName: string;
        featureColor: string | null;
        featureStatus: string;
        delta: Record<string, unknown>;
    }>;
    appliedFeatures: string[];
}) {
    if (deltas.length === 0) return null;

    return (
        <section className="space-y-3">
            <h3 className="text-sm font-medium">Feature Overlays</h3>
            <div className="space-y-2">
                {deltas.map(d => (
                    <div key={d.id} className="border-border flex items-center justify-between rounded-md border px-3 py-2">
                        <div className="flex items-center gap-2">
                            <span
                                className="size-2 rounded-full"
                                style={{ backgroundColor: d.featureColor ?? "#8b5cf6" }}
                            />
                            <span className="text-sm font-medium">{d.featureName}</span>
                            {appliedFeatures.includes(d.featureId) && (
                                <span className="rounded bg-blue-100 px-1.5 py-0.5 text-xs text-blue-800 dark:bg-blue-900 dark:text-blue-200">
                                    active
                                </span>
                            )}
                        </div>
                        <span className="text-muted-foreground text-xs">
                            {Object.keys(d.delta).join(", ")}
                        </span>
                    </div>
                ))}
            </div>
        </section>
    );
}
```

Render it in the detail page layout, passing the data from the API response:

```typescript
<FeatureOverlaysSection
    deltas={chunkDetail.deltas ?? []}
    appliedFeatures={chunkDetail._appliedFeatures ?? []}
/>
```

- [ ] **Step 3: Verify the app builds**

Run: `pnpm build`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.\$id.tsx
git commit -m "feat: show feature overlay indicators on chunk detail page"
```

---

### Task 13: Frontend — chunk edit save-to-feature dialog

**Files:**
- Modify: The chunk edit page (likely `apps/web/src/routes/chunks.$id.edit.tsx`)

- [ ] **Step 1: Find the chunk edit page**

Run: `ls apps/web/src/routes/chunks*` to locate the edit route file.

- [ ] **Step 2: Add save-to-feature dialog**

In the chunk edit form's submit handler, add a check: if any features are active, show a dialog asking where to save.

Create a `SaveTargetDialog` component:

```typescript
function SaveTargetDialog({
    open,
    onOpenChange,
    activeFeatures,
    onSave
}: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    activeFeatures: Array<{ id: string; name: string; color: string | null }>;
    onSave: (target: "base" | string) => void;
}) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Save to</DialogTitle>
                    <DialogDescription>
                        You have active features. Save changes to the base chunk or as a feature overlay.
                    </DialogDescription>
                </DialogHeader>
                <div className="space-y-2">
                    <Button
                        variant="outline"
                        className="w-full justify-start"
                        onClick={() => onSave("base")}
                    >
                        Base chunk
                    </Button>
                    {activeFeatures.map(f => (
                        <Button
                            key={f.id}
                            variant="outline"
                            className="w-full justify-start gap-2"
                            onClick={() => onSave(f.id)}
                        >
                            <span
                                className="size-2 rounded-full"
                                style={{ backgroundColor: f.color ?? "#8b5cf6" }}
                            />
                            Feature: {f.name}
                        </Button>
                    ))}
                </div>
            </DialogContent>
        </Dialog>
    );
}
```

In the submit handler logic:
- If `target === "base"`, save normally via `PATCH /api/chunks/:id`.
- If `target` is a feature ID, compute the delta (diff the edited fields against the base chunk), then call `PUT /api/chunks/:id/deltas/:featureId` with the delta.

Use the `useActiveFeatures` hook to check if features are active and decide whether to show the dialog.

- [ ] **Step 3: Verify the app builds**

Run: `pnpm build`

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.\$id.edit.tsx
git commit -m "feat: add save-to-feature dialog when editing chunks with active features"
```

---

### Task 14: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add feature overlays documentation**

Add a new section under "## Core Concepts" for Features:

```markdown
### Features (Knowledge Overlays)

Named entities that track field-level modifications to chunks. Each feature stores deltas — sparse JSONB objects containing only the changed fields. Features have a priority (higher wins on same-field conflicts) and can be toggled on/off globally via the nav switcher.

- `feature` table: `name`, `description`, `priority` (unique per user), `status` (inactive/active/merged/archived), `color`, `userId`
- `feature_codebase` join table (optional codebase association)
- `chunk_feature_delta` table: one delta per chunk per feature, contains JSONB with changed content fields only
- `user_active_feature` table: persists which features are active per user
- Resolution: `Object.assign(baseChunk, ...deltasAscByPriority)` — applied at the service layer
- Merge: permanently applies all deltas to base chunks, sets feature status to `merged`
```

Add to the API endpoints section:

```markdown
### Features
- `GET /api/features` — list (filters: `codebaseId`, `status`, `search`)
- `POST /api/features` — create
- `GET /api/features/:id` — detail with codebases and deltas
- `PATCH /api/features/:id` — update
- `DELETE /api/features/:id` — delete
- `POST /api/features/:id/merge` — merge all deltas into base chunks
- `POST /api/features/:id/reorder` — change priority
- `GET /api/features/active` — get user's active feature IDs
- `PUT /api/features/active` — set active features
- `GET /api/features/:id/deltas` — all deltas for a feature
- `GET /api/chunks/:id/deltas` — all deltas for a chunk
- `PUT /api/chunks/:id/deltas/:featureId` — upsert delta
- `DELETE /api/chunks/:id/deltas/:featureId` — delete delta
```

Add `/features` to the Web Pages section.

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add feature overlays to CLAUDE.md"
```

---

## Summary

| Task | Description | New Files | Modified Files |
|------|-------------|-----------|----------------|
| 1 | Schema definitions | `packages/db/src/schema/feature.ts` | `packages/db/src/schema/index.ts` |
| 2 | Feature repository | `packages/db/src/repository/feature.ts` | `packages/db/src/repository/index.ts` |
| 3 | Delta repository | `packages/db/src/repository/chunk-feature-delta.ts` | `packages/db/src/repository/index.ts` |
| 4 | Resolution utility + tests | `packages/api/src/features/resolve.ts`, `resolve.test.ts` | — |
| 5 | Feature service | `packages/api/src/features/service.ts` | — |
| 6 | Feature routes + mount | `packages/api/src/features/routes.ts` | `packages/api/src/index.ts` |
| 7 | Active feature context injection | — | `packages/api/src/index.ts` |
| 8 | Chunk service integration | — | `chunks/service.ts`, `chunks/routes.ts` |
| 9 | Active features hook | `use-active-features.ts` | — |
| 10 | Feature switcher component | `feature-switcher.tsx` | `__root.tsx` |
| 11 | Features management page | `routes/features.tsx` | — |
| 12 | Chunk detail delta indicators | — | `chunks.$id.tsx` |
| 13 | Chunk edit save-to dialog | — | `chunks.$id.edit.tsx` |
| 14 | Update CLAUDE.md | — | `CLAUDE.md` |
