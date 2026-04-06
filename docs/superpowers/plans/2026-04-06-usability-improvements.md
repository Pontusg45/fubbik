# Usability Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Fubbik proactively surface stale chunks, improve relationship visibility in the graph and detail pages, and enable fast knowledge capture from the command palette and CLI.

**Architecture:** Three independent feature areas: (1) staleness detection with a new `chunk_staleness` DB table, backend service, and UI surfacing across dashboard/nav/detail; (2) relationship improvements via inline dependency views, graph focus mode, and filter presets; (3) quick capture via command palette instant-create, clipboard pre-fill shortcut, and CLI `quick` command.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router/Query, React Flow, Commander.js, shadcn-ui

---

## Phase 1: Staleness Detection & Surfacing

### Task 1: Staleness Schema & Migration

**Files:**
- Create: `packages/db/src/schema/staleness.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create the staleness schema file**

```typescript
// packages/db/src/schema/staleness.ts
import { relations, sql } from "drizzle-orm";
import { pgTable, text, timestamp, index } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { user } from "./auth";

export const chunkStaleness = pgTable(
    "chunk_staleness",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        reason: text("reason").notNull(), // "file_changed" | "age" | "diverged_duplicate"
        detail: text("detail"), // which files changed, or which chunk is duplicate
        relatedChunkId: text("related_chunk_id").references(() => chunk.id, { onDelete: "cascade" }), // for diverged_duplicate
        detectedAt: timestamp("detected_at").defaultNow().notNull(),
        dismissedAt: timestamp("dismissed_at"),
        dismissedBy: text("dismissed_by").references(() => user.id, { onDelete: "set null" }),
        suppressPair: text("suppress_pair"), // for permanently suppressing duplicate pairs (sorted id pair)
    },
    table => [
        index("staleness_chunkId_idx").on(table.chunkId),
        index("staleness_reason_idx").on(table.reason),
        index("staleness_dismissedAt_idx").on(table.dismissedAt),
    ]
);

export const stalenessRelations = relations(chunkStaleness, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkStaleness.chunkId], references: [chunk.id] }),
    relatedChunk: one(chunk, { fields: [chunkStaleness.relatedChunkId], references: [chunk.id] }),
}));

export const stalenessScan = pgTable(
    "staleness_scan",
    {
        id: text("id").primaryKey(),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" }),
        lastCommitSha: text("last_commit_sha").notNull(),
        scannedAt: timestamp("scanned_at").defaultNow().notNull(),
    },
    table => [
        index("staleness_scan_codebaseId_idx").on(table.codebaseId),
    ]
);
```

- [ ] **Step 2: Export from schema index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./staleness";
```

- [ ] **Step 3: Push the schema**

Run: `pnpm db:push`
Expected: Tables `chunk_staleness` and `staleness_scan` created successfully.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/staleness.ts packages/db/src/schema/index.ts
git commit -m "feat(db): add chunk_staleness and staleness_scan tables"
```

---

### Task 2: Staleness Repository

**Files:**
- Create: `packages/db/src/repository/staleness.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the staleness repository**

```typescript
// packages/db/src/repository/staleness.ts
import { and, eq, isNull, sql, desc, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkStaleness, stalenessScan } from "../schema/staleness";
import { chunkCodebase } from "../schema/codebase";

export function getStaleFlags(userId: string, params?: { reason?: string; codebaseId?: string; limit?: number }) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                isNull(chunkStaleness.dismissedAt),
                isNull(chunkStaleness.suppressPair),
            ];
            if (params?.reason) conditions.push(eq(chunkStaleness.reason, params.reason));
            if (params?.codebaseId) {
                const inCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase).where(eq(chunkCodebase.codebaseId, params.codebaseId));
                conditions.push(sql`${chunkStaleness.chunkId} IN (${inCodebase})`);
            }

            const flags = await db
                .select({
                    id: chunkStaleness.id,
                    chunkId: chunkStaleness.chunkId,
                    chunkTitle: chunk.title,
                    chunkType: chunk.type,
                    reason: chunkStaleness.reason,
                    detail: chunkStaleness.detail,
                    relatedChunkId: chunkStaleness.relatedChunkId,
                    detectedAt: chunkStaleness.detectedAt,
                })
                .from(chunkStaleness)
                .innerJoin(chunk, eq(chunkStaleness.chunkId, chunk.id))
                .where(and(...conditions))
                .orderBy(desc(chunkStaleness.detectedAt))
                .limit(params?.limit ?? 50);

            return flags;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function getStaleCount(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                isNull(chunkStaleness.dismissedAt),
                isNull(chunkStaleness.suppressPair),
            ];
            if (codebaseId) {
                const inCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase).where(eq(chunkCodebase.codebaseId, codebaseId));
                conditions.push(sql`${chunkStaleness.chunkId} IN (${inCodebase})`);
            }
            const result = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkStaleness)
                .innerJoin(chunk, eq(chunkStaleness.chunkId, chunk.id))
                .where(and(...conditions));
            return Number(result[0]?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function getStaleFlagsForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: async () => {
            return db
                .select()
                .from(chunkStaleness)
                .where(and(eq(chunkStaleness.chunkId, chunkId), isNull(chunkStaleness.dismissedAt), isNull(chunkStaleness.suppressPair)));
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function createStaleFlag(data: { id: string; chunkId: string; reason: string; detail?: string; relatedChunkId?: string }) {
    return Effect.tryPromise({
        try: async () => {
            await db.insert(chunkStaleness).values(data).onConflictDoNothing();
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function dismissStaleFlag(flagId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            await db.update(chunkStaleness).set({ dismissedAt: new Date(), dismissedBy: userId }).where(eq(chunkStaleness.id, flagId));
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function suppressDuplicatePair(chunkIdA: string, chunkIdB: string) {
    const pairKey = [chunkIdA, chunkIdB].sort().join(":");
    return Effect.tryPromise({
        try: async () => {
            await db.update(chunkStaleness).set({ suppressPair: pairKey }).where(
                and(
                    sql`${chunkStaleness.suppressPair} IS NULL`,
                    sql`(${chunkStaleness.chunkId} = ${chunkIdA} AND ${chunkStaleness.relatedChunkId} = ${chunkIdB}) OR (${chunkStaleness.chunkId} = ${chunkIdB} AND ${chunkStaleness.relatedChunkId} = ${chunkIdA})`
                )
            );
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function getLastScan(codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const rows = await db
                .select()
                .from(stalenessScan)
                .where(eq(stalenessScan.codebaseId, codebaseId))
                .orderBy(desc(stalenessScan.scannedAt))
                .limit(1);
            return rows[0] ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function upsertScan(data: { id: string; codebaseId: string; lastCommitSha: string }) {
    return Effect.tryPromise({
        try: async () => {
            await db.insert(stalenessScan).values(data).onConflictDoNothing();
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./staleness";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/staleness.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add staleness repository with flag CRUD and scan tracking"
```

---

### Task 3: Staleness API Routes

**Files:**
- Create: `packages/api/src/staleness/routes.ts`
- Create: `packages/api/src/staleness/service.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create the staleness service**

```typescript
// packages/api/src/staleness/service.ts
import { getStaleFlags, getStaleCount, getStaleFlagsForChunk, dismissStaleFlag, suppressDuplicatePair } from "@fubbik/db/repository";

export { getStaleFlags, getStaleCount, getStaleFlagsForChunk, dismissStaleFlag, suppressDuplicatePair };
```

- [ ] **Step 2: Create the staleness routes**

```typescript
// packages/api/src/staleness/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as stalenessService from "./service";

export const stalenessRoutes = new Elysia()
    .get(
        "/chunks/stale",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        stalenessService.getStaleFlags(session.user.id, {
                            reason: ctx.query.reason,
                            codebaseId: ctx.query.codebaseId,
                            limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
                        })
                    )
                )
            ),
        {
            query: t.Object({
                reason: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                limit: t.Optional(t.String()),
            }),
        }
    )
    .get(
        "/chunks/stale/count",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        stalenessService.getStaleCount(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
            }),
        }
    )
    .post(
        "/chunks/:id/dismiss-staleness",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        stalenessService.dismissStaleFlag(ctx.params.id, session.user.id)
                    )
                )
            ),
        {
            params: t.Object({ id: t.String() }),
        }
    )
    .post(
        "/chunks/suppress-duplicate",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        stalenessService.suppressDuplicatePair(ctx.body.chunkIdA, ctx.body.chunkIdB)
                    )
                )
            ),
        {
            body: t.Object({
                chunkIdA: t.String(),
                chunkIdB: t.String(),
            }),
        }
    );
```

- [ ] **Step 3: Register routes in the API index**

Add to `packages/api/src/index.ts` — import and `.use()`:
```typescript
import { stalenessRoutes } from "./staleness/routes";
// ... in the .use() chain:
.use(stalenessRoutes)
```

- [ ] **Step 4: Verify the server starts**

Run: `cd packages/api && pnpm build` (or `pnpm dev` and hit `/api/chunks/stale`)
Expected: No compilation errors, endpoint returns `[]`.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/staleness/ packages/api/src/index.ts
git commit -m "feat(api): add staleness API routes (list, count, dismiss, suppress)"
```

---

### Task 4: Age-Based Staleness Detection

**Files:**
- Create: `packages/api/src/staleness/detect-age.ts`
- Modify: `packages/api/src/staleness/service.ts`
- Modify: `packages/api/src/staleness/routes.ts`

- [ ] **Step 1: Create age detection function**

```typescript
// packages/api/src/staleness/detect-age.ts
import { and, eq, isNull, lt, sql } from "drizzle-orm";
import { Effect } from "effect";
import { nanoid } from "nanoid";

import { DatabaseError } from "@fubbik/db/errors";
import { db } from "@fubbik/db";
import { chunk } from "@fubbik/db/schema";
import { chunkCodebase } from "@fubbik/db/schema";
import { chunkStaleness } from "@fubbik/db/schema";

export function detectAgeStaleChunks(userId: string, codebaseId?: string, thresholdDays = 90) {
    return Effect.tryPromise({
        try: async () => {
            const threshold = sql`NOW() - INTERVAL '${sql.raw(String(thresholdDays))} days'`;

            const conditions = [
                eq(chunk.userId, userId),
                lt(chunk.updatedAt, threshold),
                isNull(chunk.archivedAt),
            ];
            if (codebaseId) {
                const inCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase).where(eq(chunkCodebase.codebaseId, codebaseId));
                conditions.push(sql`${chunk.id} IN (${inCodebase})`);
            }

            // Find old chunks not already flagged for age
            const alreadyFlagged = db
                .select({ chunkId: chunkStaleness.chunkId })
                .from(chunkStaleness)
                .where(and(eq(chunkStaleness.reason, "age"), isNull(chunkStaleness.dismissedAt)));

            conditions.push(sql`${chunk.id} NOT IN (${alreadyFlagged})`);

            const staleChunks = await db
                .select({ id: chunk.id, title: chunk.title, updatedAt: chunk.updatedAt })
                .from(chunk)
                .where(and(...conditions));

            // Create flags
            const flags = staleChunks.map(c => ({
                id: nanoid(),
                chunkId: c.id,
                reason: "age" as const,
                detail: `Last updated ${c.updatedAt.toISOString().split("T")[0]}`,
            }));

            if (flags.length > 0) {
                await db.insert(chunkStaleness).values(flags).onConflictDoNothing();
            }

            return { flagged: flags.length };
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

- [ ] **Step 2: Add scan trigger route**

Add to `packages/api/src/staleness/routes.ts`:
```typescript
import { detectAgeStaleChunks } from "./detect-age";

// Add this route:
.post(
    "/chunks/stale/scan-age",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    detectAgeStaleChunks(session.user.id, ctx.body.codebaseId, ctx.body.thresholdDays)
                )
            )
        ),
    {
        body: t.Object({
            codebaseId: t.Optional(t.String()),
            thresholdDays: t.Optional(t.Number()),
        }),
    }
)
```

- [ ] **Step 3: Verify scan works**

Run: `curl -X POST http://localhost:3000/api/chunks/stale/scan-age -H "Content-Type: application/json" -d '{}'`
Expected: `{"flagged": N}` where N is the count of chunks older than 90 days.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/staleness/detect-age.ts packages/api/src/staleness/routes.ts packages/api/src/staleness/service.ts
git commit -m "feat(staleness): add age-based staleness detection (90-day threshold)"
```

---

### Task 5: Dashboard "Attention Needed" Widget

**Files:**
- Create: `apps/web/src/features/staleness/attention-needed.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1: Create the AttentionNeeded component**

```tsx
// apps/web/src/features/staleness/attention-needed.tsx
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { AlertTriangle, Check, Clock, Copy, FileEdit, GitCommit } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const REASON_CONFIG = {
    file_changed: { icon: GitCommit, label: "Files changed", color: "text-amber-500" },
    age: { icon: Clock, label: "Getting old", color: "text-orange-500" },
    diverged_duplicate: { icon: Copy, label: "Possible duplicate", color: "text-blue-500" },
} as const;

export function AttentionNeeded() {
    const queryClient = useQueryClient();

    const staleQuery = useQuery({
        queryKey: ["stale-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.get({ query: { limit: "10" } })),
    });

    const dismissMutation = useMutation({
        mutationFn: async (flagId: string) => {
            await api.api.chunks[flagId as string]["dismiss-staleness"].post();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["stale-chunks"] });
            queryClient.invalidateQueries({ queryKey: ["stale-count"] });
            toast.success("Dismissed");
        },
    });

    const flags = staleQuery.data ?? [];
    if (flags.length === 0) return null;

    // Group by reason
    const grouped = new Map<string, typeof flags>();
    for (const flag of flags) {
        const group = grouped.get(flag.reason) ?? [];
        group.push(flag);
        grouped.set(flag.reason, group);
    }

    return (
        <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-4">
            <div className="mb-3 flex items-center gap-2">
                <AlertTriangle className="size-4 text-amber-500" />
                <h3 className="text-sm font-semibold">Attention Needed</h3>
                <Badge variant="secondary" size="sm">{flags.length}</Badge>
            </div>
            <div className="space-y-3">
                {Array.from(grouped.entries()).map(([reason, items]) => {
                    const config = REASON_CONFIG[reason as keyof typeof REASON_CONFIG] ?? REASON_CONFIG.age;
                    const Icon = config.icon;
                    return (
                        <div key={reason}>
                            <p className={`mb-1.5 flex items-center gap-1.5 text-xs font-medium ${config.color}`}>
                                <Icon className="size-3" />
                                {config.label}
                            </p>
                            <div className="space-y-1">
                                {items.map(flag => (
                                    <div key={flag.id} className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-muted/50">
                                        <Link
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: flag.chunkId }}
                                            className="min-w-0 flex-1 truncate hover:underline"
                                        >
                                            {flag.chunkTitle}
                                        </Link>
                                        {flag.detail && (
                                            <span className="text-muted-foreground hidden text-xs md:inline">{flag.detail}</span>
                                        )}
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            className="size-6 shrink-0 p-0"
                                            onClick={() => dismissMutation.mutate(flag.id)}
                                            disabled={dismissMutation.isPending}
                                        >
                                            <Check className="size-3" />
                                        </Button>
                                    </div>
                                ))}
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Add to dashboard page**

In `apps/web/src/routes/dashboard.tsx`, import and render `AttentionNeeded` near the top of the dashboard content (after the welcome wizard, before stats):

```tsx
import { AttentionNeeded } from "@/features/staleness/attention-needed";

// In the JSX, add before the stats section:
<AttentionNeeded />
```

- [ ] **Step 3: Verify visually**

Run: `pnpm dev`, navigate to `/dashboard`.
Expected: If there are stale flags, the amber widget appears. If none, nothing renders.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/staleness/attention-needed.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat(dashboard): add Attention Needed widget for stale chunks"
```

---

### Task 6: Nav Badge for Stale Count

**Files:**
- Create: `apps/web/src/features/staleness/use-stale-count.ts`
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Create the stale count hook**

```tsx
// apps/web/src/features/staleness/use-stale-count.ts
import { useQuery } from "@tanstack/react-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function useStaleCount() {
    const query = useQuery({
        queryKey: ["stale-count"],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.count.get({ query: {} })),
        refetchInterval: 5 * 60 * 1000, // refresh every 5 min
    });
    return query.data ?? 0;
}
```

- [ ] **Step 2: Add amber dot to Dashboard nav link**

In `apps/web/src/routes/__root.tsx`, find the Dashboard nav link and add a stale count indicator:

```tsx
import { useStaleCount } from "@/features/staleness/use-stale-count";

// Inside the nav component:
const staleCount = useStaleCount();

// Replace the Dashboard Link with:
<Link to="/dashboard" className="relative ...">
    Dashboard
    {staleCount > 0 && (
        <span className="absolute -top-1 -right-1 flex size-4 items-center justify-center rounded-full bg-amber-500 text-[10px] font-bold text-white">
            {staleCount > 9 ? "9+" : staleCount}
        </span>
    )}
</Link>
```

- [ ] **Step 3: Verify visually**

Run: `pnpm dev`, check the nav bar.
Expected: Amber badge shows on Dashboard link when stale chunks exist.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/staleness/use-stale-count.ts apps/web/src/routes/__root.tsx
git commit -m "feat(nav): add stale chunk count badge on Dashboard link"
```

---

### Task 7: Chunk Detail Staleness Banner

**Files:**
- Create: `apps/web/src/features/staleness/staleness-banner.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Create the StalenessBanner component**

```tsx
// apps/web/src/features/staleness/staleness-banner.tsx
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { AlertTriangle, Check, Clock, Copy, GitCommit, X } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const REASON_ICONS = {
    file_changed: GitCommit,
    age: Clock,
    diverged_duplicate: Copy,
} as const;

function formatReason(flag: { reason: string; detail?: string | null; relatedChunkId?: string | null }) {
    switch (flag.reason) {
        case "file_changed":
            return `Files linked to this chunk changed: ${flag.detail ?? "unknown files"}`;
        case "age":
            return `This chunk hasn't been updated in a while. ${flag.detail ?? ""}`;
        case "diverged_duplicate":
            return "This chunk is very similar to another chunk.";
        default:
            return "This chunk may need attention.";
    }
}

export function StalenessBanner({ chunkId }: { chunkId: string }) {
    const queryClient = useQueryClient();

    const flagsQuery = useQuery({
        queryKey: ["staleness-flags", chunkId],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.get({ query: { limit: "5" } })),
        select: (data: Array<{ id: string; chunkId: string; reason: string; detail?: string | null; relatedChunkId?: string | null }>) =>
            data.filter(f => f.chunkId === chunkId),
    });

    const dismissMutation = useMutation({
        mutationFn: async (flagId: string) => {
            await api.api.chunks[flagId as string]["dismiss-staleness"].post();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["staleness-flags", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["stale-chunks"] });
            queryClient.invalidateQueries({ queryKey: ["stale-count"] });
            toast.success("Dismissed");
        },
    });

    const flags = flagsQuery.data ?? [];
    if (flags.length === 0) return null;

    return (
        <div className="space-y-2">
            {flags.map(flag => {
                const Icon = REASON_ICONS[flag.reason as keyof typeof REASON_ICONS] ?? AlertTriangle;
                return (
                    <div key={flag.id} className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/5 p-3">
                        <Icon className="mt-0.5 size-4 shrink-0 text-amber-500" />
                        <div className="min-w-0 flex-1">
                            <p className="text-sm">{formatReason(flag)}</p>
                            {flag.reason === "diverged_duplicate" && flag.relatedChunkId && (
                                <Link
                                    to="/compare"
                                    search={{ a: chunkId, b: flag.relatedChunkId }}
                                    className="text-primary mt-1 inline-block text-xs hover:underline"
                                >
                                    Compare side by side
                                </Link>
                            )}
                        </div>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="size-7 shrink-0 p-0"
                            onClick={() => dismissMutation.mutate(flag.id)}
                            disabled={dismissMutation.isPending}
                        >
                            <X className="size-3.5" />
                        </Button>
                    </div>
                );
            })}
        </div>
    );
}
```

- [ ] **Step 2: Add banner to chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, import and render `StalenessBanner` at the top of the chunk detail content (after the header, before the content area):

```tsx
import { StalenessBanner } from "@/features/staleness/staleness-banner";

// In the JSX, near the top of the detail:
<StalenessBanner chunkId={chunkId} />
```

- [ ] **Step 3: Verify visually**

Run: `pnpm dev`, navigate to a chunk that has staleness flags.
Expected: Amber banner(s) appear at the top of the detail page with dismiss buttons.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/staleness/staleness-banner.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): add staleness warning banner on chunk detail page"
```

---

## Phase 2: Relationship Improvements

### Task 8: Chunk Detail Dependency Tree

**Files:**
- Create: `apps/web/src/features/chunks/dependency-tree.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Create the DependencyTree component**

```tsx
// apps/web/src/features/chunks/dependency-tree.tsx
import { Link } from "@tanstack/react-router";
import { ArrowDownRight, ArrowUpRight } from "lucide-react";

import { Badge } from "@/components/ui/badge";

interface Connection {
    id: string;
    sourceId: string;
    targetId: string;
    relation: string;
    source?: { id: string; title: string; type: string };
    target?: { id: string; title: string; type: string };
}

interface DependencyTreeProps {
    chunkId: string;
    connections: Connection[];
}

function groupByRelation(connections: Connection[]) {
    const groups = new Map<string, Connection[]>();
    for (const conn of connections) {
        const list = groups.get(conn.relation) ?? [];
        list.push(conn);
        groups.set(conn.relation, list);
    }
    return groups;
}

const RELATION_LABELS: Record<string, string> = {
    depends_on: "Depends on",
    part_of: "Part of",
    extends: "Extends",
    references: "References",
    supports: "Supports",
    contradicts: "Contradicts",
    alternative_to: "Alternative to",
    related_to: "Related to",
};

export function DependencyTree({ chunkId, connections }: DependencyTreeProps) {
    const outgoing = connections.filter(c => c.sourceId === chunkId);
    const incoming = connections.filter(c => c.targetId === chunkId);

    const outgoingGroups = groupByRelation(outgoing);
    const incomingGroups = groupByRelation(incoming);

    if (outgoing.length === 0 && incoming.length === 0) return null;

    return (
        <div className="grid gap-4 md:grid-cols-2">
            {outgoing.length > 0 && (
                <div>
                    <h4 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        <ArrowDownRight className="size-3" />
                        Outgoing ({outgoing.length})
                    </h4>
                    <div className="space-y-3">
                        {Array.from(outgoingGroups.entries()).map(([relation, conns]) => (
                            <div key={relation}>
                                <Badge variant="outline" size="sm" className="mb-1.5">{RELATION_LABELS[relation] ?? relation}</Badge>
                                <div className="space-y-1 pl-2 border-l-2 border-muted">
                                    {conns.map(c => (
                                        <Link
                                            key={c.id}
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: c.targetId }}
                                            className="block truncate rounded px-2 py-1 text-sm hover:bg-muted"
                                        >
                                            {c.target?.title ?? c.targetId.slice(0, 8)}
                                        </Link>
                                    ))}
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            )}
            {incoming.length > 0 && (
                <div>
                    <h4 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        <ArrowUpRight className="size-3" />
                        Incoming ({incoming.length})
                    </h4>
                    <div className="space-y-3">
                        {Array.from(incomingGroups.entries()).map(([relation, conns]) => (
                            <div key={relation}>
                                <Badge variant="outline" size="sm" className="mb-1.5">{RELATION_LABELS[relation] ?? relation}</Badge>
                                <div className="space-y-1 pl-2 border-l-2 border-muted">
                                    {conns.map(c => (
                                        <Link
                                            key={c.id}
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: c.sourceId }}
                                            className="block truncate rounded px-2 py-1 text-sm hover:bg-muted"
                                        >
                                            {c.source?.title ?? c.sourceId.slice(0, 8)}
                                        </Link>
                                    ))}
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Integrate into chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, replace or augment the existing connections section with `DependencyTree`:

```tsx
import { DependencyTree } from "@/features/chunks/dependency-tree";

// Replace the flat connections list with:
<DependencyTree chunkId={chunkId} connections={chunk.connections} />
```

- [ ] **Step 3: Verify visually**

Expected: Chunk detail page shows outgoing/incoming connections grouped by relation type in a two-column layout.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/dependency-tree.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): replace flat connections with grouped dependency tree view"
```

---

### Task 9: Graph Focus Mode

**Files:**
- Modify: `apps/web/src/features/graph/graph-view.tsx`
- Modify: `apps/web/src/features/graph/graph-node.tsx`

- [ ] **Step 1: Add focus state to graph-view.tsx**

Add state and logic to `apps/web/src/features/graph/graph-view.tsx`:

```tsx
// Add state near other useState calls:
const [focusNodeId, setFocusNodeId] = useState<string | null>(null);

// Add a function to compute nodes within N hops:
function getNodesWithinHops(nodeId: string, edges: Edge[], maxHops: number): Set<string> {
    const visited = new Set<string>([nodeId]);
    let frontier = new Set<string>([nodeId]);
    for (let hop = 0; hop < maxHops; hop++) {
        const next = new Set<string>();
        for (const edge of edges) {
            if (frontier.has(edge.source) && !visited.has(edge.target)) {
                next.add(edge.target);
                visited.add(edge.target);
            }
            if (frontier.has(edge.target) && !visited.has(edge.source)) {
                next.add(edge.source);
                visited.add(edge.source);
            }
        }
        frontier = next;
    }
    return visited;
}

// In the onNodeClick handler, toggle focus mode:
const onNodeClick = useCallback((_event: React.MouseEvent, node: Node) => {
    // If already focused on this node, exit focus mode
    if (focusNodeId === node.id) {
        setFocusNodeId(null);
        return;
    }
    setFocusNodeId(node.id);
}, [focusNodeId]);

// Apply opacity styles to nodes and edges based on focus:
const styledNodes = useMemo(() => {
    if (!focusNodeId) return nodes;
    const visible = getNodesWithinHops(focusNodeId, edges, 2);
    return nodes.map(n => ({
        ...n,
        style: {
            ...n.style,
            opacity: visible.has(n.id) ? 1 : 0.15,
            transition: "opacity 0.3s ease",
        },
    }));
}, [nodes, edges, focusNodeId]);

const styledEdges = useMemo(() => {
    if (!focusNodeId) return edges;
    const visible = getNodesWithinHops(focusNodeId, edges, 2);
    return edges.map(e => ({
        ...e,
        style: {
            ...e.style,
            opacity: visible.has(e.source) && visible.has(e.target) ? 1 : 0.08,
            transition: "opacity 0.3s ease",
        },
    }));
}, [nodes, edges, focusNodeId]);

// Pass styledNodes/styledEdges to ReactFlow instead of nodes/edges
// Add Escape handler to exit focus mode:
useEffect(() => {
    const handler = (e: KeyboardEvent) => {
        if (e.key === "Escape" && focusNodeId) {
            setFocusNodeId(null);
        }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
}, [focusNodeId]);
```

- [ ] **Step 2: Add focus mode indicator**

Show a small indicator when focus mode is active:

```tsx
// Near the Controls component:
{focusNodeId && (
    <div className="absolute bottom-16 left-1/2 z-10 -translate-x-1/2 rounded-full border bg-background/90 px-4 py-1.5 text-xs shadow-sm backdrop-blur-sm">
        Focus mode — click node again or press <kbd className="mx-1 rounded border px-1.5 py-0.5 font-mono">Esc</kbd> to exit
    </div>
)}
```

- [ ] **Step 3: Verify visually**

Run: `pnpm dev`, navigate to `/graph`, click a node.
Expected: Nodes beyond 2 hops are dimmed. Clicking the same node or pressing Escape restores full opacity.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/graph-view.tsx
git commit -m "feat(graph): add focus mode — click node to dim nodes beyond 2 hops"
```

---

### Task 10: Graph Filter Presets

**Files:**
- Create: `apps/web/src/features/graph/filter-presets.tsx`
- Modify: `apps/web/src/features/graph/graph-filters.tsx`

- [ ] **Step 1: Create filter presets component**

```tsx
// apps/web/src/features/graph/filter-presets.tsx
import { Bookmark, Plus, Trash2 } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { useLocalStorage } from "@/hooks/use-local-storage";

export interface FilterState {
    activeTypes: string[];
    activeRelations: string[];
    activeTagTypeIds: string[];
}

interface FilterPreset {
    name: string;
    filters: FilterState;
}

interface FilterPresetsProps {
    currentFilters: FilterState;
    onApplyPreset: (filters: FilterState) => void;
}

export function FilterPresets({ currentFilters, onApplyPreset }: FilterPresetsProps) {
    const [presets, setPresets] = useLocalStorage<FilterPreset[]>("fubbik:graph-filter-presets", []);
    const [naming, setNaming] = useState(false);
    const [name, setName] = useState("");

    function savePreset() {
        if (!name.trim()) return;
        setPresets(prev => [...prev, { name: name.trim(), filters: currentFilters }]);
        setName("");
        setNaming(false);
    }

    function deletePreset(index: number) {
        setPresets(prev => prev.filter((_, i) => i !== index));
    }

    return (
        <div className="flex items-center gap-1.5">
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm" className="h-7 gap-1.5 text-xs">
                        <Bookmark className="size-3" />
                        Presets
                    </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                    {presets.length === 0 && (
                        <p className="text-muted-foreground px-2 py-1.5 text-xs">No saved presets</p>
                    )}
                    {presets.map((preset, i) => (
                        <DropdownMenuItem key={i} className="flex items-center justify-between gap-2">
                            <button type="button" className="flex-1 text-left" onClick={() => onApplyPreset(preset.filters)}>
                                {preset.name}
                            </button>
                            <button
                                type="button"
                                className="text-muted-foreground hover:text-destructive shrink-0"
                                onClick={e => { e.stopPropagation(); deletePreset(i); }}
                            >
                                <Trash2 className="size-3" />
                            </button>
                        </DropdownMenuItem>
                    ))}
                </DropdownMenuContent>
            </DropdownMenu>
            {naming ? (
                <form onSubmit={e => { e.preventDefault(); savePreset(); }} className="flex items-center gap-1">
                    <input
                        type="text"
                        value={name}
                        onChange={e => setName(e.target.value)}
                        placeholder="Preset name..."
                        className="h-7 rounded border bg-background px-2 text-xs outline-none"
                        autoFocus
                        onBlur={() => { if (!name.trim()) setNaming(false); }}
                    />
                    <Button type="submit" variant="ghost" size="sm" className="h-7 text-xs">Save</Button>
                </form>
            ) : (
                <Button variant="ghost" size="sm" className="h-7 gap-1 text-xs" onClick={() => setNaming(true)}>
                    <Plus className="size-3" />
                    Save
                </Button>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Integrate presets into graph filters**

In `apps/web/src/features/graph/graph-filters.tsx`, add the `FilterPresets` component and wire up the apply callback to set the active types/relations/tag types.

- [ ] **Step 3: Verify visually**

Expected: Graph page shows "Presets" dropdown + "Save" button next to filters. Saving and loading presets works.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/filter-presets.tsx apps/web/src/features/graph/graph-filters.tsx
git commit -m "feat(graph): add saveable filter presets (localStorage)"
```

---

### Task 11: Related Chunks Suggestions on Detail Page

**Files:**
- Create: `apps/web/src/features/chunks/related-suggestions.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Create RelatedSuggestions component**

This uses the existing `/api/chunks/search/semantic` endpoint to find similar chunks and suggest connections.

```tsx
// apps/web/src/features/chunks/related-suggestions.tsx
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { LinkIcon, Sparkles, X } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useLocalStorage } from "@/hooks/use-local-storage";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface RelatedSuggestionsProps {
    chunkId: string;
    chunkTitle: string;
    connectedIds: string[];
}

export function RelatedSuggestions({ chunkId, chunkTitle, connectedIds }: RelatedSuggestionsProps) {
    const queryClient = useQueryClient();
    const [dismissed, setDismissed] = useLocalStorage<string[]>(`fubbik:dismissed-suggestions:${chunkId}`, []);

    const suggestionsQuery = useQuery({
        queryKey: ["related-suggestions", chunkId],
        queryFn: async () => {
            try {
                const result = unwrapEden(
                    await api.api.chunks.search.semantic.get({
                        query: { chunkId, limit: "8" },
                    })
                );
                return result as Array<{ id: string; title: string; type: string; similarity: number }>;
            } catch {
                return [];
            }
        },
        staleTime: 60_000,
    });

    const linkMutation = useMutation({
        mutationFn: async (targetId: string) => {
            await api.api.connections.post({
                sourceId: chunkId,
                targetId,
                relation: "related_to",
            });
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["related-suggestions", chunkId] });
            toast.success("Connection created");
        },
    });

    const allSuggestions = suggestionsQuery.data ?? [];
    const excludeIds = new Set([chunkId, ...connectedIds, ...dismissed]);
    const suggestions = allSuggestions.filter(s => !excludeIds.has(s.id)).slice(0, 5);

    if (suggestions.length === 0) return null;

    return (
        <div className="rounded-lg border border-dashed p-3">
            <h4 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                <Sparkles className="size-3" />
                You might want to connect
            </h4>
            <div className="space-y-1">
                {suggestions.map(s => (
                    <div key={s.id} className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-muted/50">
                        <Link
                            to="/chunks/$chunkId"
                            params={{ chunkId: s.id }}
                            className="min-w-0 flex-1 truncate hover:underline"
                        >
                            {s.title}
                        </Link>
                        <span className="text-muted-foreground text-xs">{Math.round(s.similarity * 100)}%</span>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="size-6 shrink-0 p-0"
                            onClick={() => linkMutation.mutate(s.id)}
                            disabled={linkMutation.isPending}
                        >
                            <LinkIcon className="size-3" />
                        </Button>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="size-6 shrink-0 p-0"
                            onClick={() => setDismissed(prev => [...prev, s.id])}
                        >
                            <X className="size-3" />
                        </Button>
                    </div>
                ))}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Add to chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, render below the DependencyTree:

```tsx
import { RelatedSuggestions } from "@/features/chunks/related-suggestions";

<RelatedSuggestions
    chunkId={chunkId}
    chunkTitle={chunk.title}
    connectedIds={chunk.connections.map(c => c.sourceId === chunkId ? c.targetId : c.sourceId)}
/>
```

- [ ] **Step 3: Verify visually**

Expected: Suggestion cards appear below connections with similarity % and link/dismiss buttons. Requires Ollama embeddings to work.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/related-suggestions.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): add embedding-based related chunk suggestions on detail page"
```

---

## Phase 3: Quick Capture

### Task 12: Command Palette Quick Note

**Files:**
- Modify: `apps/web/src/features/command-palette/command-palette.tsx`

- [ ] **Step 1: Add quick note creation to command palette**

In `apps/web/src/features/command-palette/command-palette.tsx`, add a mutation and a dynamic "Quick note" action item.

Add a mutation inside `CommandPalette()`:

```tsx
const quickNoteMutation = useMutation({
    mutationFn: async (title: string) => {
        const result = unwrapEden(
            await api.api.chunks.post({
                title,
                content: "",
                type: "note",
            })
        );
        return result;
    },
    onSuccess: (data) => {
        toast.success(`Created "${query.trim()}"`, {
            action: {
                label: "Edit",
                onClick: () => navigate({ to: "/chunks/$chunkId/edit", params: { chunkId: data.id } }),
            },
        });
        close();
    },
    onError: () => {
        toast.error("Failed to create note");
    },
});
```

Then in the `items` useMemo, add a "Quick note" item when query has text and doesn't start with `#` or `*`:

```tsx
// After the Actions section, before return result:
if (query.trim().length > 0 && !isTagSearch && !isFederatedSearch && !subMode) {
    result.push({
        id: "quick-note",
        title: `Quick note: "${query.trim()}"`,
        group: "Actions",
        icon: <Plus className="size-4" />,
        badge: "Create",
        onSelect: () => quickNoteMutation.mutate(query.trim()),
    });
}
```

- [ ] **Step 2: Verify**

Run: `pnpm dev`, open Cmd+K, type "Convention: always use Effect", select "Quick note" action.
Expected: Toast shows "Created 'Convention: always use Effect'" with "Edit" link. Chunk created in DB.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/command-palette/command-palette.tsx
git commit -m "feat(command-palette): add quick note creation from search query"
```

---

### Task 13: Clipboard-to-Chunk Shortcut

**Files:**
- Modify: `apps/web/src/features/nav/keyboard-shortcuts.tsx`

- [ ] **Step 1: Add Shift+N shortcut**

In `apps/web/src/features/nav/keyboard-shortcuts.tsx`, add a case in the `handleKeyDown` switch:

```tsx
case "N": {
    // Shift+N — clipboard to chunk
    if (e.shiftKey) {
        e.preventDefault();
        navigator.clipboard.readText().then(text => {
            if (!text.trim()) {
                navigate({ to: "/chunks/new" });
                return;
            }
            // Extract title from first markdown heading if present
            const headingMatch = text.match(/^#\s+(.+)$/m);
            const title = headingMatch?.[1] ?? "";
            const searchParams: Record<string, string> = { content: text };
            if (title) searchParams.title = title;
            navigate({ to: "/chunks/new", search: searchParams });
        }).catch(() => {
            // Clipboard access denied, just open new chunk page
            navigate({ to: "/chunks/new" });
        });
    }
    break;
}
```

- [ ] **Step 2: Add to shortcuts help list**

Add to the `shortcuts` array in the Global section:

```typescript
{ key: "Shift+N", description: "New chunk from clipboard" },
```

- [ ] **Step 3: Handle query params in chunks/new page**

In `apps/web/src/routes/chunks.new.tsx`, read `content` and `title` from search params and use them as initial values:

```tsx
// In the route search schema:
export const Route = createFileRoute("/chunks/new")({
    validateSearch: z.object({
        type: z.string().optional(),
        content: z.string().optional(),
        title: z.string().optional(),
    }),
    // ...
});

// Use in component:
const { type, content: initialContent, title: initialTitle } = Route.useSearch();
// Set as initial state for the form fields
```

- [ ] **Step 4: Verify**

Copy markdown to clipboard, press Shift+N.
Expected: `/chunks/new` opens with content pre-filled. If clipboard had `# Title`, title field is also pre-filled.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/nav/keyboard-shortcuts.tsx apps/web/src/routes/chunks.new.tsx
git commit -m "feat: add Shift+N shortcut to create chunk from clipboard content"
```

---

### Task 14: CLI `quick` Command

**Files:**
- Create: `apps/cli/src/commands/quick.ts`
- Modify: `apps/cli/src/index.ts` (register command)

- [ ] **Step 1: Create the quick command**

```typescript
// apps/cli/src/commands/quick.ts
import { Command } from "commander";
import { readFileSync } from "node:fs";

import { formatId, formatSuccess } from "../lib/colors";
import { loadConfig } from "../lib/config";
import { output, outputError } from "../lib/output";
import { addChunk, getServerUrl } from "../lib/store";

export const quickCommand = new Command("quick")
    .description("Quickly create a note chunk")
    .argument("[title...]", "chunk title (joined with spaces)")
    .option("--title <title>", "chunk title (when piping content via stdin)")
    .option("--type <type>", "chunk type", "note")
    .option("--tags <tags>", "comma-separated tags", "")
    .option("--global", "skip codebase scoping")
    .option("--codebase <name>", "scope to a specific codebase by name")
    .action(async (titleParts: string[], opts: {
        title?: string;
        type: string;
        tags: string;
        global?: boolean;
        codebase?: string;
    }, cmd: Command) => {
        const config = loadConfig();
        const serverUrl = getServerUrl();

        // Determine title and content
        let title = opts.title ?? titleParts.join(" ");
        let content = "";

        // Check for piped stdin
        if (!process.stdin.isTTY) {
            try {
                content = readFileSync("/dev/stdin", "utf-8").trim();
            } catch {
                // No stdin data
            }
        }

        if (!title) {
            outputError("Title is required. Usage: fubbik quick \"My note title\"");
            process.exitCode = 1;
            return;
        }

        const tags = opts.tags ? opts.tags.split(",").map(t => t.trim()).filter(Boolean) : [];

        if (serverUrl) {
            try {
                const body: Record<string, unknown> = {
                    title,
                    content,
                    type: opts.type,
                    tags,
                };

                // Resolve codebase
                if (!opts.global && !opts.codebase) {
                    // Auto-detect from git remote
                    try {
                        const { execSync } = await import("node:child_process");
                        const remoteUrl = execSync("git remote get-url origin", { encoding: "utf-8" }).trim();
                        if (remoteUrl) {
                            const detectRes = await fetch(`${serverUrl}/api/codebases/detect?remoteUrl=${encodeURIComponent(remoteUrl)}`);
                            if (detectRes.ok) {
                                const detected = await detectRes.json() as { id: string };
                                if (detected?.id) body.codebaseIds = [detected.id];
                            }
                        }
                    } catch {
                        // No git remote, skip codebase
                    }
                } else if (opts.codebase) {
                    const cbRes = await fetch(`${serverUrl}/api/codebases`);
                    if (cbRes.ok) {
                        const codebases = await cbRes.json() as { id: string; name: string }[];
                        const match = codebases.find(c => c.name.toLowerCase() === opts.codebase!.toLowerCase());
                        if (match) body.codebaseIds = [match.id];
                    }
                }

                const res = await fetch(`${serverUrl}/api/chunks`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify(body),
                });

                if (!res.ok) {
                    const err = await res.text();
                    outputError(`Server error: ${err}`);
                    process.exitCode = 1;
                    return;
                }

                const created = await res.json() as { id: string };
                output(`${formatSuccess("Created")} ${formatId(created.id)}`);
                output(`${serverUrl.replace("/api", "").replace(":3000", ":3001")}/chunks/${created.id}`);
            } catch (err) {
                outputError(`Failed to create chunk: ${err}`);
                process.exitCode = 1;
            }
        } else {
            // Local-only mode
            const chunk = addChunk({ title, content, type: opts.type, tags });
            output(`${formatSuccess("Created")} ${formatId(chunk.id)} — ${title}`);
        }
    });
```

- [ ] **Step 2: Register in CLI index**

In `apps/cli/src/index.ts`, import and add the command:

```typescript
import { quickCommand } from "./commands/quick";
// ...
program.addCommand(quickCommand);
```

- [ ] **Step 3: Verify**

Run: `cd apps/cli && bun run src/index.ts quick "Convention: always use Effect for errors"`
Expected: "Created <id>" with URL.

Run: `echo "Some content" | bun run src/index.ts quick --title "Piped note"`
Expected: "Created <id>" with content from stdin.

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/commands/quick.ts apps/cli/src/index.ts
git commit -m "feat(cli): add quick command for one-liner chunk creation with pipe support"
```

---

## Final Verification

### Task 15: Integration Smoke Test

- [ ] **Step 1: Run type checks**

Run: `pnpm run check-types`
Expected: No type errors.

- [ ] **Step 2: Run tests**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 3: Run lint**

Run: `pnpm lint`
Expected: No lint errors.

- [ ] **Step 4: Manual smoke test**

1. Trigger age scan: `POST /api/chunks/stale/scan-age`
2. Check dashboard for Attention Needed widget
3. Check nav badge shows count
4. Navigate to a stale chunk, verify banner
5. Open Cmd+K, type a title, verify "Quick note" action
6. Press Shift+N with clipboard content, verify pre-fill
7. Open graph, click a node, verify focus mode dims distant nodes
8. Press Escape, verify full graph restored
9. Save a filter preset, reload, verify it persists
10. Open a chunk detail, verify dependency tree and suggestions

- [ ] **Step 5: Final commit if any fixes needed**
