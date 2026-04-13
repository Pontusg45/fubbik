# Dashboard Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the cluttered 12-widget dashboard with a Focus Stream layout: compact stats bar, active plan card, and unified chronological feed.

**Architecture:** Three new component files (`stats-bar`, `active-plan-card`, `unified-feed`) composed in a rewritten `dashboard.tsx`. Six old widget files deleted. No backend changes.

**Tech Stack:** React, TanStack Query, TanStack Router, Tailwind CSS, shadcn-ui on base-ui

**Spec:** `docs/superpowers/specs/2026-04-13-dashboard-redesign-design.md`

---

## File Structure

### Created

| Path | Responsibility |
|---|---|
| `apps/web/src/features/dashboard/stats-bar.tsx` | Compact inline stats row |
| `apps/web/src/features/dashboard/active-plan-card.tsx` | Plan focus card with interactive task checklist |
| `apps/web/src/features/dashboard/unified-feed.tsx` | Merged feed with filter tabs + feed item rendering |

### Rewritten

| Path | Change |
|---|---|
| `apps/web/src/routes/dashboard.tsx` | Complete rewrite — compose the 3 new components |

### Deleted

| Path |
|---|
| `apps/web/src/features/dashboard/featured-chunk-widget.tsx` |
| `apps/web/src/features/dashboard/smart-collections.tsx` |
| `apps/web/src/features/dashboard/missed-chunks-widget.tsx` |
| `apps/web/src/features/dashboard/milestone-cards.tsx` |
| `apps/web/src/features/dashboard/welcome-wizard.tsx` |
| `apps/web/src/features/dashboard/attention-needed.tsx` |

---

### Task 1: Stats Bar + Active Plan Card

**Files:**
- Create: `apps/web/src/features/dashboard/stats-bar.tsx`
- Create: `apps/web/src/features/dashboard/active-plan-card.tsx`

**Context:** These are self-contained components with their own queries. The dashboard route will import and render them in Task 3.

- [ ] **Step 1: Read reference files**

Read to confirm API patterns and types:
- `apps/web/src/routes/dashboard.tsx` lines 1-40 (imports, route declaration)
- `apps/web/src/routes/dashboard.tsx` — find the stats query (`api.api.stats.get`) and the plans query
- `apps/web/src/features/plans/plan-status-pill.tsx` — for reuse
- `apps/web/src/utils/api.ts` and `apps/web/src/utils/eden.ts` — confirm `api` and `unwrapEden`

- [ ] **Step 2: Create `apps/web/src/features/dashboard/stats-bar.tsx`**

```typescript
import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function StatsBar() {
    const statsQuery = useQuery({
        queryKey: ["stats"],
        queryFn: async () => unwrapEden(await api.api.stats.get()),
    });

    const proposalCountQuery = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await (api.api as any).proposals.count.get()),
    });

    const staleCountQuery = useQuery({
        queryKey: ["stale-count"],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.get({ query: { limit: "0" } })),
    });

    const stats = statsQuery.data as any;
    const proposalCount = (proposalCountQuery.data as any)?.pending ?? 0;
    // stale count: the API may return an array or a count object — adapt after reading the actual response
    const staleCount = Array.isArray(staleCountQuery.data) ? (staleCountQuery.data as any[]).length : 0;

    if (!stats) return null;

    return (
        <div className="flex flex-wrap gap-x-6 gap-y-1 border-b px-1 py-3 text-xs text-muted-foreground">
            <span><strong className="text-foreground text-sm">{stats.chunks ?? 0}</strong> chunks</span>
            <span><strong className="text-foreground text-sm">{stats.connections ?? 0}</strong> connections</span>
            <span><strong className="text-foreground text-sm">{stats.requirements ?? 0}</strong> requirements</span>
            {proposalCount > 0 && (
                <span className="ml-auto">
                    <strong className="text-amber-500 text-sm">{proposalCount}</strong>
                    <span className="text-amber-500/70"> pending proposals</span>
                </span>
            )}
            {staleCount > 0 && (
                <span>
                    <strong className="text-amber-500 text-sm">{staleCount}</strong>
                    <span className="text-amber-500/70"> stale</span>
                </span>
            )}
        </div>
    );
}
```

**Adaptation notes:**
- The `stats` response shape may differ — read the actual `GET /api/stats` response. It might have `chunks`, `connections`, `tags` but NOT `requirements`. If so, add a separate requirements count query via `api.api.requirements.get` or similar.
- The stale count endpoint may return `{ count: N }` instead of an array. The existing `GET /api/chunks/stale/count` endpoint (used by the nav badge) returns `{ count: N }`. Use that instead if it exists: `api.api.chunks.stale.count.get()`. Check what's available.

- [ ] **Step 3: Create `apps/web/src/features/dashboard/active-plan-card.tsx`**

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { PlanStatusPill, type PlanStatusValue } from "@/features/plans/plan-status-pill";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const MAX_VISIBLE_TASKS = 8;

export function ActivePlanCard() {
    const queryClient = useQueryClient();

    const planQuery = useQuery({
        queryKey: ["active-plan-dashboard"],
        queryFn: async () => {
            // Try in_progress first
            const inProgress = unwrapEden(
                await api.api.plans.get({ query: { status: "in_progress" } as any }),
            ) as any[];
            if (inProgress.length > 0) return inProgress[0];

            // Fall back to ready
            const ready = unwrapEden(
                await api.api.plans.get({ query: { status: "ready" } as any }),
            ) as any[];
            if (ready.length > 0) return ready[0];

            return null;
        },
    });

    const plan = planQuery.data as any;

    // Fetch plan detail for tasks
    const detailQuery = useQuery({
        queryKey: ["plan-detail", plan?.id],
        queryFn: async () => unwrapEden(await (api.api as any).plans[plan.id].get()),
        enabled: !!plan?.id,
    });

    const detail = detailQuery.data as any;
    const tasks = detail?.tasks ?? [];
    const doneCount = tasks.filter((t: any) => t.status === "done").length;

    const toggleTask = useMutation({
        mutationFn: async ({ taskId, currentStatus }: { taskId: string; currentStatus: string }) => {
            const newStatus = currentStatus === "done" ? "pending" : "done";
            return unwrapEden(
                await (api.api as any).plans[plan.id].tasks[taskId].patch({ status: newStatus }),
            );
        },
        onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: ["plan-detail", plan?.id] });
        },
    });

    if (planQuery.isLoading) return null;

    if (!plan) {
        return (
            <div className="py-4 text-center text-sm text-muted-foreground">
                No active plan —{" "}
                <Link to="/plans/new" className="text-foreground underline">
                    Start one →
                </Link>
            </div>
        );
    }

    const visibleTasks = tasks.slice(0, MAX_VISIBLE_TASKS);
    const hiddenCount = tasks.length - visibleTasks.length;
    const progressPct = tasks.length === 0 ? 0 : Math.round((doneCount / tasks.length) * 100);

    return (
        <div className="rounded-xl border border-indigo-500/15 bg-indigo-500/[0.03] p-5">
            <div className="mb-3 flex items-center justify-between">
                <div>
                    <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-indigo-400/70">
                        Active Plan
                    </div>
                    <Link
                        to="/plans/$planId"
                        params={{ planId: plan.id }}
                        className="text-base font-semibold hover:underline"
                    >
                        {plan.title}
                    </Link>
                </div>
                <div className="flex items-center gap-2">
                    <span className="font-mono text-xs text-muted-foreground">
                        {doneCount}/{tasks.length}
                    </span>
                    <div className="h-1 w-20 overflow-hidden rounded bg-muted">
                        <div
                            className="h-full rounded bg-emerald-500 transition-all"
                            style={{ width: `${progressPct}%` }}
                        />
                    </div>
                </div>
            </div>
            {tasks.length > 0 && (
                <div className="space-y-1.5">
                    {visibleTasks.map((task: any) => (
                        <button
                            key={task.id}
                            type="button"
                            onClick={() =>
                                toggleTask.mutate({ taskId: task.id, currentStatus: task.status })
                            }
                            className="flex w-full items-center gap-2 text-left text-xs"
                        >
                            <span
                                className={
                                    task.status === "done"
                                        ? "text-emerald-500"
                                        : task.status === "in_progress"
                                          ? "text-blue-400"
                                          : task.status === "blocked"
                                            ? "text-amber-500"
                                            : "text-muted-foreground/40"
                                }
                            >
                                {task.status === "done"
                                    ? "✓"
                                    : task.status === "in_progress"
                                      ? "→"
                                      : task.status === "blocked"
                                        ? "✗"
                                        : "○"}
                            </span>
                            <span
                                className={
                                    task.status === "done"
                                        ? "text-muted-foreground line-through"
                                        : "text-foreground/80"
                                }
                            >
                                {task.title}
                            </span>
                        </button>
                    ))}
                    {hiddenCount > 0 && (
                        <Link
                            to="/plans/$planId"
                            params={{ planId: plan.id }}
                            className="block text-xs text-muted-foreground hover:underline"
                        >
                            and {hiddenCount} more…
                        </Link>
                    )}
                </div>
            )}
            {tasks.length === 0 && (
                <Link
                    to="/plans/$planId"
                    params={{ planId: plan.id }}
                    className="text-xs text-muted-foreground hover:underline"
                >
                    + Add tasks
                </Link>
            )}
        </div>
    );
}
```

- [ ] **Step 4: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "stats-bar|active-plan-card" | head -10`

Expected: zero errors in the new files.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/dashboard/stats-bar.tsx apps/web/src/features/dashboard/active-plan-card.tsx
git commit -m "feat(web): add StatsBar and ActivePlanCard dashboard components

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Unified Feed

**Files:**
- Create: `apps/web/src/features/dashboard/unified-feed.tsx`

**Context:** The feed merges three data sources (proposals, stale flags, activity) into one chronological list. Feed items have type-specific rendering with inline actions for proposals.

- [ ] **Step 1: Read reference patterns**

Read to confirm response shapes:
- `apps/web/src/features/proposals/proposal-card.tsx` — `Proposal` type
- `apps/web/src/routes/dashboard.tsx` — find the activity query and the Activity type shape
- `apps/web/src/features/dashboard/attention-needed.tsx` — stale flag shape

- [ ] **Step 2: Create `apps/web/src/features/dashboard/unified-feed.tsx`**

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type FilterType = "all" | "proposals" | "stale" | "activity";

interface FeedItem {
    id: string;
    type: "proposal" | "stale" | "activity";
    title: string;
    subtitle: string | null;
    timestamp: Date;
    entityId?: string;
    entityType?: string;
    // Proposal-specific
    proposalId?: string;
    chunkId?: string;
    // Activity-specific
    action?: string;
}

export function UnifiedFeed() {
    const [filter, setFilter] = useState<FilterType>("all");
    const queryClient = useQueryClient();

    // Proposals
    const proposalsQuery = useQuery({
        queryKey: ["proposals", "pending"],
        queryFn: async () =>
            unwrapEden(await (api.api as any).proposals.get({ query: { status: "pending" } })),
    });

    // Stale flags
    const staleQuery = useQuery({
        queryKey: ["stale-dashboard"],
        queryFn: async () =>
            unwrapEden(await api.api.chunks.stale.get({ query: { limit: "10" } })),
    });

    // Activity
    const activityQuery = useQuery({
        queryKey: ["activity-dashboard"],
        queryFn: async () =>
            unwrapEden(await api.api.activity.get({ query: { limit: "20" } as any })),
    });

    // Approve/reject mutations
    const approveMutation = useMutation({
        mutationFn: async (proposalId: string) =>
            unwrapEden(await (api.api as any).proposals[proposalId].approve.post({})),
        onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: ["proposals"] });
            void queryClient.invalidateQueries({ queryKey: ["proposals-count"] });
        },
    });

    const rejectMutation = useMutation({
        mutationFn: async (proposalId: string) =>
            unwrapEden(await (api.api as any).proposals[proposalId].reject.post({})),
        onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: ["proposals"] });
            void queryClient.invalidateQueries({ queryKey: ["proposals-count"] });
        },
    });

    // Merge all sources into one feed
    const feedItems: FeedItem[] = [];

    // Add proposals
    const proposals = (proposalsQuery.data ?? []) as any[];
    for (const p of proposals) {
        feedItems.push({
            id: `proposal-${p.id}`,
            type: "proposal",
            title: p.chunkTitle ?? p.chunkId?.slice(0, 8) ?? "Unknown chunk",
            subtitle: p.reason,
            timestamp: new Date(p.createdAt),
            proposalId: p.id,
            chunkId: p.chunkId,
        });
    }

    // Add stale flags
    const staleFlags = (Array.isArray(staleQuery.data) ? staleQuery.data : []) as any[];
    for (const f of staleFlags) {
        feedItems.push({
            id: `stale-${f.id}`,
            type: "stale",
            title: f.chunkTitle ?? f.chunkId?.slice(0, 8) ?? "Unknown chunk",
            subtitle: f.detail ?? f.reason,
            timestamp: new Date(f.detectedAt),
            chunkId: f.chunkId,
        });
    }

    // Add activity
    const activityData = activityQuery.data as any;
    const activities = (Array.isArray(activityData) ? activityData : activityData?.activities ?? []) as any[];
    for (const a of activities) {
        feedItems.push({
            id: `activity-${a.id}`,
            type: "activity",
            title: a.entityTitle ?? a.entityId?.slice(0, 8) ?? "Unknown",
            subtitle: `${a.entityType}${a.entityType && a.action ? " · " : ""}${a.action ?? ""}`,
            timestamp: new Date(a.createdAt),
            entityId: a.entityId,
            entityType: a.entityType,
            action: a.action,
        });
    }

    // Sort by timestamp descending
    feedItems.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());

    // Apply filter
    const filtered =
        filter === "all"
            ? feedItems
            : feedItems.filter(item => {
                  if (filter === "proposals") return item.type === "proposal";
                  if (filter === "stale") return item.type === "stale";
                  if (filter === "activity") return item.type === "activity";
                  return true;
              });

    const FILTERS: { key: FilterType; label: string }[] = [
        { key: "all", label: "All" },
        { key: "proposals", label: "Proposals" },
        { key: "stale", label: "Stale" },
        { key: "activity", label: "Activity" },
    ];

    const isLoading = proposalsQuery.isLoading && staleQuery.isLoading && activityQuery.isLoading;

    return (
        <div>
            {/* Filter tabs */}
            <div className="mb-2 flex items-center justify-between">
                <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    Feed
                </div>
                <div className="flex gap-1">
                    {FILTERS.map(f => (
                        <button
                            key={f.key}
                            type="button"
                            onClick={() => setFilter(f.key)}
                            className={`rounded px-2 py-0.5 text-[10px] transition-colors ${
                                filter === f.key
                                    ? "bg-muted text-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            {f.label}
                        </button>
                    ))}
                </div>
            </div>

            {/* Feed items */}
            {isLoading ? (
                <div className="py-8 text-center text-xs text-muted-foreground">Loading…</div>
            ) : filtered.length === 0 ? (
                <div className="py-8 text-center text-sm text-muted-foreground">
                    Nothing happening yet.{" "}
                    <Link to="/chunks/new" className="underline">
                        Create a chunk
                    </Link>{" "}
                    or{" "}
                    <Link to="/plans/new" className="underline">
                        start a plan
                    </Link>{" "}
                    to get started.
                </div>
            ) : (
                <div className="divide-y divide-border/50">
                    {filtered.map(item => (
                        <div key={item.id} className="flex gap-3 py-3">
                            {/* Dot */}
                            <div
                                className={`mt-1.5 size-1.5 shrink-0 rounded-full ${
                                    item.type === "proposal"
                                        ? "bg-amber-500"
                                        : item.type === "stale"
                                          ? "bg-amber-500/50"
                                          : "bg-muted-foreground/30"
                                }`}
                            />
                            <div className="flex-1">
                                {/* Badge + time */}
                                <div className="mb-1 flex items-center gap-1.5">
                                    <span
                                        className={`rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase ${
                                            item.type === "proposal"
                                                ? "bg-amber-500/12 text-amber-500"
                                                : item.type === "stale"
                                                  ? "bg-amber-500/8 text-amber-500/70"
                                                  : "bg-muted text-muted-foreground"
                                        }`}
                                    >
                                        {item.type === "proposal"
                                            ? "Proposal"
                                            : item.type === "stale"
                                              ? "Stale"
                                              : item.action ?? "Activity"}
                                    </span>
                                    <span className="text-[10px] text-muted-foreground">
                                        {formatRelativeTime(item.timestamp)}
                                    </span>
                                </div>
                                {/* Title */}
                                {item.chunkId ? (
                                    <Link
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: item.chunkId }}
                                        className="text-[13px] font-medium hover:underline"
                                    >
                                        {item.type === "proposal" && "AI proposed changes to "}
                                        <strong>{item.title}</strong>
                                    </Link>
                                ) : (
                                    <div className="text-[13px]">{item.title}</div>
                                )}
                                {/* Subtitle */}
                                {item.subtitle && (
                                    <div className="mt-0.5 text-[11px] text-muted-foreground">
                                        {item.subtitle}
                                    </div>
                                )}
                                {/* Proposal actions */}
                                {item.type === "proposal" && item.proposalId && (
                                    <div className="mt-2 flex gap-2">
                                        <Button
                                            size="sm"
                                            variant="outline"
                                            className="h-6 border-emerald-500/30 bg-emerald-500/8 px-3 text-[10px] font-semibold text-emerald-500 hover:bg-emerald-500/15"
                                            onClick={() => approveMutation.mutate(item.proposalId!)}
                                            disabled={approveMutation.isPending}
                                        >
                                            Approve
                                        </Button>
                                        <Button
                                            size="sm"
                                            variant="ghost"
                                            className="h-6 px-3 text-[10px] text-red-500/70 hover:text-red-500"
                                            onClick={() => rejectMutation.mutate(item.proposalId!)}
                                            disabled={rejectMutation.isPending}
                                        >
                                            Reject
                                        </Button>
                                    </div>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}

function formatRelativeTime(date: Date): string {
    const diff = Date.now() - date.getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return "just now";
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}
```

**Adaptation notes:**
- The activity response shape may be `{ activities: [...] }` or a flat array — the code handles both.
- The stale flags response may be an array or `{ flags: [...] }` — handle both.
- `Button` component — check if it accepts `className` for custom styling. If not, use a plain `<button>` with Tailwind classes.
- The `Link to="/chunks/$chunkId"` pattern needs the `chunkId` param. For activity items that link to plans or requirements, the link path differs by `entityType`. For v1, only link chunk types.

- [ ] **Step 3: Type check**

Run: `pnpm --filter web run check-types 2>&1 | grep "unified-feed" | head -10`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/dashboard/unified-feed.tsx
git commit -m "feat(web): add UnifiedFeed dashboard component with filter tabs + inline proposal actions

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Rewrite Dashboard Route + Delete Old Widgets

**Files:**
- Rewrite: `apps/web/src/routes/dashboard.tsx`
- Delete: 6 old widget files

- [ ] **Step 1: Delete old widget files**

```bash
rm -f apps/web/src/features/dashboard/featured-chunk-widget.tsx \
      apps/web/src/features/dashboard/smart-collections.tsx \
      apps/web/src/features/dashboard/missed-chunks-widget.tsx \
      apps/web/src/features/dashboard/milestone-cards.tsx \
      apps/web/src/features/dashboard/welcome-wizard.tsx \
      apps/web/src/features/dashboard/attention-needed.tsx
```

Some of these may not exist (they may have been moved or renamed). The `rm -f` won't error on missing files.

- [ ] **Step 2: Read the current dashboard.tsx**

Read `apps/web/src/routes/dashboard.tsx` fully. Note:
- The route declaration pattern (`createFileRoute`, `beforeLoad`)
- Any logic we need to preserve (e.g., codebase filtering, session check)
- The `getUser` or session helper import

- [ ] **Step 3: Rewrite `apps/web/src/routes/dashboard.tsx`**

```typescript
import { createFileRoute } from "@tanstack/react-router";

import { PageContainer } from "@/components/ui/page";
import { ActivePlanCard } from "@/features/dashboard/active-plan-card";
import { StatsBar } from "@/features/dashboard/stats-bar";
import { UnifiedFeed } from "@/features/dashboard/unified-feed";
import { getUser } from "@/functions/get-user";

export const Route = createFileRoute("/dashboard")({
    component: DashboardPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // continue without session
        }
        return { session };
    },
});

function DashboardPage() {
    return (
        <PageContainer>
            <div className="mx-auto max-w-3xl space-y-6 py-4">
                <StatsBar />
                <ActivePlanCard />
                <UnifiedFeed />
            </div>
        </PageContainer>
    );
}
```

That's it — ~30 lines. The three components handle their own data fetching.

**Adaptation:** If `PageContainer` expects specific props (like `maxWidth`), pass them. If `getUser` is imported differently, match the current pattern. If the `beforeLoad` isn't needed (dev mode skips auth), keep it anyway for production parity.

- [ ] **Step 4: Check for any remaining imports of deleted files**

```bash
grep -rn "featured-chunk-widget\|smart-collections\|missed-chunks\|milestone-cards\|welcome-wizard\|attention-needed" apps/web/src --include="*.tsx" --include="*.ts" | head -10
```

Remove any lingering imports in other files.

- [ ] **Step 5: Build**

```bash
pnpm --filter web run build 2>&1 | tail -10
```

Expected: success. The route tree regenerates automatically during build.

- [ ] **Step 6: Verify the new dashboard renders**

Start dev: `pnpm dev`

Navigate to `/dashboard`. Verify:
1. Stats bar shows at top with inline numbers
2. Active plan card shows "Federated chunk search" (from seed) with task checklist
3. Click a task checkbox — toggles done/pending
4. Feed shows interleaved proposals (amber), stale flags (amber lighter), activity (gray)
5. Filter tabs work (click "Proposals" — only proposals shown)
6. Proposal items have Approve/Reject buttons
7. No scroll depth explosion — content fits in ~1 screen for seed data

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/routes/dashboard.tsx apps/web/src/features/dashboard/
git commit -m "feat(web): rewrite dashboard as Focus Stream — stats bar, active plan, unified feed

Replaces 12 widgets / 770 lines with 3 focused zones / ~30 lines in the route.
Deletes: featured-chunk-widget, smart-collections, missed-chunks-widget,
milestone-cards, welcome-wizard, attention-needed.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
