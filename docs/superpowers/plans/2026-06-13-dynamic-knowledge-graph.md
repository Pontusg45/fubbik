# Dynamic Knowledge Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make fubbik's knowledge graph dynamic — usage-weighted edges, code-derived nodes, timeline scrubber, impact propagation with degree scoring, and emergent concept nodes — all on the existing Apache AGE integration.

**Architecture:** PostgreSQL stays the system of record; the AGE `knowledge` graph expands with new vertex labels (`code_file`, `code_symbol`, `concept`) and edge types (`defines`, `imports`, `annotates`, `co_referenced`, `embodies`, `impacted_by`). Two new PostgreSQL tables (`usage_event`, `graph_event`) provide the append-only event streams that power usage tracking and timeline scrubbing. The AGE sync layer in `packages/db/src/age/sync.ts` is extended with new helpers and an event-log hook.

**Tech Stack:** Apache AGE (Cypher), Drizzle ORM, Effect, tree-sitter (WASM), React Flow, vitest

**Spec:** `docs/superpowers/specs/2026-06-13-dynamic-knowledge-graph-design.md`

---

## File Map

### New Files

| File | Responsibility |
|---|---|
| `packages/db/src/schema/usage-event.ts` | `usage_event` table schema |
| `packages/db/src/schema/graph-event.ts` | `graph_event` table schema |
| `packages/db/src/repository/usage-event.ts` | Insert & query usage events |
| `packages/db/src/repository/graph-event.ts` | Insert & query graph events, reconstruct graph at time T |
| `packages/db/src/age/sync-logged.ts` | Wrappers around `sync.ts` that also write to `graph_event` |
| `packages/db/src/age/impact.ts` | Degree-scored impact propagation via Cypher |
| `packages/api/src/usage/service.ts` | Usage event recording + co-reference aggregation |
| `packages/api/src/usage/routes.ts` | Usage API endpoints |
| `packages/api/src/code-index/service.ts` | Tree-sitter indexing + AGE population |
| `packages/api/src/code-index/routes.ts` | Code index API endpoints |
| `packages/api/src/concepts/service.ts` | Emergent concept detection + lifecycle |
| `packages/api/src/concepts/routes.ts` | Concepts API endpoints |
| `packages/api/src/graph/timeline-service.ts` | Graph reconstruction at time T |
| `apps/web/src/features/graph/timeline-scrubber.tsx` | Timeline slider UI component |
| `apps/web/src/features/graph/code-node.tsx` | React Flow node for code_file/code_symbol |
| `apps/web/src/features/graph/concept-node.tsx` | React Flow node for concept vertices |

### Modified Files

| File | Change |
|---|---|
| `packages/db/src/schema/index.ts` | Export new schemas |
| `packages/db/src/repository/index.ts` | Export new repositories |
| `packages/api/src/events/bus.ts` | Add new event types |
| `packages/api/src/events/handlers.ts` | Register usage + impact handlers |
| `packages/api/src/staleness/detect-impact.ts` | Replace flat 3-hop with degree-scored propagation |
| `packages/api/src/graph/service.ts` | Include code nodes, concepts, co-ref edges in graph response |
| `packages/api/src/graph/routes.ts` | Add `/graph/at` and `/graph/events` endpoints |
| `packages/api/src/startup.ts` | Register co-reference aggregation job |
| `packages/api/src/index.ts` | Mount new route modules |
| `packages/api/src/chunks/chunk-mutations.ts` | Emit usage events on view; use logged sync |
| `apps/web/src/features/graph/use-graph-data.ts` | Fetch + merge code nodes, concepts, co-ref edges |
| `apps/web/src/features/graph/use-graph-nodes.ts` | Add code/concept node types + co-ref edge styling |
| `apps/web/src/features/graph/use-graph-state.ts` | Add code-node toggle + timeline state |
| `apps/web/src/features/graph/graph-view.tsx` | Register new node types, add timeline scrubber + code toggle |
| `apps/web/src/features/graph/typed-edge.tsx` | Add `co_referenced` edge style with weight-based thickness |

---

## Phase 1: Foundation

### Task 1: Usage Event Schema

**Files:**
- Create: `packages/db/src/schema/usage-event.ts`
- Modify: `packages/db/src/schema/index.ts`
- Test: `packages/db/src/__tests__/schema/usage-event.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/db/src/__tests__/schema/usage-event.test.ts
import { describe, expect, it } from "vitest";
import { getTableColumns } from "drizzle-orm";
import { usageEvent } from "../../schema/usage-event";

describe("usageEvent schema", () => {
    it("has required columns", () => {
        const cols = getTableColumns(usageEvent);
        expect(cols.id).toBeDefined();
        expect(cols.kind).toBeDefined();
        expect(cols.chunkIds).toBeDefined();
        expect(cols.query).toBeDefined();
        expect(cols.userId).toBeDefined();
        expect(cols.createdAt).toBeDefined();
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/__tests__/schema/usage-event.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Write the schema**

```typescript
// packages/db/src/schema/usage-event.ts
import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const usageEvent = pgTable(
    "usage_event",
    {
        id: text("id").primaryKey(),
        kind: text("kind").notNull(), // "context_query" | "chunk_view" | "mcp_resolve"
        chunkIds: jsonb("chunk_ids").$type<string[]>().notNull(),
        query: text("query"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("usage_event_kind_idx").on(table.kind),
        index("usage_event_userId_idx").on(table.userId),
        index("usage_event_createdAt_idx").on(table.createdAt)
    ]
);

export const usageEventRelations = relations(usageEvent, ({ one }) => ({
    user: one(user, { fields: [usageEvent.userId], references: [user.id] })
}));

export type UsageEvent = typeof usageEvent.$inferSelect;
export type NewUsageEvent = typeof usageEvent.$inferInsert;
```

- [ ] **Step 4: Export from schema index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./usage-event";
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/__tests__/schema/usage-event.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/usage-event.ts packages/db/src/schema/index.ts packages/db/src/__tests__/schema/usage-event.test.ts
git commit -m "feat(db): add usage_event schema"
```

---

### Task 2: Graph Event Schema

**Files:**
- Create: `packages/db/src/schema/graph-event.ts`
- Modify: `packages/db/src/schema/index.ts`
- Test: `packages/db/src/__tests__/schema/graph-event.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/db/src/__tests__/schema/graph-event.test.ts
import { describe, expect, it } from "vitest";
import { getTableColumns } from "drizzle-orm";
import { graphEvent } from "../../schema/graph-event";

describe("graphEvent schema", () => {
    it("has required columns", () => {
        const cols = getTableColumns(graphEvent);
        expect(cols.id).toBeDefined();
        expect(cols.vertexLabel).toBeDefined();
        expect(cols.vertexId).toBeDefined();
        expect(cols.edgeType).toBeDefined();
        expect(cols.edgeTargetId).toBeDefined();
        expect(cols.action).toBeDefined();
        expect(cols.snapshot).toBeDefined();
        expect(cols.createdAt).toBeDefined();
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/__tests__/schema/graph-event.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the schema**

```typescript
// packages/db/src/schema/graph-event.ts
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

export const graphEvent = pgTable(
    "graph_event",
    {
        id: text("id").primaryKey(),
        vertexLabel: text("vertex_label").notNull(),
        vertexId: text("vertex_id").notNull(),
        edgeType: text("edge_type"),
        edgeTargetId: text("edge_target_id"),
        action: text("action").notNull(), // "created" | "updated" | "deleted" | "property_changed"
        snapshot: jsonb("snapshot").$type<Record<string, unknown>>(),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("graph_event_vertexId_idx").on(table.vertexId),
        index("graph_event_createdAt_idx").on(table.createdAt),
        index("graph_event_vertexLabel_idx").on(table.vertexLabel)
    ]
);

export type GraphEvent = typeof graphEvent.$inferSelect;
export type NewGraphEvent = typeof graphEvent.$inferInsert;
```

- [ ] **Step 4: Export from schema index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./graph-event";
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/__tests__/schema/graph-event.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/graph-event.ts packages/db/src/schema/index.ts packages/db/src/__tests__/schema/graph-event.test.ts
git commit -m "feat(db): add graph_event schema"
```

---

### Task 3: Logged AGE Sync Layer

**Files:**
- Create: `packages/db/src/age/sync-logged.ts`
- Test: `packages/db/src/__tests__/age/sync-logged.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/db/src/__tests__/age/sync-logged.test.ts
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../src/age/sync", () => ({
    ensureVertex: vi.fn(() => ({ pipe: (fn: any) => fn({ _tag: "Success" }) })),
    deleteVertex: vi.fn(() => ({ pipe: (fn: any) => fn({ _tag: "Success" }) })),
    createEdge: vi.fn(() => ({ pipe: (fn: any) => fn({ _tag: "Success" }) })),
    deleteEdge: vi.fn(() => ({ pipe: (fn: any) => fn({ _tag: "Success" }) }))
}));

vi.mock("../../../src/repository/graph-event", () => ({
    insertGraphEvent: vi.fn(() => ({ pipe: (fn: any) => fn({ _tag: "Success" }) }))
}));

import { ensureVertexLogged, createEdgeLogged, deleteVertexLogged } from "../../age/sync-logged";

describe("sync-logged", () => {
    it("exports ensureVertexLogged", () => {
        expect(ensureVertexLogged).toBeDefined();
        expect(typeof ensureVertexLogged).toBe("function");
    });

    it("exports createEdgeLogged", () => {
        expect(createEdgeLogged).toBeDefined();
        expect(typeof createEdgeLogged).toBe("function");
    });

    it("exports deleteVertexLogged", () => {
        expect(deleteVertexLogged).toBeDefined();
        expect(typeof deleteVertexLogged).toBe("function");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/__tests__/age/sync-logged.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the graph-event repository** (dependency)

```typescript
// packages/db/src/repository/graph-event.ts
import { desc, eq, lte } from "drizzle-orm";

import { dbEffect } from "../index";
import { graphEvent } from "../schema/graph-event";
import { db } from "../index";

export function insertGraphEvent(data: {
    id: string;
    vertexLabel: string;
    vertexId: string;
    action: string;
    edgeType?: string;
    edgeTargetId?: string;
    snapshot?: Record<string, unknown>;
}) {
    return dbEffect(async () => {
        await db.insert(graphEvent).values(data);
    });
}

export function getGraphEventsUpTo(before: Date, limit = 10000) {
    return dbEffect(async () => {
        return db
            .select()
            .from(graphEvent)
            .where(lte(graphEvent.createdAt, before))
            .orderBy(graphEvent.createdAt)
            .limit(limit);
    });
}

export function getGraphEventsBetween(from: Date, to: Date) {
    return dbEffect(async () => {
        const { and, gte, lte: lteOp } = await import("drizzle-orm");
        return db
            .select()
            .from(graphEvent)
            .where(and(gte(graphEvent.createdAt, from), lteOp(graphEvent.createdAt, to)))
            .orderBy(graphEvent.createdAt);
    });
}
```

- [ ] **Step 4: Write the logged sync layer**

```typescript
// packages/db/src/age/sync-logged.ts
import { Effect } from "effect";

import { insertGraphEvent } from "../repository/graph-event";
import { createEdge, deleteEdge, deleteVertex, ensureVertex } from "./sync";

function logEvent(
    vertexLabel: string,
    vertexId: string,
    action: string,
    opts?: { edgeType?: string; edgeTargetId?: string; snapshot?: Record<string, unknown> }
) {
    return insertGraphEvent({
        id: crypto.randomUUID(),
        vertexLabel,
        vertexId,
        action,
        ...opts
    }).pipe(Effect.catchAll(() => Effect.succeed(undefined)));
}

export function ensureVertexLogged(label: string, id: string, snapshot?: Record<string, unknown>) {
    return ensureVertex(label, id).pipe(
        Effect.tap(() => logEvent(label, id, "created", { snapshot }))
    );
}

export function deleteVertexLogged(label: string, id: string) {
    return deleteVertex(label, id).pipe(
        Effect.tap(() => logEvent(label, id, "deleted"))
    );
}

export function createEdgeLogged(
    edgeLabel: string,
    fromLabel: string,
    fromId: string,
    toLabel: string,
    toId: string,
    props: Record<string, string> = {}
) {
    return createEdge(edgeLabel, fromLabel, fromId, toLabel, toId, props).pipe(
        Effect.tap(() =>
            logEvent(fromLabel, fromId, "created", {
                edgeType: edgeLabel,
                edgeTargetId: toId,
                snapshot: props
            })
        )
    );
}

export function deleteEdgeLogged(edgeLabel: string, props: Record<string, string>) {
    const sourceId = props.id ?? Object.values(props)[0] ?? "unknown";
    return deleteEdge(edgeLabel, props).pipe(
        Effect.tap(() =>
            logEvent("edge", sourceId, "deleted", { edgeType: edgeLabel, snapshot: props })
        )
    );
}
```

- [ ] **Step 5: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./graph-event";
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/__tests__/age/sync-logged.test.ts`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/age/sync-logged.ts packages/db/src/repository/graph-event.ts packages/db/src/__tests__/age/sync-logged.test.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add logged AGE sync layer and graph-event repository"
```

---

### Task 4: Push schema to database

- [ ] **Step 1: Push the new tables**

Run: `pnpm db:push`
Expected: Two new tables created: `usage_event`, `graph_event`

- [ ] **Step 2: Verify tables exist**

Run: `psql $DATABASE_URL -c "\dt usage_event; \dt graph_event;"`
Expected: Both tables listed

- [ ] **Step 3: Commit** (if db:push generates migration files)

---

## Phase 2: Usage Tracking

### Task 5: Usage Event Repository

**Files:**
- Create: `packages/db/src/repository/usage-event.ts`
- Modify: `packages/db/src/repository/index.ts`
- Test: `packages/db/src/__tests__/repository/usage-event.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/db/src/__tests__/repository/usage-event.test.ts
import { describe, expect, it } from "vitest";
import { insertUsageEvent, getRecentUsageEvents } from "../../repository/usage-event";

describe("usage-event repository", () => {
    it("exports insertUsageEvent", () => {
        expect(typeof insertUsageEvent).toBe("function");
    });

    it("exports getRecentUsageEvents", () => {
        expect(typeof getRecentUsageEvents).toBe("function");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/__tests__/repository/usage-event.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the repository**

```typescript
// packages/db/src/repository/usage-event.ts
import { and, gte, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { usageEvent } from "../schema/usage-event";

export function insertUsageEvent(data: {
    id: string;
    kind: string;
    chunkIds: string[];
    query?: string;
    userId: string;
}) {
    return dbEffect(async () => {
        await db.insert(usageEvent).values(data);
    });
}

export function getRecentUsageEvents(since: Date) {
    return dbEffect(async () => {
        return db
            .select()
            .from(usageEvent)
            .where(gte(usageEvent.createdAt, since))
            .orderBy(usageEvent.createdAt);
    });
}

export function getCoReferenceCounts(since: Date, minCount = 2) {
    return dbEffect(async () => {
        const rows = await db.execute(sql`
            WITH pairs AS (
                SELECT a.value::text AS chunk_a, b.value::text AS chunk_b
                FROM usage_event,
                     jsonb_array_elements_text(chunk_ids) a,
                     jsonb_array_elements_text(chunk_ids) b
                WHERE a.value::text < b.value::text
                  AND created_at >= ${since}
            )
            SELECT chunk_a, chunk_b, COUNT(*) AS co_count
            FROM pairs
            GROUP BY chunk_a, chunk_b
            HAVING COUNT(*) >= ${minCount}
            ORDER BY co_count DESC
        `);
        return rows.rows as Array<{ chunk_a: string; chunk_b: string; co_count: number }>;
    });
}
```

- [ ] **Step 4: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./usage-event";
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/__tests__/repository/usage-event.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/usage-event.ts packages/db/src/__tests__/repository/usage-event.test.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add usage-event repository with co-reference counting"
```

---

### Task 6: Usage Service & Routes

**Files:**
- Create: `packages/api/src/usage/service.ts`
- Create: `packages/api/src/usage/routes.ts`
- Modify: `packages/api/src/index.ts`
- Test: `packages/api/src/__tests__/usage.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/api/src/__tests__/usage.test.ts
import { describe, expect, it } from "vitest";
import { recordUsage, aggregateCoReferences } from "../usage/service";

describe("usage service", () => {
    it("exports recordUsage", () => {
        expect(typeof recordUsage).toBe("function");
    });

    it("exports aggregateCoReferences", () => {
        expect(typeof aggregateCoReferences).toBe("function");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/__tests__/usage.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the usage service**

```typescript
// packages/api/src/usage/service.ts
import {
    getCoReferenceCounts,
    insertUsageEvent,
    isAgeAvailable
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { logger } from "../logger";
import { createEdge, escCypher } from "@fubbik/db/age/sync";
import { cypherVoid } from "@fubbik/db/age/client";

export function recordUsage(
    kind: "context_query" | "chunk_view" | "mcp_resolve",
    chunkIds: string[],
    userId: string,
    query?: string
) {
    if (chunkIds.length === 0) return Effect.succeed(undefined);
    return insertUsageEvent({
        id: crypto.randomUUID(),
        kind,
        chunkIds,
        query,
        userId
    });
}

export function aggregateCoReferences(sinceDays = 1) {
    return Effect.gen(function* () {
        const since = new Date(Date.now() - sinceDays * 24 * 60 * 60 * 1000);
        const pairs = yield* getCoReferenceCounts(since, 2);

        if (pairs.length === 0) return { upserted: 0 };

        const ageReady = await isAgeAvailable();
        if (!ageReady) return { upserted: 0 };

        let upserted = 0;
        for (const { chunk_a, chunk_b, co_count } of pairs) {
            yield* cypherVoid(`
                MATCH (a:chunk {id: '${escCypher(chunk_a)}'}), (b:chunk {id: '${escCypher(chunk_b)}'})
                MERGE (a)-[r:co_referenced]->(b)
                SET r.count = ${co_count}, r.lastSeenAt = '${new Date().toISOString()}'
            `);
            upserted++;
        }

        logger.info("Co-reference aggregation complete", { upserted });
        return { upserted };
    });
}
```

- [ ] **Step 4: Write the usage routes**

```typescript
// packages/api/src/usage/routes.ts
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../auth-middleware";
import { aggregateCoReferences } from "./service";

export const usageRoutes = new Elysia({ prefix: "/usage" })
    .post("/aggregate", async ctx => {
        const result = await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => aggregateCoReferences())
            )
        );
        return result;
    })
    .get("/co-references", async ctx => {
        const chunkId = (ctx.query as { chunkId?: string }).chunkId;
        if (!chunkId) return { edges: [] };
        // Cypher query for co-referenced edges involving this chunk
        const { cypher } = await import("@fubbik/db/age/client");
        const rows = await Effect.runPromise(
            cypher(
                `MATCH (a:chunk {id: '${chunkId}'})-[r:co_referenced]-(b:chunk)
                 RETURN b.id AS target_id, r.count AS count`,
                "target_id agtype, count agtype"
            )
        );
        return {
            edges: rows.map(r => ({
                targetId: String(r.target_id).replace(/"/g, ""),
                count: Number(r.count)
            }))
        };
    });
```

- [ ] **Step 5: Mount routes in API index**

Add to `packages/api/src/index.ts` alongside other route imports:
```typescript
import { usageRoutes } from "./usage/routes";
```
And mount with `.use(usageRoutes)`.

- [ ] **Step 6: Run test to verify it passes**

Run: `cd packages/api && pnpm vitest run src/__tests__/usage.test.ts`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add packages/api/src/usage/service.ts packages/api/src/usage/routes.ts packages/api/src/__tests__/usage.test.ts packages/api/src/index.ts
git commit -m "feat(api): add usage tracking service and routes"
```

---

### Task 7: Wire Usage Recording into Existing Endpoints

**Files:**
- Modify: `packages/api/src/chunks/chunk-mutations.ts`
- Modify: `packages/api/src/context/` (the context resolver entry points)
- Modify: `packages/api/src/events/handlers.ts`

- [ ] **Step 1: Add CHUNK_VIEWED event type**

In `packages/api/src/events/bus.ts`, add to `EVENTS`:
```typescript
CHUNK_VIEWED: "chunk:viewed",
```

- [ ] **Step 2: Record usage on chunk detail view**

In the chunk detail route (the `GET /chunks/:id` handler), after returning the chunk, emit the event fire-and-forget:

```typescript
events.emit(EVENTS.CHUNK_VIEWED, { chunkId: id, userId });
```

- [ ] **Step 3: Register usage handler in events/handlers.ts**

Add to `packages/api/src/events/handlers.ts`:
```typescript
import { recordUsage } from "../usage/service";

events.on<{ chunkId: string; userId: string }>(EVENTS.CHUNK_VIEWED, async ({ chunkId, userId }) => {
    Effect.runPromise(recordUsage("chunk_view", [chunkId], userId)).catch(err => {
        logger.error("[event] Failed to record chunk view:", { err });
    });
});
```

- [ ] **Step 4: Record usage in context API**

In the context service functions that resolve chunks (e.g., `resolveForFiles`, `resolveForPlan`, `resolveForConcept`), after chunks are resolved, call:
```typescript
Effect.runPromise(recordUsage("context_query", chunkIds, userId, queryString)).catch(() => {});
```

- [ ] **Step 5: Register aggregation job in startup.ts**

In `packages/api/src/startup.ts`, add alongside the staleness scan:
```typescript
import { aggregateCoReferences } from "./usage/service";

async function runCoRefAggregation() {
    const start = Date.now();
    try {
        const result = await Effect.runPromise(aggregateCoReferences());
        logger.info("Co-reference aggregation completed", { upserted: result.upserted, durationMs: Date.now() - start });
    } catch (err) {
        logger.error("Co-reference aggregation failed", { error: err });
    }
}
```

Schedule it on the same interval as staleness scanning, offset by 5 minutes:
```typescript
setTimeout(() => { runCoRefAggregation(); }, 35000);
setInterval(() => { runCoRefAggregation(); }, intervalMs);
```

- [ ] **Step 6: Run full test suite**

Run: `pnpm test`
Expected: All existing tests pass

- [ ] **Step 7: Commit**

```bash
git add packages/api/src/events/bus.ts packages/api/src/events/handlers.ts packages/api/src/startup.ts packages/api/src/chunks/ packages/api/src/context/
git commit -m "feat(api): wire usage recording into chunk views and context queries"
```

---

## Phase 3: Impact Propagation

### Task 8: Degree-Scored Impact via AGE

**Files:**
- Create: `packages/db/src/age/impact.ts`
- Test: `packages/db/src/__tests__/age/impact.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/db/src/__tests__/age/impact.test.ts
import { describe, expect, it } from "vitest";
import { computeImpactRipple } from "../../age/impact";

describe("impact", () => {
    it("exports computeImpactRipple", () => {
        expect(typeof computeImpactRipple).toBe("function");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/__tests__/age/impact.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the impact module**

```typescript
// packages/db/src/age/impact.ts
import { Effect } from "effect";

import { cypher, escCypher } from "./client";

const DISTANCE_DECAY: Record<number, number> = { 1: 0.9, 2: 0.5, 3: 0.2 };

const RELATION_WEIGHT: Record<string, number> = {
    depends_on: 1.0,
    extends: 0.8,
    part_of: 0.7,
    references: 0.3,
    related_to: 0.2
};

export interface ImpactTarget {
    chunkId: string;
    degree: number;
    hops: number;
    path: string[];
}

export function computeImpactRipple(changedChunkId: string): Effect.Effect<ImpactTarget[], import("../errors").DatabaseError> {
    return Effect.gen(function* () {
        const rows = yield* cypher(
            `MATCH (source:chunk {id: '${escCypher(changedChunkId)}'})-[r:connects*1..3]->(downstream:chunk)
             WHERE downstream.id <> '${escCypher(changedChunkId)}'
             RETURN downstream.id AS did, length(r) AS hops, [rel IN r | rel.relation] AS path`,
            "did agtype, hops agtype, path agtype"
        );

        const bestByChunk = new Map<string, ImpactTarget>();

        for (const row of rows) {
            const chunkId = String(row.did).replace(/"/g, "");
            const hops = Number(row.hops);
            const pathRaw = String(row.path);
            const relations = pathRaw.replace(/[\[\]"]/g, "").split(",").map(s => s.trim()).filter(Boolean);

            const distanceFactor = DISTANCE_DECAY[hops] ?? 0.1;
            const relationFactor = relations.reduce(
                (min, rel) => Math.min(min, RELATION_WEIGHT[rel] ?? 0.2),
                1.0
            );
            const degree = distanceFactor * relationFactor;

            if (degree <= 0.1) continue;

            const existing = bestByChunk.get(chunkId);
            if (!existing || degree > existing.degree) {
                bestByChunk.set(chunkId, { chunkId, degree, hops, path: relations });
            }
        }

        return Array.from(bestByChunk.values());
    });
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/__tests__/age/impact.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/impact.ts packages/db/src/__tests__/age/impact.test.ts
git commit -m "feat(db): add degree-scored impact propagation via AGE"
```

---

### Task 9: Replace Flat Staleness with Degree-Scored Impact

**Files:**
- Modify: `packages/api/src/staleness/detect-impact.ts`
- Modify: `packages/api/src/events/handlers.ts`
- Test: existing staleness tests should still pass

- [ ] **Step 1: Add `upstream_impact` reason and degree-scored propagation**

Replace `flagDownstreamStale` and `flagUpstreamStale` in `packages/api/src/staleness/detect-impact.ts`:

```typescript
import { computeImpactRipple } from "@fubbik/db/age/impact";
import { createStaleFlag, getStaleFlags } from "@fubbik/db/repository";
import { Effect } from "effect";

export function flagImpactRipple(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.gen(function* () {
        const targets = yield* computeImpactRipple(updatedChunkId);
        if (targets.length === 0) return { flagged: 0 };

        const existingFlags = yield* getStaleFlags(userId, { reason: "upstream_impact" });
        const alreadyFlagged = new Map(
            existingFlags
                .filter(f => f.relatedChunkId === updatedChunkId)
                .map(f => [f.chunkId, f])
        );

        let flagged = 0;
        for (const target of targets) {
            if (alreadyFlagged.has(target.chunkId)) continue;
            yield* createStaleFlag({
                id: crypto.randomUUID(),
                chunkId: target.chunkId,
                reason: "upstream_impact",
                detail: `Impacted by change to "${updatedChunkTitle}" (degree: ${target.degree.toFixed(2)}, ${target.hops} hops via ${target.path.join(" → ")})`,
                relatedChunkId: updatedChunkId
            });
            flagged++;
        }

        return { flagged };
    });
}

// Keep the old functions for backward compatibility but delegate to the new one
export function flagBidirectionalImpact(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return flagImpactRipple(updatedChunkId, updatedChunkTitle, userId);
}
```

- [ ] **Step 2: Run existing tests**

Run: `pnpm test`
Expected: All pass — `flagBidirectionalImpact` signature unchanged

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/staleness/detect-impact.ts
git commit -m "feat(api): replace flat staleness with degree-scored impact propagation"
```

---

## Phase 4: Code Indexing

### Task 10: Code Index Service

**Files:**
- Create: `packages/api/src/code-index/service.ts`
- Create: `packages/api/src/code-index/routes.ts`
- Modify: `packages/api/src/index.ts`
- Test: `packages/api/src/__tests__/code-index.test.ts`

- [ ] **Step 1: Install tree-sitter dependency**

Run: `cd packages/api && pnpm add tree-sitter-wasms web-tree-sitter`

- [ ] **Step 2: Write the failing test**

```typescript
// packages/api/src/__tests__/code-index.test.ts
import { describe, expect, it } from "vitest";
import { indexFile, indexDirectory } from "../code-index/service";

describe("code-index service", () => {
    it("exports indexFile", () => {
        expect(typeof indexFile).toBe("function");
    });

    it("exports indexDirectory", () => {
        expect(typeof indexDirectory).toBe("function");
    });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/__tests__/code-index.test.ts`
Expected: FAIL

- [ ] **Step 4: Write the code-index service**

```typescript
// packages/api/src/code-index/service.ts
import { ensureVertex, createEdge, deleteEdgesFrom } from "@fubbik/db/age/sync";
import { cypherVoid, escCypher, isAgeAvailable } from "@fubbik/db/age/client";
import { Effect } from "effect";
import { readdir, readFile, stat } from "node:fs/promises";
import { join, extname, relative } from "node:path";

import { logger } from "../logger";

interface ExtractedSymbol {
    name: string;
    kind: "function" | "class" | "type" | "interface" | "variable";
    line: number;
    exported: boolean;
}

interface IndexedFile {
    path: string;
    language: string;
    symbols: ExtractedSymbol[];
    imports: string[];
}

const SUPPORTED_EXTENSIONS: Record<string, string> = {
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript"
};

function extractSymbolsRegex(content: string, language: string): ExtractedSymbol[] {
    const symbols: ExtractedSymbol[] = [];
    const lines = content.split("\n");

    const exportPatterns = [
        { regex: /^export\s+(?:async\s+)?function\s+(\w+)/,      kind: "function" as const },
        { regex: /^export\s+class\s+(\w+)/,                       kind: "class" as const },
        { regex: /^export\s+(?:type|interface)\s+(\w+)/,           kind: "type" as const },
        { regex: /^export\s+const\s+(\w+)/,                        kind: "variable" as const },
        { regex: /^export\s+default\s+(?:async\s+)?function\s+(\w+)/, kind: "function" as const },
        { regex: /^export\s+default\s+class\s+(\w+)/,              kind: "class" as const }
    ];

    const nonExportPatterns = [
        { regex: /^(?:async\s+)?function\s+(\w+)/,  kind: "function" as const },
        { regex: /^class\s+(\w+)/,                    kind: "class" as const },
        { regex: /^(?:type|interface)\s+(\w+)/,        kind: "type" as const },
        { regex: /^const\s+(\w+)/,                     kind: "variable" as const }
    ];

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();
        let matched = false;

        for (const { regex, kind } of exportPatterns) {
            const m = line.match(regex);
            if (m) {
                symbols.push({ name: m[1], kind, line: i + 1, exported: true });
                matched = true;
                break;
            }
        }

        if (!matched) {
            for (const { regex, kind } of nonExportPatterns) {
                const m = line.match(regex);
                if (m) {
                    symbols.push({ name: m[1], kind, line: i + 1, exported: false });
                    break;
                }
            }
        }
    }

    return symbols;
}

function extractImports(content: string): string[] {
    const imports: string[] = [];
    const importRegex = /(?:import|from)\s+['"]([^'"]+)['"]/g;
    let match;
    while ((match = importRegex.exec(content)) !== null) {
        const spec = match[1];
        if (spec.startsWith(".")) {
            imports.push(spec);
        }
    }
    return imports;
}

export function indexFile(filePath: string, basePath: string) {
    return Effect.tryPromise({
        try: async () => {
            const ext = extname(filePath);
            const language = SUPPORTED_EXTENSIONS[ext];
            if (!language) return null;

            const content = await readFile(filePath, "utf-8");
            const relPath = relative(basePath, filePath);
            const symbols = extractSymbolsRegex(content, language);
            const imports = extractImports(content);

            return { path: relPath, language, symbols, imports } satisfies IndexedFile;
        },
        catch: cause => new Error(`Failed to index ${filePath}: ${cause}`)
    });
}

export function indexDirectory(dirPath: string, basePath?: string) {
    const base = basePath ?? dirPath;
    return Effect.tryPromise({
        try: async () => {
            const files: IndexedFile[] = [];

            async function walk(dir: string) {
                const entries = await readdir(dir, { withFileTypes: true });
                for (const entry of entries) {
                    const full = join(dir, entry.name);
                    if (entry.name.startsWith(".") || entry.name === "node_modules" || entry.name === "dist") continue;
                    if (entry.isDirectory()) {
                        await walk(full);
                    } else {
                        const ext = extname(entry.name);
                        if (SUPPORTED_EXTENSIONS[ext]) {
                            const result = await Effect.runPromise(indexFile(full, base));
                            if (result) files.push(result);
                        }
                    }
                }
            }

            await walk(dirPath);
            return files;
        },
        catch: cause => new Error(`Failed to index directory ${dirPath}: ${cause}`)
    });
}

export function syncIndexToGraph(files: IndexedFile[]) {
    return Effect.gen(function* () {
        if (!(await isAgeAvailable())) return { synced: 0 };

        let synced = 0;
        for (const file of files) {
            yield* ensureVertex("code_file", file.path);
            yield* cypherVoid(
                `MATCH (f:code_file {id: '${escCypher(file.path)}'})
                 SET f.language = '${escCypher(file.language)}', f.lastIndexedAt = '${new Date().toISOString()}'`
            );

            yield* deleteEdgesFrom("defines", "code_file", file.path);

            for (const sym of file.symbols.filter(s => s.exported)) {
                const symId = `${file.path}::${sym.name}`;
                yield* ensureVertex("code_symbol", symId);
                yield* cypherVoid(
                    `MATCH (s:code_symbol {id: '${escCypher(symId)}'})
                     SET s.name = '${escCypher(sym.name)}', s.kind = '${sym.kind}',
                         s.filePath = '${escCypher(file.path)}', s.line = ${sym.line},
                         s.exported = true`
                );
                yield* createEdge("defines", "code_file", file.path, "code_symbol", symId);
            }

            for (const imp of file.imports) {
                const resolvedImport = imp.replace(/^\.\//, "").replace(/\.\w+$/, "");
                yield* cypherVoid(
                    `MATCH (a:code_file {id: '${escCypher(file.path)}'}), (b:code_file)
                     WHERE b.id ENDS WITH '${escCypher(resolvedImport)}'
                     MERGE (a)-[:imports]->(b)`
                );
            }

            synced++;
        }

        logger.info("Code index synced to graph", { files: synced });
        return { synced };
    });
}
```

- [ ] **Step 5: Write the code-index routes**

```typescript
// packages/api/src/code-index/routes.ts
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../auth-middleware";
import { indexDirectory, syncIndexToGraph } from "./service";

export const codeIndexRoutes = new Elysia({ prefix: "/code-index" })
    .post("/scan", async ctx => {
        const { path } = ctx.body as { path: string };
        const files = await Effect.runPromise(indexDirectory(path));
        const result = await Effect.runPromise(syncIndexToGraph(files));
        return { indexed: files.length, synced: result.synced };
    }, {
        body: t.Object({ path: t.String() })
    })
    .get("/status", async () => {
        // Return basic status — could be extended with last scan timestamp
        return { available: true };
    });
```

- [ ] **Step 6: Mount routes**

Add to `packages/api/src/index.ts`:
```typescript
import { codeIndexRoutes } from "./code-index/routes";
```
Mount with `.use(codeIndexRoutes)`.

- [ ] **Step 7: Run test to verify it passes**

Run: `cd packages/api && pnpm vitest run src/__tests__/code-index.test.ts`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add packages/api/src/code-index/ packages/api/src/__tests__/code-index.test.ts packages/api/src/index.ts
git commit -m "feat(api): add code indexing service with regex-based symbol extraction"
```

---

### Task 11: Wire Annotates Edges from Existing File Refs

**Files:**
- Modify: `packages/api/src/code-index/service.ts`

- [ ] **Step 1: Add function to link chunks to code nodes**

Add to `packages/api/src/code-index/service.ts`:

```typescript
import { listFileRefs } from "@fubbik/db/repository";

export function syncAnnotatesEdges(userId: string) {
    return Effect.gen(function* () {
        if (!(await isAgeAvailable())) return { linked: 0 };

        const fileRefs = yield* listFileRefs(userId);
        let linked = 0;

        for (const ref of fileRefs) {
            yield* cypherVoid(
                `MATCH (c:chunk {id: '${escCypher(ref.chunkId)}'}), (f:code_file)
                 WHERE f.id ENDS WITH '${escCypher(ref.filePath)}'
                 MERGE (c)-[:annotates {via: 'file_ref'}]->(f)`
            );
            linked++;
        }

        logger.info("Annotates edges synced", { linked });
        return { linked };
    });
}
```

- [ ] **Step 2: Run tests**

Run: `pnpm test`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/code-index/service.ts
git commit -m "feat(api): sync annotates edges from chunk file refs to code nodes"
```

---

## Phase 5: Timeline Scrubber

### Task 12: Graph Reconstruction Service

**Files:**
- Create: `packages/api/src/graph/timeline-service.ts`
- Test: `packages/api/src/__tests__/graph/timeline.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/api/src/__tests__/graph/timeline.test.ts
import { describe, expect, it } from "vitest";
import { reconstructGraphAt } from "../graph/timeline-service";

describe("timeline service", () => {
    it("exports reconstructGraphAt", () => {
        expect(typeof reconstructGraphAt).toBe("function");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/__tests__/graph/timeline.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the timeline service**

```typescript
// packages/api/src/graph/timeline-service.ts
import { getGraphEventsUpTo, getGraphEventsBetween } from "@fubbik/db/repository";
import { Effect } from "effect";

interface TimelineNode {
    id: string;
    label: string;
    properties: Record<string, unknown>;
}

interface TimelineEdge {
    sourceId: string;
    targetId: string;
    type: string;
    properties: Record<string, unknown>;
}

interface TimelineGraph {
    nodes: TimelineNode[];
    edges: TimelineEdge[];
    eventCount: number;
}

export function reconstructGraphAt(timestamp: Date): Effect.Effect<TimelineGraph, import("@fubbik/db/errors").DatabaseError> {
    return Effect.gen(function* () {
        const events = yield* getGraphEventsUpTo(timestamp);

        const nodes = new Map<string, TimelineNode>();
        const edges = new Map<string, TimelineEdge>();

        for (const event of events) {
            const nodeKey = `${event.vertexLabel}:${event.vertexId}`;

            if (!event.edgeType) {
                if (event.action === "created" || event.action === "updated" || event.action === "property_changed") {
                    const existing = nodes.get(nodeKey);
                    nodes.set(nodeKey, {
                        id: event.vertexId,
                        label: event.vertexLabel,
                        properties: { ...(existing?.properties ?? {}), ...(event.snapshot ?? {}) }
                    });
                } else if (event.action === "deleted") {
                    nodes.delete(nodeKey);
                }
            } else {
                const edgeKey = `${event.vertexId}-${event.edgeType}-${event.edgeTargetId}`;
                if (event.action === "created") {
                    edges.set(edgeKey, {
                        sourceId: event.vertexId,
                        targetId: event.edgeTargetId!,
                        type: event.edgeType,
                        properties: event.snapshot ?? {}
                    });
                } else if (event.action === "deleted") {
                    edges.delete(edgeKey);
                }
            }
        }

        return {
            nodes: Array.from(nodes.values()),
            edges: Array.from(edges.values()),
            eventCount: events.length
        };
    });
}

export function getGraphEventRange() {
    return Effect.gen(function* () {
        const events = yield* getGraphEventsUpTo(new Date(), 1);
        if (events.length === 0) return null;

        const earliest = yield* getGraphEventsUpTo(new Date(), 1);
        return {
            earliest: earliest[0]?.createdAt ?? null,
            latest: new Date()
        };
    });
}
```

- [ ] **Step 4: Add timeline routes**

Add to `packages/api/src/graph/routes.ts`:

```typescript
import { reconstructGraphAt } from "./timeline-service";

// Inside the graph route group:
.get("/at", async ctx => {
    const t = (ctx.query as { t?: string }).t;
    if (!t) return { error: "Missing t parameter" };
    const timestamp = new Date(t);
    if (isNaN(timestamp.getTime())) return { error: "Invalid timestamp" };
    const result = await Effect.runPromise(reconstructGraphAt(timestamp));
    return result;
})

.get("/events", async ctx => {
    const { from, to } = ctx.query as { from?: string; to?: string };
    const fromDate = from ? new Date(from) : new Date(Date.now() - 7 * 24 * 60 * 60 * 1000);
    const toDate = to ? new Date(to) : new Date();
    const events = await Effect.runPromise(getGraphEventsBetween(fromDate, toDate));
    return { events };
})
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/api && pnpm vitest run src/__tests__/graph/timeline.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/graph/timeline-service.ts packages/api/src/__tests__/graph/timeline.test.ts packages/api/src/graph/routes.ts
git commit -m "feat(api): add timeline reconstruction service and graph/at endpoint"
```

---

## Phase 6: Emergent Concepts

### Task 13: Concept Detection Service

**Files:**
- Create: `packages/api/src/concepts/service.ts`
- Create: `packages/api/src/concepts/routes.ts`
- Modify: `packages/api/src/index.ts`
- Test: `packages/api/src/__tests__/concepts.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/api/src/__tests__/concepts.test.ts
import { describe, expect, it } from "vitest";
import { detectEmergentConcepts, listConcepts } from "../concepts/service";

describe("concepts service", () => {
    it("exports detectEmergentConcepts", () => {
        expect(typeof detectEmergentConcepts).toBe("function");
    });

    it("exports listConcepts", () => {
        expect(typeof listConcepts).toBe("function");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/__tests__/concepts.test.ts`
Expected: FAIL

- [ ] **Step 3: Write the concept service**

```typescript
// packages/api/src/concepts/service.ts
import { getCoReferenceCounts, isAgeAvailable } from "@fubbik/db/repository";
import { cypher, cypherVoid, escCypher } from "@fubbik/db/age/client";
import { ensureVertex, createEdge } from "@fubbik/db/age/sync";
import { Effect } from "effect";
import { db } from "@fubbik/db";
import { sql } from "drizzle-orm";

import { logger } from "../logger";

interface ConceptCluster {
    chunkIds: string[];
    coRefCount: number;
}

function findClusters(pairs: Array<{ chunk_a: string; chunk_b: string; co_count: number }>, minSize = 3): ConceptCluster[] {
    const adjacency = new Map<string, Set<string>>();
    const pairCounts = new Map<string, number>();

    for (const { chunk_a, chunk_b, co_count } of pairs) {
        if (!adjacency.has(chunk_a)) adjacency.set(chunk_a, new Set());
        if (!adjacency.has(chunk_b)) adjacency.set(chunk_b, new Set());
        adjacency.get(chunk_a)!.add(chunk_b);
        adjacency.get(chunk_b)!.add(chunk_a);
        pairCounts.set(`${chunk_a}:${chunk_b}`, co_count);
    }

    const visited = new Set<string>();
    const clusters: ConceptCluster[] = [];

    for (const node of adjacency.keys()) {
        if (visited.has(node)) continue;
        const component: string[] = [];
        const queue = [node];
        let totalCount = 0;

        while (queue.length > 0) {
            const current = queue.shift()!;
            if (visited.has(current)) continue;
            visited.add(current);
            component.push(current);

            for (const neighbor of adjacency.get(current) ?? []) {
                if (!visited.has(neighbor)) {
                    queue.push(neighbor);
                    const key = [current, neighbor].sort().join(":");
                    totalCount += pairCounts.get(key) ?? 0;
                }
            }
        }

        if (component.length >= minSize) {
            clusters.push({ chunkIds: component, coRefCount: totalCount });
        }
    }

    return clusters;
}

async function deriveConceptLabel(chunkIds: string[]): Promise<string> {
    const placeholders = chunkIds.map((_, i) => `$${i + 1}`).join(", ");
    const result = await db.execute(sql`
        SELECT t.name, COUNT(*) as cnt
        FROM chunk_tag ct
        JOIN tag t ON ct.tag_id = t.id
        WHERE ct.chunk_id IN (${sql.raw(chunkIds.map(id => `'${id}'`).join(", "))})
        GROUP BY t.name
        ORDER BY cnt DESC
        LIMIT 1
    `);

    if (result.rows.length > 0) {
        return String(result.rows[0].name);
    }

    const titleResult = await db.execute(sql`
        SELECT title FROM chunk
        WHERE id IN (${sql.raw(chunkIds.map(id => `'${id}'`).join(", "))})
    `);

    const titles = titleResult.rows.map(r => String(r.title));
    const words = new Map<string, number>();
    for (const title of titles) {
        for (const word of title.toLowerCase().split(/\s+/).filter(w => w.length > 3)) {
            words.set(word, (words.get(word) ?? 0) + 1);
        }
    }

    let bestWord = "unnamed-concept";
    let bestCount = 0;
    for (const [word, count] of words) {
        if (count > bestCount) { bestWord = word; bestCount = count; }
    }

    return bestWord;
}

export function detectEmergentConcepts(sinceDays = 30, minCoRef = 5) {
    return Effect.gen(function* () {
        if (!(await isAgeAvailable())) return { created: 0 };

        const since = new Date(Date.now() - sinceDays * 24 * 60 * 60 * 1000);
        const pairs = yield* getCoReferenceCounts(since, minCoRef);

        if (pairs.length === 0) return { created: 0 };

        const clusters = findClusters(pairs, 3);
        let created = 0;

        for (const cluster of clusters) {
            const label = await deriveConceptLabel(cluster.chunkIds);
            const conceptId = `concept:${label}`;

            yield* ensureVertex("concept", conceptId);
            yield* cypherVoid(
                `MATCH (c:concept {id: '${escCypher(conceptId)}'})
                 SET c.label = '${escCypher(label)}',
                     c.strength = ${cluster.coRefCount},
                     c.firstSeenAt = COALESCE(c.firstSeenAt, '${new Date().toISOString()}')`
            );

            for (const chunkId of cluster.chunkIds) {
                yield* cypherVoid(
                    `MATCH (concept:concept {id: '${escCypher(conceptId)}'}), (chunk:chunk {id: '${escCypher(chunkId)}'})
                     MERGE (concept)-[r:embodies]->(chunk)
                     SET r.strength = ${cluster.coRefCount}`
                );
            }

            created++;
        }

        logger.info("Emergent concepts detected", { created, clusters: clusters.length });
        return { created };
    });
}

export function listConcepts() {
    return Effect.gen(function* () {
        const rows = yield* cypher(
            `MATCH (c:concept)-[r:embodies]->(chunk:chunk)
             RETURN c.id AS cid, c.label AS label, c.strength AS strength,
                    collect(chunk.id) AS member_ids`,
            "cid agtype, label agtype, strength agtype, member_ids agtype"
        );

        return rows.map(row => ({
            id: String(row.cid).replace(/"/g, ""),
            label: String(row.label).replace(/"/g, ""),
            strength: Number(row.strength),
            memberChunkIds: String(row.member_ids)
                .replace(/[\[\]"]/g, "")
                .split(",")
                .map(s => s.trim())
                .filter(Boolean)
        }));
    });
}
```

- [ ] **Step 4: Write the concept routes**

```typescript
// packages/api/src/concepts/routes.ts
import { Elysia } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../auth-middleware";
import { detectEmergentConcepts, listConcepts } from "./service";

export const conceptRoutes = new Elysia({ prefix: "/concepts" })
    .get("/", async ctx => {
        const concepts = await Effect.runPromise(listConcepts());
        return { concepts };
    })
    .post("/detect", async ctx => {
        const result = await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => detectEmergentConcepts())
            )
        );
        return result;
    });
```

- [ ] **Step 5: Mount routes**

Add to `packages/api/src/index.ts`:
```typescript
import { conceptRoutes } from "./concepts/routes";
```
Mount with `.use(conceptRoutes)`.

- [ ] **Step 6: Run test to verify it passes**

Run: `cd packages/api && pnpm vitest run src/__tests__/concepts.test.ts`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add packages/api/src/concepts/ packages/api/src/__tests__/concepts.test.ts packages/api/src/index.ts
git commit -m "feat(api): add emergent concept detection from co-reference clusters"
```

---

## Phase 7: Graph Visualization

### Task 14: Extend Graph Service to Include New Node Types

**Files:**
- Modify: `packages/api/src/graph/service.ts`

- [ ] **Step 1: Add code nodes and concepts to graph response**

In `getUserGraph()`, add after the existing parallel queries:

```typescript
const codeNodes = await Effect.runPromise(
    cypher(
        `MATCH (f:code_file) RETURN f.id AS id, f.language AS lang, f.lastIndexedAt AS indexed`,
        "id agtype, lang agtype, indexed agtype"
    ).pipe(Effect.catchAll(() => Effect.succeed([])))
);

const codeSymbols = await Effect.runPromise(
    cypher(
        `MATCH (s:code_symbol) RETURN s.id AS id, s.name AS name, s.kind AS kind, s.filePath AS fp, s.line AS line`,
        "id agtype, name agtype, kind agtype, fp agtype, line agtype"
    ).pipe(Effect.catchAll(() => Effect.succeed([])))
);

const concepts = await Effect.runPromise(
    cypher(
        `MATCH (c:concept)-[r:embodies]->(chunk:chunk)
         RETURN c.id AS cid, c.label AS label, c.strength AS str, collect(chunk.id) AS members`,
        "cid agtype, label agtype, str agtype, members agtype"
    ).pipe(Effect.catchAll(() => Effect.succeed([])))
);

const coRefEdges = await Effect.runPromise(
    cypher(
        `MATCH (a:chunk)-[r:co_referenced]->(b:chunk) RETURN a.id AS src, b.id AS tgt, r.count AS cnt`,
        "src agtype, tgt agtype, cnt agtype"
    ).pipe(Effect.catchAll(() => Effect.succeed([])))
);
```

Extend the return object with these new arrays, parsing AGE agtype values.

- [ ] **Step 2: Run existing graph tests**

Run: `pnpm test`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/graph/service.ts
git commit -m "feat(api): include code nodes, concepts, and co-ref edges in graph response"
```

---

### Task 15: Code Node and Concept Node React Flow Components

**Files:**
- Create: `apps/web/src/features/graph/code-node.tsx`
- Create: `apps/web/src/features/graph/concept-node.tsx`

- [ ] **Step 1: Write the code node component**

```tsx
// apps/web/src/features/graph/code-node.tsx
import { Handle, Position, type NodeProps } from "@xyflow/react";

interface CodeNodeData {
    name: string;
    kind: "file" | "function" | "class" | "type" | "interface" | "variable";
    language?: string;
    symbolCount?: number;
    collapsed?: boolean;
}

const KIND_ICONS: Record<string, string> = {
    file: "📄",
    function: "ƒ",
    class: "C",
    type: "T",
    interface: "I",
    variable: "x"
};

export function CodeNode({ data }: NodeProps) {
    const d = data as CodeNodeData;
    return (
        <div className="rounded border border-blue-300 bg-blue-50 px-2 py-1 text-xs dark:border-blue-700 dark:bg-blue-950">
            <Handle type="target" position={Position.Top} className="!bg-blue-400" />
            <div className="flex items-center gap-1">
                <span className="font-mono text-blue-600 dark:text-blue-400">{KIND_ICONS[d.kind] ?? "?"}</span>
                <span className="truncate font-medium">{d.name}</span>
                {d.symbolCount != null && (
                    <span className="ml-auto rounded bg-blue-200 px-1 text-[10px] dark:bg-blue-800">{d.symbolCount}</span>
                )}
            </div>
            <Handle type="source" position={Position.Bottom} className="!bg-blue-400" />
        </div>
    );
}
```

- [ ] **Step 2: Write the concept node component**

```tsx
// apps/web/src/features/graph/concept-node.tsx
import { Handle, Position, type NodeProps } from "@xyflow/react";

interface ConceptNodeData {
    label: string;
    strength: number;
    memberCount: number;
}

export function ConceptNode({ data }: NodeProps) {
    const d = data as ConceptNodeData;
    const opacity = Math.min(1, 0.3 + (d.strength / 20) * 0.7);
    return (
        <div
            className="rounded-full border-2 border-dashed border-violet-400 bg-violet-50 px-3 py-1.5 text-xs dark:border-violet-600 dark:bg-violet-950"
            style={{ opacity }}
        >
            <Handle type="target" position={Position.Top} className="!bg-violet-400" />
            <div className="flex items-center gap-1.5">
                <span className="font-semibold text-violet-700 dark:text-violet-300">{d.label}</span>
                <span className="text-[10px] text-violet-500">{d.memberCount}</span>
            </div>
            <Handle type="source" position={Position.Bottom} className="!bg-violet-400" />
        </div>
    );
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/code-node.tsx apps/web/src/features/graph/concept-node.tsx
git commit -m "feat(web): add CodeNode and ConceptNode React Flow components"
```

---

### Task 16: Timeline Scrubber Component

**Files:**
- Create: `apps/web/src/features/graph/timeline-scrubber.tsx`

- [ ] **Step 1: Write the timeline scrubber**

```tsx
// apps/web/src/features/graph/timeline-scrubber.tsx
import { useCallback, useState } from "react";

interface TimelineScrubberProps {
    earliest: Date;
    latest: Date;
    onTimeChange: (time: Date | null) => void;
    isPlaying: boolean;
    onPlayToggle: () => void;
}

export function TimelineScrubber({ earliest, latest, onTimeChange, isPlaying, onPlayToggle }: TimelineScrubberProps) {
    const range = latest.getTime() - earliest.getTime();
    const [value, setValue] = useState(1);

    const handleChange = useCallback(
        (e: React.ChangeEvent<HTMLInputElement>) => {
            const v = Number(e.target.value);
            setValue(v);
            if (v >= 1) {
                onTimeChange(null);
            } else {
                const time = new Date(earliest.getTime() + v * range);
                onTimeChange(time);
            }
        },
        [earliest, range, onTimeChange]
    );

    const displayDate = value >= 1
        ? "Now"
        : new Date(earliest.getTime() + value * range).toLocaleDateString();

    return (
        <div className="flex items-center gap-2 rounded-lg border bg-background/80 px-3 py-1.5 backdrop-blur">
            <button
                onClick={onPlayToggle}
                className="text-sm hover:text-primary"
                aria-label={isPlaying ? "Pause" : "Play"}
            >
                {isPlaying ? "⏸" : "▶"}
            </button>
            <input
                type="range"
                min={0}
                max={1}
                step={0.001}
                value={value}
                onChange={handleChange}
                className="w-48 accent-primary"
            />
            <span className="min-w-[5rem] text-xs text-muted-foreground">{displayDate}</span>
        </div>
    );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/graph/timeline-scrubber.tsx
git commit -m "feat(web): add timeline scrubber component"
```

---

### Task 17: Wire New Features into Graph View

**Files:**
- Modify: `apps/web/src/features/graph/use-graph-state.ts`
- Modify: `apps/web/src/features/graph/use-graph-nodes.ts`
- Modify: `apps/web/src/features/graph/graph-view.tsx`
- Modify: `apps/web/src/features/graph/typed-edge.tsx`

- [ ] **Step 1: Add state for code nodes toggle and timeline**

In `use-graph-state.ts`, add to the state type and reducer:

```typescript
// New state fields:
showCodeNodes: boolean;     // default false
timelineTime: Date | null;  // null = current
isTimelinePlaying: boolean; // default false

// New actions:
| { type: "TOGGLE_CODE_NODES" }
| { type: "SET_TIMELINE_TIME"; time: Date | null }
| { type: "TOGGLE_TIMELINE_PLAYING" }
```

- [ ] **Step 2: Register new node types in graph-view.tsx**

Add to the `nodeTypes` object passed to `<ReactFlow>`:

```typescript
import { CodeNode } from "./code-node";
import { ConceptNode } from "./concept-node";

const nodeTypes = {
    island: GraphIslandNode,
    chunkCard: GraphChunkCard,
    chunkDot: GraphChunkDot,
    codeNode: CodeNode,
    conceptNode: ConceptNode
};
```

- [ ] **Step 3: Add code toggle button and timeline scrubber to toolbar**

In the graph toolbar area of `graph-view.tsx`, add:

```tsx
import { TimelineScrubber } from "./timeline-scrubber";

// In the top bar controls:
<button
    onClick={() => dispatch({ type: "TOGGLE_CODE_NODES" })}
    className={cn("rounded px-2 py-1 text-xs", state.showCodeNodes && "bg-blue-100 dark:bg-blue-900")}
>
    Code
</button>

// Below the graph canvas:
{graphEventRange && (
    <TimelineScrubber
        earliest={graphEventRange.earliest}
        latest={graphEventRange.latest}
        onTimeChange={time => dispatch({ type: "SET_TIMELINE_TIME", time })}
        isPlaying={state.isTimelinePlaying}
        onPlayToggle={() => dispatch({ type: "TOGGLE_TIMELINE_PLAYING" })}
    />
)}
```

- [ ] **Step 4: Add co-referenced edge style to typed-edge.tsx**

In the `EDGE_STYLES` map in `typed-edge.tsx`, add:

```typescript
co_referenced: {
    color: "#8b5cf6",        // violet
    strokeWidth: 1,          // base — multiplied by weight
    strokeDasharray: "4 2",  // dashed to distinguish from manual edges
    markerEnd: undefined     // no arrow — bidirectional by nature
}
```

In the edge rendering, scale `strokeWidth` by the `count` data property:

```typescript
const weight = data?.count ? Math.min(6, 1 + Math.log2(data.count)) : style.strokeWidth;
```

- [ ] **Step 5: Add code/concept nodes to use-graph-nodes.ts**

When `state.showCodeNodes` is true and zoom level is neighborhood, include code nodes:

```typescript
if (showCodeNodes) {
    const codeFiles = data.codeFiles ?? [];
    codeFiles.forEach((codeFile, i) => {
        layoutNodes.push({
            id: `code:${codeFile.id}`,
            type: "codeNode",
            position: { x: 600 + i * 120, y: -200 },
            data: { name: codeFile.id.split("/").pop(), kind: "file", symbolCount: codeFile.symbolCount }
        });
    });
}

// Always include concepts (they're emergent — not toggleable)
const concepts = data.concepts ?? [];
concepts.forEach((concept, i) => {
    const angle = (2 * Math.PI * i) / Math.max(concepts.length, 1);
    const radius = 500;
    layoutNodes.push({
        id: `concept:${concept.id}`,
        type: "conceptNode",
        position: { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius },
        data: { label: concept.label, strength: concept.strength, memberCount: concept.memberChunkIds.length }
    });
});
```

- [ ] **Step 6: Run the dev server and test manually**

Run: `pnpm dev`
- Navigate to `/graph`
- Verify the Code toggle button appears
- Verify the timeline scrubber appears (if graph events exist)
- Verify concept nodes render for any existing concepts

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/features/graph/
git commit -m "feat(web): wire code nodes, concepts, timeline, and co-ref edges into graph view"
```

---

### Task 18: Impact Ripple Visualization

**Files:**
- Modify: `apps/web/src/features/graph/use-graph-nodes.ts`
- Modify: `apps/web/src/features/graph/graph-chunk-card.tsx`

- [ ] **Step 1: Add impact ring to chunk card**

In `graph-chunk-card.tsx`, accept an optional `impactDegree` in the data:

```tsx
interface ChunkCardData {
    // ... existing fields
    impactDegree?: number; // 0-1, from staleness flags
}

// In the component render:
{d.impactDegree != null && d.impactDegree > 0 && (
    <div
        className="pointer-events-none absolute inset-0 rounded-lg border-2 border-amber-400"
        style={{ opacity: d.impactDegree }}
    />
)}
```

- [ ] **Step 2: Pass impact data through use-graph-nodes**

In `use-graph-nodes.ts`, when building chunk card nodes, look up staleness flags with reason `upstream_impact` and parse the degree from the detail string:

```typescript
const impactDegree = staleFlagsMap?.get(chunk.id)
    ?.filter(f => f.reason === "upstream_impact")
    .reduce((max, f) => {
        const match = f.detail?.match(/degree: ([\d.]+)/);
        return match ? Math.max(max, parseFloat(match[1])) : max;
    }, 0) ?? 0;
```

Pass `impactDegree` in the node data.

- [ ] **Step 3: Run dev server and verify**

Run: `pnpm dev`
- Trigger a chunk update that has downstream connections
- Navigate to `/graph`, zoom into neighborhood view
- Verify amber ring appears on impacted chunks

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/graph-chunk-card.tsx apps/web/src/features/graph/use-graph-nodes.ts
git commit -m "feat(web): show impact ripple as amber ring on affected chunk cards"
```

---

### Task 19: Type-Check and Final Verification

- [ ] **Step 1: Type-check all packages**

Run: `pnpm run check-types`
Expected: No errors

- [ ] **Step 2: Run full test suite**

Run: `pnpm test`
Expected: All tests pass

- [ ] **Step 3: Run linting**

Run: `pnpm lint`
Expected: No errors (or only pre-existing ones)

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve type errors and lint issues from dynamic graph features"
```

---

## Summary

| Phase | Tasks | What it delivers |
|---|---|---|
| 1. Foundation | 1–4 | `usage_event` + `graph_event` tables, logged AGE sync |
| 2. Usage Tracking | 5–7 | Usage recording, co-reference aggregation, periodic job |
| 3. Impact Propagation | 8–9 | Degree-scored ripples replacing flat staleness |
| 4. Code Indexing | 10–11 | Symbol extraction, code_file/code_symbol vertices, annotates edges |
| 5. Timeline Scrubber | 12 | Graph reconstruction at any point in time |
| 6. Emergent Concepts | 13 | Concept vertices from co-reference clusters |
| 7. Graph Visualization | 14–19 | New node types, timeline UI, co-ref edges, impact rings |
