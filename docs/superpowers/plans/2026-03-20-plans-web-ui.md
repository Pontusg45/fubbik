# Plans Feature — Web UI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add web pages for creating, viewing, and managing plans with interactive step checklists.

**Architecture:** Three new routes: `/plans` (list), `/plans/new` (create), `/plans/:id` (detail with step management). Uses Eden treaty client for API calls. Step status updates are inline — no page navigation required. Progress bar shows completion percentage.

**Tech Stack:** React, TanStack Router, TanStack Query, shadcn-ui (base-ui), Tailwind CSS, Eden treaty

**Depends on:** Plans Backend plan must be implemented first (schema + API).

---

## File Structure

### New files:
- `apps/web/src/routes/plans.index.tsx` — Plan list page
- `apps/web/src/routes/plans.new.tsx` — Create plan form
- `apps/web/src/routes/plans.$planId.tsx` — Plan detail with steps
- `apps/web/src/features/plans/plan-step-item.tsx` — Interactive step row component
- `apps/web/src/features/plans/plan-progress-bar.tsx` — Progress bar component

### Files to modify:
- `apps/web/src/routes/__root.tsx` — Add Plans to navigation
- `apps/web/src/features/nav/mobile-nav.tsx` — Add to mobile nav

---

## Task 1: Plan List Page

**Files:**
- Create: `apps/web/src/routes/plans.index.tsx`

- [ ] **Step 1: Create list route**

Read an existing list page (e.g., `apps/web/src/routes/templates.tsx`) for the pattern. Create:

```tsx
// apps/web/src/routes/plans.index.tsx
import { createFileRoute, Link } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { Plus, FileText, CheckCircle2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { Empty, EmptyMedia, EmptyTitle, EmptyDescription, EmptyAction } from "@/components/ui/empty";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";

export const Route = createFileRoute("/plans/")({
    component: PlansListPage,
});

function PlansListPage() {
    const { codebaseId } = useActiveCodebase();

    const plansQuery = useQuery({
        queryKey: ["plans", codebaseId],
        queryFn: async () => {
            const query: Record<string, string> = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.plans.get({ query }));
        },
    });

    const plans = plansQuery.data ?? [];

    return (
        <div className="container max-w-4xl py-6">
            <div className="mb-6 flex items-center justify-between">
                <div>
                    <h1 className="text-xl font-bold">Plans</h1>
                    <p className="text-muted-foreground text-sm">
                        Track implementation progress with structured checklists.
                    </p>
                </div>
                <Button asChild size="sm">
                    <Link to="/plans/new"><Plus className="mr-1.5 size-3.5" /> New Plan</Link>
                </Button>
            </div>

            {plansQuery.isLoading ? (
                <SkeletonList count={4} />
            ) : plans.length === 0 ? (
                <Empty>
                    <EmptyMedia variant="icon"><FileText className="size-10" /></EmptyMedia>
                    <EmptyTitle>No plans yet</EmptyTitle>
                    <EmptyDescription>Create a plan to track implementation steps.</EmptyDescription>
                    <EmptyAction>
                        <Button asChild><Link to="/plans/new">Create Plan</Link></Button>
                    </EmptyAction>
                </Empty>
            ) : (
                <Card>
                    {plans.map((p: any, i: number) => (
                        <div key={p.id}>
                            {i > 0 && <div className="border-t" />}
                            <CardPanel className="p-4 hover:bg-muted/50 transition-colors">
                                <Link to="/plans/$planId" params={{ planId: p.id }} className="flex items-center gap-4">
                                    <div className="min-w-0 flex-1">
                                        <p className="truncate text-sm font-medium">{p.title}</p>
                                        <div className="mt-1 flex items-center gap-2">
                                            <Badge variant={p.status === "completed" ? "default" : "secondary"} size="sm">
                                                {p.status}
                                            </Badge>
                                            <span className="text-muted-foreground text-xs">
                                                {p.doneCount}/{p.stepCount} steps
                                            </span>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        {/* Simple progress indicator */}
                                        <div className="h-2 w-20 rounded-full bg-muted overflow-hidden">
                                            <div
                                                className="h-full rounded-full bg-primary transition-all"
                                                style={{ width: `${p.progress}%` }}
                                            />
                                        </div>
                                        <span className="text-muted-foreground text-xs w-8 text-right">{p.progress}%</span>
                                    </div>
                                </Link>
                            </CardPanel>
                        </div>
                    ))}
                </Card>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Run TanStack Router generation**

Start dev server briefly or run the route generator to update `routeTree.gen.ts`.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/plans.index.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add plans list page"
```

---

## Task 2: Create Plan Form

**Files:**
- Create: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Create form route**

Read `apps/web/src/routes/chunks.new.tsx` for form patterns. The plan create form needs:
- Title input
- Description textarea
- Steps: dynamic list of inputs with drag-to-reorder (or simple add/remove)
- Submit creates plan with steps

```tsx
// Key form state:
const [title, setTitle] = useState("");
const [description, setDescription] = useState("");
const [steps, setSteps] = useState<Array<{ description: string }>>([{ description: "" }]);

// Add step: setSteps(prev => [...prev, { description: "" }])
// Remove step: setSteps(prev => prev.filter((_, i) => i !== index))
// Update step: setSteps(prev => prev.map((s, i) => i === index ? { ...s, description: value } : s))

// Submit:
const createMutation = useMutation({
    mutationFn: async () => {
        return unwrapEden(await api.api.plans.post({
            title,
            description: description || undefined,
            codebaseId: codebaseId || undefined,
            steps: steps.filter(s => s.description.trim()).map((s, i) => ({ description: s.description, order: i })),
        }));
    },
    onSuccess: (data) => {
        navigate({ to: "/plans/$planId", params: { planId: data.id } });
    },
});
```

Each step input should have a "+" button to add below and an "X" to remove. Use a numbered list style.

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/plans.new.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add plan creation form"
```

---

## Task 3: Plan Detail Page with Interactive Steps

**Files:**
- Create: `apps/web/src/features/plans/plan-step-item.tsx`
- Create: `apps/web/src/features/plans/plan-progress-bar.tsx`
- Create: `apps/web/src/routes/plans.$planId.tsx`

- [ ] **Step 1: Create PlanProgressBar**

```tsx
// apps/web/src/features/plans/plan-progress-bar.tsx
interface PlanProgressBarProps {
    progress: number;
    stepCount: number;
    doneCount: number;
}

export function PlanProgressBar({ progress, stepCount, doneCount }: PlanProgressBarProps) {
    return (
        <div className="space-y-1">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>{doneCount} of {stepCount} steps complete</span>
                <span>{progress}%</span>
            </div>
            <div className="h-2 w-full rounded-full bg-muted overflow-hidden">
                <div
                    className="h-full rounded-full bg-primary transition-all duration-300"
                    style={{ width: `${progress}%` }}
                />
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Create PlanStepItem**

```tsx
// apps/web/src/features/plans/plan-step-item.tsx
import { useState } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { ChevronDown, ChevronRight, Link2 } from "lucide-react";

interface PlanStepItemProps {
    step: {
        id: string;
        description: string;
        status: string;
        note: string | null;
        chunkId: string | null;
        order: number;
    };
    onStatusChange: (stepId: string, status: string) => void;
    onNoteChange: (stepId: string, note: string) => void;
    disabled?: boolean;
}

export function PlanStepItem({ step, onStatusChange, onNoteChange, disabled }: PlanStepItemProps) {
    const [expanded, setExpanded] = useState(false);
    const isDone = step.status === "done";
    const isBlocked = step.status === "blocked";

    return (
        <div className={`border-l-2 pl-3 py-2 ${isDone ? "border-green-500/50" : isBlocked ? "border-red-500/50" : "border-muted"}`}>
            <div className="flex items-start gap-2">
                <Checkbox
                    checked={isDone}
                    disabled={disabled}
                    onCheckedChange={(checked) => {
                        onStatusChange(step.id, checked ? "done" : "pending");
                    }}
                    className="mt-0.5"
                />
                <div className="flex-1 min-w-0">
                    <p className={`text-sm ${isDone ? "line-through text-muted-foreground" : ""}`}>
                        {step.description}
                    </p>
                    {step.status !== "done" && step.status !== "pending" && (
                        <Badge variant="outline" size="sm" className="mt-1 text-[10px]">{step.status}</Badge>
                    )}
                    {step.chunkId && (
                        <span className="text-muted-foreground text-xs flex items-center gap-1 mt-1">
                            <Link2 className="size-3" /> Linked to chunk
                        </span>
                    )}
                </div>
                <button onClick={() => setExpanded(!expanded)} className="text-muted-foreground hover:text-foreground">
                    {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                </button>
            </div>
            {expanded && (
                <div className="mt-2 ml-6">
                    <textarea
                        value={step.note ?? ""}
                        onChange={(e) => onNoteChange(step.id, e.target.value)}
                        placeholder="Add a note..."
                        className="w-full rounded-md border bg-transparent px-3 py-2 text-xs resize-none"
                        rows={2}
                    />
                    {/* Status buttons */}
                    <div className="flex gap-1 mt-2">
                        {["pending", "in_progress", "done", "blocked", "skipped"].map(s => (
                            <button
                                key={s}
                                onClick={() => onStatusChange(step.id, s)}
                                className={`rounded px-2 py-0.5 text-[10px] ${
                                    step.status === s ? "bg-primary text-primary-foreground" : "bg-muted hover:bg-muted/80"
                                }`}
                            >
                                {s.replace("_", " ")}
                            </button>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 3: Create plan detail route**

Create `apps/web/src/routes/plans.$planId.tsx` with:
- Plan title, description, status badge
- Progress bar
- Step list with PlanStepItem components
- "Add Step" button at the bottom
- Status change buttons (Activate, Complete, Archive)
- "Add Chunk Reference" button

Read existing detail pages (e.g., `chunks.$chunkId.tsx`) for layout patterns.

The step status/note updates should use `useMutation` calling `api.api.plans({ id: planId }).steps({ stepId }).patch(...)`.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/plans/ apps/web/src/routes/plans.\$planId.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add plan detail page with interactive step checklist"
```

---

## Task 4: Navigation

**Files:**
- Modify: `apps/web/src/routes/__root.tsx`
- Modify: `apps/web/src/features/nav/mobile-nav.tsx`

- [ ] **Step 1: Add Plans to navigation**

In `__root.tsx`, add "Plans" to the main nav (alongside Dashboard, Chunks, Graph, etc.):
```tsx
<Link to="/plans" className="...">Plans</Link>
```

Or add to the Manage dropdown if the main nav is too crowded. Read the file to decide.

In `mobile-nav.tsx`, add to the appropriate items array.

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx
git commit -m "feat(web): add plans to navigation"
```
