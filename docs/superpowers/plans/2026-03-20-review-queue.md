# Chunk Review Queue Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A dedicated review queue page showing all AI-generated chunks in "draft" status, with batch approve/reject/edit actions for efficient knowledge curation.

**Architecture:** New web route `/reviews/queue` using the existing chunk list API with `origin=ai&reviewStatus=draft` filters. No new backend endpoints needed — the existing PATCH `/chunks/:id` with `reviewStatus` and bulk update endpoints already support this. Pure frontend feature.

**Tech Stack:** React, TanStack Router, TanStack Query, shadcn-ui, Eden treaty client

---

## File Structure

### New files:
- `apps/web/src/routes/reviews_.queue.tsx` — Review queue page route
- `apps/web/src/features/reviews/review-queue-item.tsx` — Individual review card with actions

### Files to modify:
- `apps/web/src/routes/__root.tsx` — Add nav link to review queue
- `apps/web/src/features/nav/mobile-nav.tsx` — Add to mobile nav

---

## Task 1: Review Queue Page

**Files:**
- Create: `apps/web/src/routes/reviews_.queue.tsx`

- [ ] **Step 1: Create the route file**

```tsx
// apps/web/src/routes/reviews_.queue.tsx
import { createFileRoute, Link } from "@tanstack/react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { CheckCircle, XCircle, Pencil, ChevronDown, ChevronUp, Bot } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { toast } from "sonner";

// NOTE: File is reviews_.queue.tsx — the underscore breaks out of /reviews layout.
// TanStack Router generates the route ID as "/reviews_/queue" (with underscore).
// Check existing reviews_.$sessionId.tsx for the pattern.
export const Route = createFileRoute("/reviews_/queue")({
    component: ReviewQueuePage,
});

function ReviewQueuePage() {
    const queryClient = useQueryClient();
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const [expandedId, setExpandedId] = useState<string | null>(null);

    // Fetch AI-generated draft chunks
    const draftsQuery = useQuery({
        queryKey: ["review-queue"],
        queryFn: async () =>
            unwrapEden(
                await api.api.chunks.get({
                    query: { origin: "ai", reviewStatus: "draft", limit: "50", sort: "newest" },
                })
            ),
    });

    const chunks = draftsQuery.data?.chunks ?? [];
    const total = draftsQuery.data?.total ?? 0;

    // Approve mutation
    const approveMutation = useMutation({
        mutationFn: async (ids: string[]) => {
            await Promise.all(
                ids.map(id =>
                    api.api.chunks({ id }).patch({ reviewStatus: "approved" })
                )
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["review-queue"] });
            setSelectedIds(new Set());
            toast.success("Chunks approved");
        },
    });

    // Reject (archive) mutation
    const rejectMutation = useMutation({
        mutationFn: async (ids: string[]) => {
            await Promise.all(
                ids.map(id =>
                    // NOTE: Eden treaty types may not support this path cleanly.
                    // Existing code uses: (api.api.chunks as any)[id].archive.post()
                    (api.api.chunks as any)[id].archive.post()
                )
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["review-queue"] });
            setSelectedIds(new Set());
            toast.success("Chunks archived");
        },
    });

    const toggleSelect = (id: string) => {
        setSelectedIds(prev => {
            const next = new Set(prev);
            next.has(id) ? next.delete(id) : next.add(id);
            return next;
        });
    };

    const selectAll = () => {
        if (selectedIds.size === chunks.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(chunks.map(c => c.id)));
        }
    };

    return (
        <div className="container max-w-4xl py-6">
            <div className="mb-6 flex items-center justify-between">
                <div>
                    <h1 className="text-xl font-bold">Review Queue</h1>
                    <p className="text-muted-foreground text-sm">
                        {total} AI-generated chunk{total !== 1 ? "s" : ""} awaiting review
                    </p>
                </div>
                {chunks.length > 0 && (
                    <div className="flex items-center gap-2">
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={selectAll}
                        >
                            {selectedIds.size === chunks.length ? "Deselect all" : "Select all"}
                        </Button>
                    </div>
                )}
            </div>

            {/* Bulk action bar */}
            {selectedIds.size > 0 && (
                <div className="mb-4 flex items-center gap-3 rounded-lg border bg-muted/50 px-4 py-2">
                    <span className="text-sm font-medium">{selectedIds.size} selected</span>
                    <Button
                        size="sm"
                        onClick={() => approveMutation.mutate(Array.from(selectedIds))}
                        disabled={approveMutation.isPending}
                    >
                        <CheckCircle className="mr-1.5 size-3.5" />
                        Approve
                    </Button>
                    <Button
                        size="sm"
                        variant="destructive"
                        onClick={() => rejectMutation.mutate(Array.from(selectedIds))}
                        disabled={rejectMutation.isPending}
                    >
                        <XCircle className="mr-1.5 size-3.5" />
                        Reject
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setSelectedIds(new Set())}>
                        Cancel
                    </Button>
                </div>
            )}

            {/* Chunk list */}
            {draftsQuery.isLoading ? (
                <SkeletonList count={5} />
            ) : chunks.length === 0 ? (
                <Card>
                    <CardPanel className="py-12 text-center">
                        <CheckCircle className="mx-auto mb-3 size-8 text-green-500" />
                        <p className="font-medium">All caught up!</p>
                        <p className="text-muted-foreground text-sm mt-1">No AI-generated chunks need review.</p>
                    </CardPanel>
                </Card>
            ) : (
                <Card>
                    {chunks.map((chunk, i) => (
                        <div key={chunk.id}>
                            {i > 0 && <div className="border-t" />}
                            <CardPanel className="p-4">
                                <div className="flex items-start gap-3">
                                    <Checkbox
                                        checked={selectedIds.has(chunk.id)}
                                        onCheckedChange={() => toggleSelect(chunk.id)}
                                    />
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2">
                                            <Link
                                                to="/chunks/$chunkId"
                                                params={{ chunkId: chunk.id }}
                                                className="truncate text-sm font-medium hover:underline"
                                            >
                                                {chunk.title}
                                            </Link>
                                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                {chunk.type}
                                            </Badge>
                                            <Badge variant="outline" size="sm" className="border-yellow-500/30 bg-yellow-500/10 text-[10px] text-yellow-600">
                                                <Bot className="mr-0.5 size-2.5" /> Draft
                                            </Badge>
                                        </div>

                                        {/* Expandable content preview */}
                                        <button
                                            onClick={() => setExpandedId(expandedId === chunk.id ? null : chunk.id)}
                                            className="text-muted-foreground mt-1 flex items-center gap-1 text-xs hover:text-foreground"
                                        >
                                            {expandedId === chunk.id ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />}
                                            {expandedId === chunk.id ? "Hide preview" : "Show preview"}
                                        </button>
                                        {expandedId === chunk.id && (
                                            <div className="mt-2 rounded-md bg-muted/50 p-3 text-sm whitespace-pre-wrap">
                                                {chunk.content.slice(0, 500)}
                                                {chunk.content.length > 500 && "..."}
                                            </div>
                                        )}
                                    </div>

                                    {/* Per-row actions */}
                                    <div className="flex shrink-0 gap-1">
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            className="size-7 p-0"
                                            onClick={() => approveMutation.mutate([chunk.id])}
                                            title="Approve"
                                        >
                                            <CheckCircle className="size-4 text-green-600" />
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            className="size-7 p-0"
                                            asChild
                                        >
                                            <Link to="/chunks/$chunkId/edit" params={{ chunkId: chunk.id }} title="Edit">
                                                <Pencil className="size-4" />
                                            </Link>
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            className="size-7 p-0"
                                            onClick={() => rejectMutation.mutate([chunk.id])}
                                            title="Reject (archive)"
                                        >
                                            <XCircle className="size-4 text-red-600" />
                                        </Button>
                                    </div>
                                </div>
                            </CardPanel>
                        </div>
                    ))}
                </Card>
            )}
        </div>
    );
}
```

**Important:** Read existing route files to understand the exact pattern for `createFileRoute`, imports, and the Eden treaty API call syntax. The file naming `reviews_.queue.tsx` uses TanStack Router's layout route convention — the `_` avoids nesting under a reviews layout. Verify this matches the project's routing convention.

- [ ] **Step 2: Run route generation**

TanStack Router auto-generates route trees. After creating the file:
```bash
cd apps/web && pnpm dev
```
This should regenerate `routeTree.gen.ts` to include the new route.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/reviews_.queue.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add review queue page for AI-generated chunks"
```

---

## Task 2: Navigation Links

**Files:**
- Modify: `apps/web/src/routes/__root.tsx`
- Modify: `apps/web/src/features/nav/mobile-nav.tsx`

- [ ] **Step 1: Add nav link in root layout**

Read `__root.tsx`. Find where "Reviews" is in the nav. Add "Review Queue" as a sub-link or update the Reviews link to include a badge showing draft count.

Alternatively, add it to the "Manage" dropdown:
```tsx
<DropdownMenuItem render={<Link to="/reviews/queue" />}>
    Review Queue
</DropdownMenuItem>
```

- [ ] **Step 2: Add to mobile nav**

In `mobile-nav.tsx`, add "Review Queue" to the manage section.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx
git commit -m "feat(web): add review queue to navigation"
```
