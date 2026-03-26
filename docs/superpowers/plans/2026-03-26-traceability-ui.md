# Traceability UI — Web Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show plan progress on requirement detail, rewrite coverage dashboard as requirement→plan→session traceability, and add assumption→requirement conversion.

**Architecture:** Three UI features. The requirement detail page gets a "Plans" section showing linked plan steps. The coverage page is rewritten to show requirement traceability instead of chunk coverage. The knowledge health page gets a "Convert to requirement" action on assumptions.

**Tech Stack:** React, TanStack Router, TanStack Query, Eden treaty, shadcn-ui

**Depends on:** Plan-requirement bridge (backend plan) must be implemented first — `planStep.requirementId` must exist.

---

## File Structure

### New files:
- `apps/web/src/features/requirements/requirement-plans.tsx` — Plan coverage section for requirement detail
- `packages/api/src/coverage/traceability.ts` — Traceability query service

### Files to modify:
- `apps/web/src/routes/requirements_.$requirementId.tsx` — Add plans section
- `apps/web/src/routes/coverage.tsx` — Rewrite as traceability dashboard
- `packages/api/src/coverage/routes.ts` — Add traceability endpoint
- `packages/db/src/repository/coverage.ts` — Add traceability query
- `apps/web/src/routes/knowledge-health.tsx` — Add "Convert to requirement" on assumptions

---

## Task 1: Plan Progress on Requirement Detail

**Files:**
- Create: `apps/web/src/features/requirements/requirement-plans.tsx`
- Modify: `apps/web/src/routes/requirements_.$requirementId.tsx`

- [ ] **Step 1: Create RequirementPlans component**

A component that queries plan steps linked to a specific requirement:

```tsx
// apps/web/src/features/requirements/requirement-plans.tsx
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { CheckCircle, Circle, Clock, AlertTriangle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface RequirementPlansProps {
    requirementId: string;
}

export function RequirementPlans({ requirementId }: RequirementPlansProps) {
    // Query plans that have steps linked to this requirement
    // This needs a new API endpoint or we can filter from the plans list
    const plansQuery = useQuery({
        queryKey: ["requirement-plans", requirementId],
        queryFn: async () => {
            // Fetch all plans and filter for steps linked to this requirement
            const plans = unwrapEden(await api.api.plans.get({ query: {} }));
            if (!Array.isArray(plans)) return [];

            const relevant: Array<{
                planId: string;
                planTitle: string;
                planStatus: string;
                steps: Array<{ id: string; description: string; status: string }>;
            }> = [];

            for (const plan of plans) {
                // Fetch plan detail to get steps with requirementId
                const detail = unwrapEden(await api.api.plans({ id: plan.id }).get());
                const matchingSteps = (detail?.steps ?? []).filter(
                    (s: any) => s.requirementId === requirementId
                );
                if (matchingSteps.length > 0) {
                    relevant.push({
                        planId: plan.id,
                        planTitle: plan.title,
                        planStatus: plan.status,
                        steps: matchingSteps,
                    });
                }
            }
            return relevant;
        },
        staleTime: 30_000,
    });

    const plans = plansQuery.data ?? [];
    if (plans.length === 0) return null;

    const statusIcon = (status: string) => {
        if (status === "done") return <CheckCircle className="size-3.5 text-green-500" />;
        if (status === "in_progress") return <Clock className="size-3.5 text-blue-500" />;
        if (status === "blocked") return <AlertTriangle className="size-3.5 text-red-500" />;
        return <Circle className="size-3.5 text-muted-foreground" />;
    };

    return (
        <div className="space-y-3">
            <h3 className="text-sm font-medium">Covered by Plans</h3>
            {plans.map(p => (
                <div key={p.planId} className="rounded-md border p-3">
                    <div className="flex items-center gap-2 mb-2">
                        <Link to="/plans/$planId" params={{ planId: p.planId }} className="text-sm font-medium hover:underline">
                            {p.planTitle}
                        </Link>
                        <Badge variant="secondary" size="sm">{p.planStatus}</Badge>
                    </div>
                    <div className="space-y-1">
                        {p.steps.map(s => (
                            <div key={s.id} className="flex items-center gap-2 text-xs text-muted-foreground">
                                {statusIcon(s.status)}
                                <span className={s.status === "done" ? "line-through" : ""}>{s.description}</span>
                            </div>
                        ))}
                    </div>
                </div>
            ))}
        </div>
    );
}
```

**Note:** This N+1 query approach is simple but slow for many plans. A proper solution would be a dedicated API endpoint. For now, this works for typical knowledge bases (<50 plans).

- [ ] **Step 2: Add to requirement detail page**

In `apps/web/src/routes/requirements_.$requirementId.tsx`, import and render `<RequirementPlans requirementId={requirementId} />` in a section after the steps/description area.

Read the file first to find the right placement.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): show plan step coverage on requirement detail page"
```

---

## Task 2: Traceability Dashboard (Coverage Rewrite)

**Files:**
- Create: `packages/api/src/coverage/traceability.ts`
- Modify: `packages/api/src/coverage/routes.ts`
- Modify: `packages/db/src/repository/coverage.ts`
- Modify: `apps/web/src/routes/coverage.tsx`

- [ ] **Step 1: Add traceability query**

In `packages/db/src/repository/coverage.ts`, add a function that builds the full requirement→plan→session traceability:

```ts
export function getTraceabilityMatrix(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            // For each requirement, find:
            // 1. Plan steps linked to it (via planStep.requirementId)
            // 2. Sessions that addressed it (via sessionRequirementRef)
            const reqs = await db.select({
                id: requirement.id,
                title: requirement.title,
                status: requirement.status,
                priority: requirement.priority,
            })
            .from(requirement)
            .where(and(
                eq(requirement.userId, userId),
                ...(codebaseId ? [eq(requirement.codebaseId, codebaseId)] : [])
            ))
            .orderBy(asc(requirement.order));

            // For each requirement, fetch linked plan steps and sessions
            // (This could be optimized into joins but is clearer as separate queries)
            const matrix = [];
            for (const req of reqs) {
                const steps = await db.select({
                    stepId: planStep.id,
                    stepDescription: planStep.description,
                    stepStatus: planStep.status,
                    planId: plan.id,
                    planTitle: plan.title,
                    planStatus: plan.status,
                })
                .from(planStep)
                .innerJoin(plan, eq(planStep.planId, plan.id))
                .where(eq(planStep.requirementId, req.id));

                const sessions = await db.select({
                    sessionId: sessionRequirementRef.sessionId,
                    stepsAddressed: sessionRequirementRef.stepsAddressed,
                    sessionTitle: implementationSession.title,
                    sessionStatus: implementationSession.status,
                })
                .from(sessionRequirementRef)
                .innerJoin(implementationSession, eq(sessionRequirementRef.sessionId, implementationSession.id))
                .where(eq(sessionRequirementRef.requirementId, req.id));

                matrix.push({
                    requirement: req,
                    planSteps: steps,
                    sessions,
                    hasPlan: steps.length > 0,
                    hasSession: sessions.length > 0,
                });
            }
            return matrix;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

Read existing coverage.ts to understand imports and patterns. You'll need to import `planStep`, `plan`, `sessionRequirementRef`, `implementationSession`, `requirement` from their respective schema files.

- [ ] **Step 2: Add API endpoint**

In `packages/api/src/coverage/routes.ts`, add:
```ts
.get("/requirements/traceability", ctx => Effect.runPromise(
    requireSession(ctx).pipe(
        Effect.flatMap(session => getTraceabilityMatrix(session.user.id, ctx.query.codebaseId))
    )
), { query: t.Object({ codebaseId: t.Optional(t.String()) }) })
```

- [ ] **Step 3: Rewrite coverage page**

In `apps/web/src/routes/coverage.tsx`, add a "Traceability" tab or replace the existing content:

Show a table/list where each row is a requirement with columns:
- Requirement title + status badge
- Plan coverage: "Covered by Plan X (2/3 steps done)" or "No plan"
- Session coverage: "Addressed in Session Y" or "Not addressed"
- Gap indicator: red if no plan AND no session

Stats at top: "X requirements covered by plans, Y addressed in sessions, Z gaps"

**IMPORTANT:** The new API endpoint path is `/requirements/traceability` (NOT `/requirements/coverage`). The Eden treaty client call must be `api.api.requirements.traceability.get(...)`, not `api.api.requirements.coverage.get(...)` which hits the existing chunk-coverage endpoint.

Read the existing coverage page to understand the current layout and preserve the chunk coverage view as a secondary tab.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: add requirement traceability dashboard (requirement → plan → session)"
```

---

## Task 3: Assumption → Requirement Conversion

**Files:**
- Modify: `apps/web/src/routes/knowledge-health.tsx`

- [ ] **Step 1: Read the knowledge gaps section**

In `knowledge-health.tsx`, find where assumptions/knowledge gaps are displayed. Each gap has `description`, `frequency`, `session_ids`.

- [ ] **Step 2: Add "Convert to Requirement" button**

Next to each gap, add a button that creates a requirement from the assumption text:

```tsx
<Button
    variant="outline"
    size="sm"
    onClick={async () => {
        await api.api.requirements.post({
            title: gap.description.slice(0, 100),
            description: `Identified as knowledge gap from ${gap.frequency} implementation session(s).\n\nOriginal assumption: ${gap.description}`,
            priority: "should",
            codebaseId: codebaseId ?? undefined,
        });
        toast.success("Requirement created from assumption");
        queryClient.invalidateQueries({ queryKey: ["knowledge-gaps"] });
    }}
>
    <FileText className="mr-1.5 size-3" />
    Create Requirement
</Button>
```

Read the existing requirements POST API to confirm the body shape.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add 'Convert to Requirement' action on knowledge gap assumptions"
```
