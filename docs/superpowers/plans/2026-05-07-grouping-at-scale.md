# Grouping at Scale Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make chunk grouping fast and usable on codebases with thousands of chunks — server-side aggregation, lazy loading, virtual scrolling, graph clustering, and compound grouping.

**Architecture:** Five layered improvements: (1) a new server-side grouped query endpoint that returns group headers + counts with per-group pagination, (2) lazy-expand UI that only fetches chunks on group open, (3) virtual scrolling within expanded groups, (4) graph server-side cluster aggregation, (5) compound two-level grouping. Each builds on the previous but is independently shippable.

**Tech Stack:** Drizzle ORM (SQL GROUP BY), Elysia routes, Effect, @tanstack/react-virtual, @tanstack/react-query, React Flow

---

## File Map

### New files
| File | Responsibility |
|------|---------------|
| `packages/db/src/repository/chunk-groups.ts` | DB queries for grouped counts and per-group chunk pages |
| `packages/api/src/chunks/group-routes.ts` | Elysia routes for `/api/chunks/grouped` and `/api/chunks/grouped/:group/chunks` |
| `packages/api/src/chunks/group-service.ts` | Service layer for grouped queries |
| `apps/web/src/features/chunks/lazy-group-list.tsx` | Lazy-expand group list with virtual scrolling |
| `apps/web/src/features/graph/cluster-strategy.ts` | Server-side cluster aggregation strategy |
| `packages/db/src/repository/chunk-groups.test.ts` | Repository tests |
| `packages/api/src/chunks/group-service.test.ts` | Service tests |

### Modified files
| File | Changes |
|------|---------|
| `packages/api/src/index.ts` | Register group routes |
| `apps/web/src/routes/chunks.index.tsx` | Use lazy group list when grouping is active |
| `apps/web/src/features/graph/graph-view.tsx` | Add cluster aggregation path |
| `apps/web/src/features/graph/group-strategies.ts` | Add cluster strategy |
| `apps/web/src/features/graph/graph-filter-form.tsx` | Add compound grouping UI |
| `apps/web/package.json` | Add `@tanstack/react-virtual` dependency |

---

## Task 1: Server-Side Grouped Counts (Repository)

**Files:**
- Create: `packages/db/src/repository/chunk-groups.ts`
- Test: `packages/db/src/repository/chunk-groups.test.ts`

This task adds the SQL queries that return `{ groupName, count }[]` for each grouping dimension. All existing filter conditions (codebase, type, search, tags, etc.) are reused.

- [ ] **Step 1: Write the failing test for type grouping**

```typescript
// packages/db/src/repository/chunk-groups.test.ts
import { describe, it, expect, vi } from "vitest";

vi.mock("../index", () => {
    const mockRows = [
        { groupName: "document", count: 42 },
        { groupName: "note", count: 15 },
    ];
    return {
        db: {
            select: () => ({ from: () => ({ where: () => ({ groupBy: () => ({ orderBy: () => mockRows }) }) }) }),
            execute: () => Promise.resolve(mockRows),
        },
        dbEffect: (fn: () => Promise<unknown>) => {
            const { Effect } = require("effect");
            return Effect.tryPromise({ try: fn, catch: (e: unknown) => ({ _tag: "DatabaseError" as const, cause: e }) });
        },
    };
});

import { Effect } from "effect";
import { getGroupedCounts } from "./chunk-groups";

describe("getGroupedCounts", () => {
    it("returns counts grouped by chunk type", async () => {
        const result = await Effect.runPromise(
            getGroupedCounts({ groupBy: "type", userId: "u1" })
        );
        expect(result).toEqual([
            { groupName: "document", count: 42 },
            { groupName: "note", count: 15 },
        ]);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && npx vitest run src/repository/chunk-groups.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement getGroupedCounts**

```typescript
// packages/db/src/repository/chunk-groups.ts
import { and, desc, eq, inArray, isNull, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { chunkTag, tag, tagType } from "../schema/tag";
import { workspaceCodebase } from "../schema/workspace";

export interface GroupedCountsParams {
    groupBy: "type" | "status" | "origin" | "freshness" | "tagtype";
    tagTypeId?: string;
    userId?: string;
    codebaseId?: string;
    workspaceId?: string;
    globalOnly?: boolean;
    type?: string;
    search?: string;
    tags?: string[];
    tagMode?: "any" | "all";
    origin?: string;
    reviewStatus?: string;
}

export interface GroupCount {
    groupName: string;
    count: number;
}

function buildBaseConditions(params: GroupedCountsParams) {
    const conditions = [];
    if (params.userId) conditions.push(eq(chunk.userId, params.userId));
    conditions.push(isNull(chunk.archivedAt));
    if (params.type) conditions.push(eq(chunk.type, params.type));
    if (params.origin) conditions.push(eq(chunk.origin, params.origin));
    if (params.reviewStatus) conditions.push(eq(chunk.reviewStatus, params.reviewStatus));
    if (params.workspaceId) {
        const inWorkspace = db
            .select({ codebaseId: workspaceCodebase.codebaseId })
            .from(workspaceCodebase)
            .where(eq(workspaceCodebase.workspaceId, params.workspaceId));
        const inCodebases = db
            .select({ chunkId: chunkCodebase.chunkId })
            .from(chunkCodebase)
            .where(sql`${chunkCodebase.codebaseId} IN (${inWorkspace})`);
        const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
        conditions.push(or(sql`${chunk.id} IN (${inCodebases})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!);
    } else if (params.codebaseId) {
        const inCodebase = db
            .select({ chunkId: chunkCodebase.chunkId })
            .from(chunkCodebase)
            .where(eq(chunkCodebase.codebaseId, params.codebaseId));
        const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
        conditions.push(or(sql`${chunk.id} IN (${inCodebase})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!);
    }
    if (params.globalOnly) {
        const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
        conditions.push(sql`${chunk.id} NOT IN (${inAnyCodebase})`);
    }
    if (params.tags && params.tags.length > 0) {
        if (params.tagMode === "all") {
            const tagSub = db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(inArray(tag.name, params.tags))
                .groupBy(chunkTag.chunkId)
                .having(sql`COUNT(DISTINCT ${tag.name}) = ${params.tags.length}`);
            conditions.push(sql`${chunk.id} IN (${tagSub})`);
        } else {
            const tagSub = db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(inArray(tag.name, params.tags));
            conditions.push(sql`${chunk.id} IN (${tagSub})`);
        }
    }
    return conditions;
}

export function getGroupedCounts(params: GroupedCountsParams): Effect.Effect<GroupCount[], { _tag: "DatabaseError"; cause: unknown }> {
    return dbEffect(async () => {
        const conditions = buildBaseConditions(params);
        const where = conditions.length > 0 ? and(...conditions) : undefined;

        if (params.groupBy === "type") {
            const rows = await db
                .select({ groupName: chunk.type, count: sql<number>`count(*)::int` })
                .from(chunk)
                .where(where)
                .groupBy(chunk.type)
                .orderBy(desc(sql`count(*)`));
            return rows;
        }

        if (params.groupBy === "status") {
            const rows = await db
                .select({
                    groupName: sql<string>`coalesce(${chunk.reviewStatus}, 'draft')`,
                    count: sql<number>`count(*)::int`
                })
                .from(chunk)
                .where(where)
                .groupBy(sql`coalesce(${chunk.reviewStatus}, 'draft')`)
                .orderBy(desc(sql`count(*)`));
            return rows;
        }

        if (params.groupBy === "origin") {
            const rows = await db
                .select({
                    groupName: sql<string>`coalesce(${chunk.origin}, 'human')`,
                    count: sql<number>`count(*)::int`
                })
                .from(chunk)
                .where(where)
                .groupBy(sql`coalesce(${chunk.origin}, 'human')`)
                .orderBy(desc(sql`count(*)`));
            return rows;
        }

        if (params.groupBy === "freshness") {
            const rows = await db
                .select({
                    groupName: sql<string>`
                        CASE
                            WHEN ${chunk.updatedAt} > now() - interval '7 days' THEN 'This week'
                            WHEN ${chunk.updatedAt} > now() - interval '30 days' THEN 'This month'
                            WHEN ${chunk.updatedAt} > now() - interval '90 days' THEN 'Last 3 months'
                            ELSE 'Older'
                        END`,
                    count: sql<number>`count(*)::int`
                })
                .from(chunk)
                .where(where)
                .groupBy(sql`
                    CASE
                        WHEN ${chunk.updatedAt} > now() - interval '7 days' THEN 'This week'
                        WHEN ${chunk.updatedAt} > now() - interval '30 days' THEN 'This month'
                        WHEN ${chunk.updatedAt} > now() - interval '90 days' THEN 'Last 3 months'
                        ELSE 'Older'
                    END`)
                .orderBy(desc(sql`count(*)`));
            return rows;
        }

        if (params.groupBy === "tagtype" && params.tagTypeId) {
            const rows = await db
                .select({
                    groupName: tag.name,
                    count: sql<number>`count(DISTINCT ${chunk.id})::int`
                })
                .from(chunk)
                .innerJoin(chunkTag, eq(chunk.id, chunkTag.chunkId))
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(and(where, eq(tag.tagTypeId, params.tagTypeId)))
                .groupBy(tag.name)
                .orderBy(desc(sql`count(DISTINCT ${chunk.id})`));
            return rows;
        }

        return [];
    });
}

export interface GroupChunksParams {
    groupBy: "type" | "status" | "origin" | "freshness" | "tagtype";
    groupName: string;
    tagTypeId?: string;
    userId?: string;
    codebaseId?: string;
    workspaceId?: string;
    globalOnly?: boolean;
    type?: string;
    search?: string;
    tags?: string[];
    tagMode?: "any" | "all";
    origin?: string;
    reviewStatus?: string;
    sort?: "newest" | "oldest" | "alpha" | "updated";
    limit: number;
    offset: number;
}

export function getChunksInGroup(params: GroupChunksParams) {
    return dbEffect(async () => {
        const conditions = buildBaseConditions(params);

        if (params.groupBy === "type") {
            conditions.push(eq(chunk.type, params.groupName));
        } else if (params.groupBy === "status") {
            conditions.push(sql`coalesce(${chunk.reviewStatus}, 'draft') = ${params.groupName}`);
        } else if (params.groupBy === "origin") {
            conditions.push(sql`coalesce(${chunk.origin}, 'human') = ${params.groupName}`);
        } else if (params.groupBy === "freshness") {
            const freshnessCase: Record<string, string> = {
                "This week": "7 days",
                "This month": "30 days",
                "Last 3 months": "90 days",
            };
            const interval = freshnessCase[params.groupName];
            if (interval) {
                conditions.push(sql.raw(`${chunk.updatedAt.name} > now() - interval '${interval}'`));
                if (params.groupName === "This month") {
                    conditions.push(sql.raw(`${chunk.updatedAt.name} <= now() - interval '7 days'`));
                } else if (params.groupName === "Last 3 months") {
                    conditions.push(sql.raw(`${chunk.updatedAt.name} <= now() - interval '30 days'`));
                }
            } else {
                conditions.push(sql`${chunk.updatedAt} <= now() - interval '90 days'`);
            }
        } else if (params.groupBy === "tagtype" && params.tagTypeId) {
            const tagSub = db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(and(eq(tag.tagTypeId, params.tagTypeId), eq(tag.name, params.groupName)));
            conditions.push(sql`${chunk.id} IN (${tagSub})`);
        }

        const orderClause = (() => {
            switch (params.sort) {
                case "oldest": return sql`${chunk.createdAt} ASC`;
                case "alpha": return sql`${chunk.title} ASC`;
                case "updated": return sql`${chunk.updatedAt} DESC`;
                default: return sql`${chunk.createdAt} DESC`;
            }
        })();

        const where = conditions.length > 0 ? and(...conditions) : undefined;

        const [chunks, totalResult] = await Promise.all([
            db.select().from(chunk).where(where).orderBy(orderClause).limit(params.limit).offset(params.offset),
            db.select({ count: sql<number>`count(*)::int` }).from(chunk).where(where),
        ]);

        return { chunks, total: Number(totalResult[0]?.count ?? 0) };
    });
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && npx vitest run src/repository/chunk-groups.test.ts`
Expected: PASS

- [ ] **Step 5: Export from repository barrel**

Add to `packages/db/src/repository/index.ts`:

```typescript
export { getGroupedCounts, getChunksInGroup } from "./chunk-groups";
```

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/chunk-groups.ts packages/db/src/repository/chunk-groups.test.ts packages/db/src/repository/index.ts
git commit -m "feat: add server-side grouped count and per-group chunk queries"
```

---

## Task 2: Server-Side Grouped API Routes

**Files:**
- Create: `packages/api/src/chunks/group-service.ts`
- Create: `packages/api/src/chunks/group-routes.ts`
- Create: `packages/api/src/chunks/group-service.test.ts`
- Modify: `packages/api/src/index.ts` (register routes)

- [ ] **Step 1: Write the failing test for group service**

```typescript
// packages/api/src/chunks/group-service.test.ts
import { describe, it, expect, vi } from "vitest";

vi.mock("@fubbik/db/repository", () => ({
    getGroupedCounts: vi.fn(() => {
        const { Effect } = require("effect");
        return Effect.succeed([
            { groupName: "document", count: 42 },
            { groupName: "note", count: 15 },
        ]);
    }),
    getChunksInGroup: vi.fn(() => {
        const { Effect } = require("effect");
        return Effect.succeed({
            chunks: [{ id: "c1", title: "Test", type: "document" }],
            total: 1,
        });
    }),
}));

import { Effect } from "effect";
import { listGroupedCounts, listGroupChunks } from "./group-service";

describe("listGroupedCounts", () => {
    it("parses query params and returns group counts", async () => {
        const result = await Effect.runPromise(
            listGroupedCounts("user1", { groupBy: "type" })
        );
        expect(result).toEqual({
            groups: [
                { groupName: "document", count: 42 },
                { groupName: "note", count: 15 },
            ],
            totalGroups: 2,
        });
    });
});

describe("listGroupChunks", () => {
    it("returns chunks within a group", async () => {
        const result = await Effect.runPromise(
            listGroupChunks("user1", "document", { groupBy: "type" })
        );
        expect(result.chunks).toHaveLength(1);
        expect(result.total).toBe(1);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && npx vitest run src/chunks/group-service.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement group-service.ts**

```typescript
// packages/api/src/chunks/group-service.ts
import { getGroupedCounts, getChunksInGroup } from "@fubbik/db/repository";
import { Effect } from "effect";

export function listGroupedCounts(
    userId: string,
    query: {
        groupBy: string;
        tagTypeId?: string;
        codebaseId?: string;
        workspaceId?: string;
        global?: string;
        type?: string;
        search?: string;
        tags?: string;
        tagMode?: "any" | "all";
        origin?: string;
        reviewStatus?: string;
    }
) {
    const parsedTags = query.tags?.split(",").map(s => s.trim()).filter(Boolean);
    const groupBy = parseGroupBy(query.groupBy);
    const tagTypeId = groupBy === "tagtype" ? query.tagTypeId : undefined;

    return getGroupedCounts({
        groupBy,
        tagTypeId,
        userId,
        codebaseId: query.global === "true" ? undefined : query.codebaseId,
        workspaceId: query.global === "true" ? undefined : query.workspaceId,
        globalOnly: query.global === "true",
        type: query.type,
        search: query.search,
        tags: parsedTags?.length ? parsedTags : undefined,
        tagMode: query.tagMode,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
    }).pipe(
        Effect.map(groups => ({ groups, totalGroups: groups.length }))
    );
}

export function listGroupChunks(
    userId: string,
    groupName: string,
    query: {
        groupBy: string;
        tagTypeId?: string;
        codebaseId?: string;
        workspaceId?: string;
        global?: string;
        type?: string;
        search?: string;
        tags?: string;
        tagMode?: "any" | "all";
        origin?: string;
        reviewStatus?: string;
        sort?: "newest" | "oldest" | "alpha" | "updated";
        limit?: string;
        offset?: string;
    }
) {
    const limit = Math.min(Number(query.limit ?? 20), 100);
    const offset = Number(query.offset ?? 0);
    const parsedTags = query.tags?.split(",").map(s => s.trim()).filter(Boolean);
    const groupBy = parseGroupBy(query.groupBy);
    const tagTypeId = groupBy === "tagtype" ? query.tagTypeId : undefined;

    return getChunksInGroup({
        groupBy,
        groupName,
        tagTypeId,
        userId,
        codebaseId: query.global === "true" ? undefined : query.codebaseId,
        workspaceId: query.global === "true" ? undefined : query.workspaceId,
        globalOnly: query.global === "true",
        type: query.type,
        search: query.search,
        tags: parsedTags?.length ? parsedTags : undefined,
        tagMode: query.tagMode,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        sort: query.sort,
        limit,
        offset,
    }).pipe(
        Effect.map(result => ({ ...result, limit, offset }))
    );
}

function parseGroupBy(raw: string): "type" | "status" | "origin" | "freshness" | "tagtype" {
    if (raw.startsWith("tagtype")) return "tagtype";
    if (raw === "status" || raw === "origin" || raw === "freshness") return raw;
    return "type";
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/api && npx vitest run src/chunks/group-service.test.ts`
Expected: PASS

- [ ] **Step 5: Implement group-routes.ts**

```typescript
// packages/api/src/chunks/group-routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as groupService from "./group-service";

const groupBySchema = t.Union([
    t.Literal("type"),
    t.Literal("status"),
    t.Literal("origin"),
    t.Literal("freshness"),
    t.Literal("tagtype"),
]);

const sharedQuery = {
    groupBy: groupBySchema,
    tagTypeId: t.Optional(t.String()),
    codebaseId: t.Optional(t.String()),
    workspaceId: t.Optional(t.String()),
    global: t.Optional(t.String()),
    type: t.Optional(t.String()),
    search: t.Optional(t.String()),
    tags: t.Optional(t.String()),
    tagMode: t.Optional(t.Union([t.Literal("any"), t.Literal("all")])),
    origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
    reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")])),
};

export const chunkGroupRoutes = new Elysia()
    .get(
        "/chunks/grouped",
        ctx => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => groupService.listGroupedCounts(session.user.id, ctx.query))
            )
        ),
        { query: t.Object(sharedQuery) }
    )
    .get(
        "/chunks/grouped/:groupName/chunks",
        ctx => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    groupService.listGroupChunks(session.user.id, ctx.params.groupName, ctx.query)
                )
            )
        ),
        {
            params: t.Object({ groupName: t.String() }),
            query: t.Object({
                ...sharedQuery,
                sort: t.Optional(t.Union([t.Literal("newest"), t.Literal("oldest"), t.Literal("alpha"), t.Literal("updated")])),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
            }),
        }
    );
```

- [ ] **Step 6: Register routes in packages/api/src/index.ts**

Find the section where routes are registered (look for `.use(chunkRoutes)`) and add:

```typescript
import { chunkGroupRoutes } from "./chunks/group-routes";
// ... in the .use() chain:
.use(chunkGroupRoutes)
```

The group routes must be registered **before** `chunkRoutes` because `/chunks/grouped` needs to match before `/chunks/:id`.

- [ ] **Step 7: Run tests**

Run: `pnpm test --filter=@fubbik/api`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add packages/api/src/chunks/group-service.ts packages/api/src/chunks/group-routes.ts packages/api/src/chunks/group-service.test.ts packages/api/src/index.ts
git commit -m "feat: add /api/chunks/grouped endpoint for server-side group counts"
```

---

## Task 3: Install @tanstack/react-virtual

**Files:**
- Modify: `apps/web/package.json`

- [ ] **Step 1: Install the dependency**

```bash
cd apps/web && pnpm add @tanstack/react-virtual
```

- [ ] **Step 2: Verify installation**

Run: `pnpm run check-types --filter=web`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add apps/web/package.json pnpm-lock.yaml
git commit -m "chore: add @tanstack/react-virtual for virtualized lists"
```

---

## Task 4: Lazy-Expand Group List Component

**Files:**
- Create: `apps/web/src/features/chunks/lazy-group-list.tsx`

This component:
1. Fetches group headers + counts from `/api/chunks/grouped` (single query)
2. Shows collapsed group headers with counts
3. On expand, fetches that group's chunks from `/api/chunks/grouped/:groupName/chunks`
4. Uses `@tanstack/react-virtual` for virtualized rendering within each group

- [ ] **Step 1: Create the lazy group list component**

```tsx
// apps/web/src/features/chunks/lazy-group-list.tsx
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronRight, Loader2 } from "lucide-react";
import { useCallback, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ChunkRow, type ChunkRowProps } from "@/features/chunks/chunk-row";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface LazyGroupListProps {
    groupBy: string;
    tagTypeId?: string;
    codebaseId?: string;
    sort?: string;
    filters: Record<string, string | undefined>;
    chunkRowProps: Omit<ChunkRowProps, "chunk" | "index" | "allChunkIds" | "isSelected" | "isPinned" | "showSeparator">;
    selectedIds: Set<string>;
    pinnedIds: string[];
    isPinned: (id: string) => boolean;
    onSelectionClick: (id: string, e: React.MouseEvent) => void;
}

const ROW_HEIGHT = 64;

export function LazyGroupList({
    groupBy,
    tagTypeId,
    codebaseId,
    sort,
    filters,
    chunkRowProps,
    selectedIds,
    pinnedIds,
    isPinned,
    onSelectionClick,
}: LazyGroupListProps) {
    const [expanded, setExpanded] = useState<Set<string>>(new Set());

    const groupsQuery = useQuery({
        queryKey: ["chunk-groups", groupBy, tagTypeId, codebaseId, filters],
        queryFn: async () => {
            const query: Record<string, string> = { groupBy };
            if (tagTypeId) query.tagTypeId = tagTypeId;
            if (codebaseId && codebaseId !== "global") query.codebaseId = codebaseId;
            if (codebaseId === "global") query.global = "true";
            for (const [k, v] of Object.entries(filters)) {
                if (v) query[k] = v;
            }
            return unwrapEden(await api.api.chunks.grouped.get({ query: query as never }));
        },
        staleTime: 30_000,
    });

    const toggleGroup = (name: string) => {
        setExpanded(prev => {
            const next = new Set(prev);
            if (next.has(name)) next.delete(name);
            else next.add(name);
            return next;
        });
    };

    const groups = groupsQuery.data?.groups ?? [];

    return (
        <div className="space-y-3">
            {groupsQuery.isLoading && (
                <div className="flex items-center justify-center py-8">
                    <Loader2 className="text-muted-foreground size-5 animate-spin" />
                </div>
            )}
            {groups.map(g => (
                <div key={g.groupName}>
                    <button
                        onClick={() => toggleGroup(g.groupName)}
                        className="mb-2 flex items-center gap-2"
                    >
                        <ChevronRight
                            className={`size-3 transition-transform ${expanded.has(g.groupName) && "rotate-90"}`}
                        />
                        <Badge variant="secondary">{g.groupName}</Badge>
                        <span className="text-muted-foreground text-xs tabular-nums">({g.count})</span>
                    </button>
                    {expanded.has(g.groupName) && (
                        <VirtualGroupChunks
                            groupName={g.groupName}
                            groupBy={groupBy}
                            tagTypeId={tagTypeId}
                            codebaseId={codebaseId}
                            sort={sort}
                            filters={filters}
                            totalCount={g.count}
                            chunkRowProps={chunkRowProps}
                            selectedIds={selectedIds}
                            isPinned={isPinned}
                            onSelectionClick={onSelectionClick}
                        />
                    )}
                </div>
            ))}
        </div>
    );
}

interface VirtualGroupChunksProps {
    groupName: string;
    groupBy: string;
    tagTypeId?: string;
    codebaseId?: string;
    sort?: string;
    filters: Record<string, string | undefined>;
    totalCount: number;
    chunkRowProps: Omit<ChunkRowProps, "chunk" | "index" | "allChunkIds" | "isSelected" | "isPinned" | "showSeparator">;
    selectedIds: Set<string>;
    isPinned: (id: string) => boolean;
    onSelectionClick: (id: string, e: React.MouseEvent) => void;
}

function VirtualGroupChunks({
    groupName,
    groupBy,
    tagTypeId,
    codebaseId,
    sort,
    filters,
    totalCount,
    chunkRowProps,
    selectedIds,
    isPinned,
    onSelectionClick,
}: VirtualGroupChunksProps) {
    const parentRef = useRef<HTMLDivElement>(null);

    const chunksQuery = useInfiniteQuery({
        queryKey: ["chunk-group-items", groupName, groupBy, tagTypeId, codebaseId, sort, filters],
        queryFn: async ({ pageParam = 0 }) => {
            const query: Record<string, string> = {
                groupBy,
                offset: String(pageParam),
                limit: "50",
            };
            if (tagTypeId) query.tagTypeId = tagTypeId;
            if (codebaseId && codebaseId !== "global") query.codebaseId = codebaseId;
            if (codebaseId === "global") query.global = "true";
            if (sort) query.sort = sort;
            for (const [k, v] of Object.entries(filters)) {
                if (v) query[k] = v;
            }
            return unwrapEden(
                await api.api.chunks.grouped({ groupName }).chunks.get({ query: query as never })
            );
        },
        initialPageParam: 0,
        getNextPageParam: (lastPage, allPages) => {
            const loaded = allPages.reduce((sum, p) => sum + (p?.chunks?.length ?? 0), 0);
            return loaded < (lastPage?.total ?? 0) ? loaded : undefined;
        },
    });

    const allChunks = chunksQuery.data?.pages.flatMap(p => p?.chunks ?? []) ?? [];
    const allChunkIds = allChunks.map(c => c.id);
    const hasMore = chunksQuery.hasNextPage;

    const maxHeight = Math.min(totalCount * ROW_HEIGHT, 600);

    const virtualizer = useVirtualizer({
        count: allChunks.length,
        getScrollElement: () => parentRef.current,
        estimateSize: () => ROW_HEIGHT,
        overscan: 10,
    });

    const fetchMore = useCallback(() => {
        if (hasMore && !chunksQuery.isFetchingNextPage) {
            chunksQuery.fetchNextPage();
        }
    }, [hasMore, chunksQuery]);

    const items = virtualizer.getVirtualItems();
    const lastItem = items[items.length - 1];
    if (lastItem && lastItem.index >= allChunks.length - 5) {
        fetchMore();
    }

    if (chunksQuery.isLoading) {
        return (
            <div className="flex items-center justify-center py-4">
                <Loader2 className="text-muted-foreground size-4 animate-spin" />
            </div>
        );
    }

    return (
        <Card>
            <div
                ref={parentRef}
                style={{ maxHeight, overflow: "auto" }}
            >
                <div
                    style={{
                        height: virtualizer.getTotalSize(),
                        width: "100%",
                        position: "relative",
                    }}
                >
                    {items.map(virtualRow => {
                        const chunkItem = allChunks[virtualRow.index];
                        if (!chunkItem) return null;
                        return (
                            <div
                                key={virtualRow.key}
                                style={{
                                    position: "absolute",
                                    top: 0,
                                    left: 0,
                                    width: "100%",
                                    height: `${virtualRow.size}px`,
                                    transform: `translateY(${virtualRow.start}px)`,
                                }}
                            >
                                <ChunkRow
                                    {...chunkRowProps}
                                    chunk={chunkItem}
                                    index={virtualRow.index}
                                    allChunkIds={allChunkIds}
                                    isSelected={selectedIds.has(chunkItem.id)}
                                    isPinned={isPinned(chunkItem.id)}
                                    showSeparator={virtualRow.index > 0}
                                />
                            </div>
                        );
                    })}
                </div>
            </div>
            {hasMore && (
                <div className="border-t p-2 text-center">
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => chunksQuery.fetchNextPage()}
                        disabled={chunksQuery.isFetchingNextPage}
                    >
                        {chunksQuery.isFetchingNextPage ? "Loading..." : "Load more"}
                    </Button>
                </div>
            )}
        </Card>
    );
}
```

- [ ] **Step 2: Run type check**

Run: `pnpm run check-types --filter=web`
Expected: PASS (may need minor type adjustments to `ChunkRowProps` — make the props type exported if not already)

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/lazy-group-list.tsx
git commit -m "feat: add LazyGroupList component with virtual scrolling"
```

---

## Task 5: Wire Lazy Group List into Chunks Page

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx`

Replace the client-side grouping path with the server-side lazy group list when a group is active.

- [ ] **Step 1: Import LazyGroupList**

Add to the top of `chunks.index.tsx`:

```typescript
import { LazyGroupList } from "@/features/chunks/lazy-group-list";
```

- [ ] **Step 2: Replace grouped rendering**

Find the block that renders `groupedChunks` (around line 661):

```tsx
) : groupedChunks ? (
    <div className="space-y-4">
        {[...groupedChunks.entries()].map(([groupName, items]) => (
            // ...existing group rendering...
        ))}
    </div>
```

Replace with:

```tsx
) : group ? (
    <LazyGroupList
        groupBy={isTagTypeGroup ? "tagtype" : group}
        tagTypeId={selectedTagTypeId ?? undefined}
        codebaseId={codebaseId}
        sort={sort}
        filters={{ type, tags, origin, reviewStatus }}
        chunkRowProps={{
            editingChunkId,
            editTitle,
            onStartEditing: startEditing,
            onCommitEdit: commitEdit,
            onCancelEdit: cancelEdit,
            onEditTitleChange: setEditTitle,
            onHover: handleChunkHover,
            onTogglePin: togglePin,
            onSelectionClick: handleSelectionClick,
            onDelete: (id, title) =>
                setConfirmAction({
                    title: "Delete chunk",
                    description: `Delete "${title}" permanently?`,
                    action: () => singleDeleteMutation.mutate(id),
                }),
        }}
        selectedIds={selectedIds}
        pinnedIds={pinnedIds}
        isPinned={isPinned}
        onSelectionClick={handleSelectionClick}
    />
```

- [ ] **Step 3: Remove client-side grouping code that is no longer needed**

The `groupedChunks` useMemo, the `chunkTagsQuery`, and the `collapsed`/`toggleCollapsed` state can be removed since the `LazyGroupList` handles all of that internally. The `isTagTypeGroup` and `selectedTagTypeId` derived values are still needed for the dropdown and the `LazyGroupList` props.

- [ ] **Step 4: Run type check and tests**

Run: `pnpm run check-types --filter=web && pnpm test --filter=web`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/chunks.index.tsx
git commit -m "feat: wire lazy server-side grouping into chunks list page"
```

---

## Task 6: Graph Server-Side Cluster Aggregation

**Files:**
- Create: `apps/web/src/features/graph/cluster-strategy.ts`
- Modify: `apps/web/src/features/graph/group-strategies.ts`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

For large graphs (500+ nodes), instead of rendering all individual nodes inside a group bounding box, collapse each group into a single "cluster node" showing the group name + count. Clicking a cluster node expands it to show the individual nodes.

- [ ] **Step 1: Add cluster node types**

```typescript
// apps/web/src/features/graph/cluster-strategy.ts
import type { GroupStrategyResult } from "./group-strategies";

export interface ClusterNode {
    id: string;
    groupName: string;
    count: number;
    color?: string;
    expanded: boolean;
}

export function buildClusterNodes(
    groupResult: GroupStrategyResult,
    expandedGroups: Set<string>,
    threshold: number = 500
): { clusters: ClusterNode[]; shouldCluster: boolean } {
    const totalNodes = [...groupResult.groups.values()].reduce((sum, ids) => sum + ids.length, 0);
    const shouldCluster = totalNodes >= threshold;

    if (!shouldCluster) {
        return { clusters: [], shouldCluster: false };
    }

    const clusters: ClusterNode[] = [];
    for (const [groupName, chunkIds] of groupResult.groups) {
        clusters.push({
            id: `cluster-${groupName}`,
            groupName,
            count: chunkIds.length,
            color: groupResult.colorFor(groupName),
            expanded: expandedGroups.has(groupName),
        });
    }

    return { clusters, shouldCluster: true };
}

export function getVisibleChunkIds(
    groupResult: GroupStrategyResult,
    expandedGroups: Set<string>,
    shouldCluster: boolean
): Set<string> {
    if (!shouldCluster) {
        const all = new Set<string>();
        for (const ids of groupResult.groups.values()) {
            for (const id of ids) all.add(id);
        }
        return all;
    }

    const visible = new Set<string>();
    for (const [groupName, chunkIds] of groupResult.groups) {
        if (expandedGroups.has(groupName)) {
            for (const id of chunkIds) visible.add(id);
        }
    }
    return visible;
}
```

- [ ] **Step 2: Add expanded cluster state to graph state**

In `apps/web/src/features/graph/use-graph-state.ts`, add to the `GraphState` interface:

```typescript
expandedClusters: Set<string>;
```

Add the action type:

```typescript
| { type: "TOGGLE_CLUSTER"; groupName: string }
```

Add the initial state:

```typescript
expandedClusters: new Set(),
```

Add the reducer case:

```typescript
case "TOGGLE_CLUSTER":
    return { ...state, expandedClusters: toggleSetItem(state.expandedClusters, action.groupName) };
```

- [ ] **Step 3: Integrate cluster logic into graph-view.tsx**

After the `groupResult` useMemo (around line 331), add:

```typescript
import { buildClusterNodes, getVisibleChunkIds } from "./cluster-strategy";

// After groupResult:
const { clusters, shouldCluster } = useMemo(() => {
    if (!groupResult) return { clusters: [], shouldCluster: false };
    return buildClusterNodes(groupResult, expandedClusters);
}, [groupResult, expandedClusters]);

const visibleChunkIds = useMemo(() => {
    if (!groupResult) return null;
    return getVisibleChunkIds(groupResult, expandedClusters, shouldCluster);
}, [groupResult, expandedClusters, shouldCluster]);
```

Then, in the node-building pipeline, filter out chunks not in `visibleChunkIds` when clustering is active, and add cluster nodes to the React Flow nodes array. Each cluster node should use a custom node type that renders as a larger pill with the group name and count, and dispatches `TOGGLE_CLUSTER` on click.

- [ ] **Step 4: Run type check**

Run: `pnpm run check-types --filter=web`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/graph/cluster-strategy.ts apps/web/src/features/graph/use-graph-state.ts apps/web/src/features/graph/graph-view.tsx
git commit -m "feat: add graph cluster aggregation for large graphs"
```

---

## Task 7: Compound Grouping (Two-Level)

**Files:**
- Modify: `packages/db/src/repository/chunk-groups.ts` — add `subGroupBy` support
- Modify: `packages/api/src/chunks/group-service.ts` — pass through sub-group
- Modify: `packages/api/src/chunks/group-routes.ts` — add sub-group query param
- Modify: `apps/web/src/features/chunks/lazy-group-list.tsx` — nested groups
- Modify: `apps/web/src/routes/chunks.index.tsx` — compound group selector
- Modify: `apps/web/src/features/graph/graph-filter-form.tsx` — compound group selector

Compound grouping adds a second dimension: "Group by Domain, then by Type". The server returns a two-level structure: `{ groups: [{ groupName, count, subGroups: [{ groupName, count }] }] }`.

- [ ] **Step 1: Add compound counts query**

Add to `packages/db/src/repository/chunk-groups.ts`:

```typescript
export interface CompoundGroupCount {
    groupName: string;
    count: number;
    subGroups: GroupCount[];
}

export function getCompoundGroupedCounts(
    params: GroupedCountsParams & { subGroupBy: "type" | "status" | "origin" | "freshness" | "tagtype"; subTagTypeId?: string }
): Effect.Effect<CompoundGroupCount[], { _tag: "DatabaseError"; cause: unknown }> {
    return getGroupedCounts(params).pipe(
        Effect.flatMap(groups =>
            Effect.forEach(
                groups,
                group => {
                    const subParams: GroupedCountsParams = {
                        ...params,
                        groupBy: params.subGroupBy,
                        tagTypeId: params.subTagTypeId,
                    };
                    // Add the parent group as an additional filter
                    return getFilteredSubGroupCounts(subParams, params.groupBy, group.groupName, params.tagTypeId).pipe(
                        Effect.map(subGroups => ({
                            groupName: group.groupName,
                            count: group.count,
                            subGroups,
                        }))
                    );
                },
                { concurrency: 5 }
            )
        )
    );
}
```

The `getFilteredSubGroupCounts` function runs the sub-group query with an additional WHERE clause filtering to only chunks in the parent group. Implementation follows the same pattern as `getGroupedCounts` but with the parent-group constraint injected.

- [ ] **Step 2: Add compound route parameter**

In `group-routes.ts`, add optional `subGroupBy` and `subTagTypeId` query params to the `/chunks/grouped` endpoint. When present, call `getCompoundGroupedCounts` instead.

- [ ] **Step 3: Update LazyGroupList for nested rendering**

In `lazy-group-list.tsx`, when sub-groups are present, render a nested accordion: parent group header → sub-group headers → chunks. Each sub-group header is independently expandable and lazy-loads its chunks.

- [ ] **Step 4: Add compound group UI to chunks page**

In the `TagTypeGroupSelect` component in `chunks.index.tsx`, when a primary group is selected, show a secondary "then by" dropdown. Store the compound group as `group=tagtype:<id>&subGroup=type` in the URL search params.

- [ ] **Step 5: Add compound group UI to graph filter form**

In `graph-filter-form.tsx`, when a tag type is selected for grouping, show an optional "Sub-group by" section with the same options (type, status, another tag type). This maps to visual sub-clusters within each group bounding box.

- [ ] **Step 6: Run tests and type check**

Run: `pnpm run check-types && pnpm test`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/repository/chunk-groups.ts packages/api/src/chunks/group-service.ts packages/api/src/chunks/group-routes.ts apps/web/src/features/chunks/lazy-group-list.tsx apps/web/src/routes/chunks.index.tsx apps/web/src/features/graph/graph-filter-form.tsx
git commit -m "feat: add compound two-level grouping"
```

---

## Task 8: End-to-End Verification

- [ ] **Step 1: Run full test suite**

```bash
pnpm test
```

Expected: All tests pass

- [ ] **Step 2: Run type check**

```bash
pnpm run check-types
```

Expected: All packages pass

- [ ] **Step 3: Build Docker images**

```bash
docker build -f docker/build/server.Dockerfile -t fubbik-server:test .
docker build -f docker/build/web.Dockerfile -t fubbik-web:test .
```

Expected: Both build successfully

- [ ] **Step 4: Manual smoke test**

Start dev server (`pnpm dev`) and verify:
1. Navigate to `/chunks`, select a tag type grouping → see group headers with counts, no chunks loaded
2. Expand a group → chunks load on demand with virtual scrolling
3. Navigate to `/graph`, select tag grouping with a tag type → groups render
4. For large graphs, clusters appear as single nodes; clicking expands
5. Select compound grouping → nested groups appear

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "test: verify grouping at scale improvements"
```
