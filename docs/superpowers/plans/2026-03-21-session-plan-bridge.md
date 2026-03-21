# Session → Plan Bridge Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bridge MCP implementation sessions to the plans system so AI agents can link sessions to plans, and step completion is tracked across both.

**Architecture:** Add optional `planId` to implementation sessions. When `begin_implementation` is called with a `planId`, the session links to that plan. When `record_chunk_reference` is called and the chunk matches a plan step's `chunkId`, auto-mark that step as done. Add MCP tools for plan-step operations within a session context.

**Tech Stack:** MCP server (Zod), Elysia API, Effect, Drizzle

---

## File Structure

### Files to modify:
- `packages/db/src/schema/implementation-session.ts` — Add `planId` FK to session
- `packages/api/src/sessions/service.ts` — Link session to plan, auto-complete steps
- `packages/mcp/src/session-tools.ts` — Add planId to begin_implementation, add step-tracking tools

---

## Task 1: Add planId to Implementation Sessions

**Files:**
- Modify: `packages/db/src/schema/implementation-session.ts`

- [ ] **Step 1: Read the session schema**

Read `packages/db/src/schema/implementation-session.ts` to understand the current table definition.

- [ ] **Step 2: Add planId column**

Add to the `implementationSession` table:
```ts
planId: text("plan_id").references(() => plan.id, { onDelete: "set null" }),
```

Import `plan` from `"./plan"`. Add an index on `planId`.

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: add planId FK to implementation sessions"
```

---

## Task 2: Session-Plan Integration in Service

**Files:**
- Modify: `packages/api/src/sessions/service.ts`

- [ ] **Step 1: Read the sessions service**

Read `packages/api/src/sessions/service.ts` — understand `createSession` and `completeSession`.

- [ ] **Step 2: Accept planId in createSession**

When `planId` is provided:
1. Validate the plan exists and belongs to the user
2. Set the plan status to "active" if it's "draft"
3. Store `planId` on the session record
4. Include plan steps in the context bundle returned to the AI

```ts
// In createSession, after creating the session:
if (body.planId) {
    const planDetail = yield* getPlanDetail(body.planId, userId);
    // Auto-activate draft plans
    if (planDetail.status === "draft") {
        yield* updatePlan(body.planId, userId, { status: "active" });
    }
    // Include plan steps in context
    context.planSteps = planDetail.steps;
}
```

- [ ] **Step 3: Auto-complete plan steps on chunk reference**

In the session service, when `recordChunkReference` is called:
1. Check if the session has a `planId`
2. If so, find any plan step whose `chunkId` matches the referenced chunk
3. Auto-mark that step as "done"

```ts
// After recording the chunk reference:
if (session.planId) {
    const steps = yield* getStepsForPlan(session.planId);
    for (const step of steps) {
        if (step.chunkId === chunkId && step.status !== "done") {
            // NOTE: updateStep requires (planId, stepId, userId, body)
            yield* updateStep(session.planId, step.id, userId, { status: "done", note: `Auto-completed via session ${sessionId}` });
        }
    }
}
```

- [ ] **Step 4: On session complete, check plan progress**

When `completeSession` is called:
1. If session has a `planId`, check if all plan steps are done
2. If so, auto-mark the plan as "completed"

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: bridge implementation sessions to plans with auto-step-completion"
```

---

## Task 3: MCP Session-Plan Tools

**Files:**
- Modify: `packages/mcp/src/session-tools.ts`

- [ ] **Step 1: Add planId to begin_implementation**

Extend the `begin_implementation` tool schema:
```ts
{
    title: z.string(),
    codebaseId: z.string().optional(),
    planId: z.string().optional().describe("Link session to a plan for automatic step tracking"),
}
```

Pass `planId` to the API call. Include plan steps in the returned context.

**Also update** `packages/api/src/sessions/routes.ts` — add `planId: t.Optional(t.String())` to the POST `/sessions` body schema. Without this, Elysia will reject requests with `planId` in the body.

- [ ] **Step 2: Add mark_plan_step tool**

New tool that marks a plan step done within the current session context:

```ts
server.tool(
    "mark_plan_step",
    "Mark a plan step as done (within an active session)",
    {
        sessionId: z.string(),
        stepId: z.string(),
        note: z.string().optional(),
    },
    async ({ sessionId, stepId, note }) => {
        // Get session to find planId
        const session = await apiFetch(`/sessions/${sessionId}`);
        if (!session.planId) return { content: [{ type: "text", text: "Session is not linked to a plan" }] };

        await apiFetch(`/plans/${session.planId}/steps/${stepId}`, {
            method: "PATCH",
            body: JSON.stringify({ status: "done", note }),
        });

        return { content: [{ type: "text", text: `Step marked as done` }] };
    }
);
```

- [ ] **Step 3: Add get_plan_progress tool**

```ts
server.tool(
    "get_plan_progress",
    "Get current plan progress (step statuses) for an active session",
    { sessionId: z.string() },
    async ({ sessionId }) => {
        const session = await apiFetch(`/sessions/${sessionId}`);
        if (!session.planId) return { content: [{ type: "text", text: "Session is not linked to a plan" }] };

        const plan = await apiFetch(`/plans/${session.planId}`);
        const summary = plan.steps.map((s: any) =>
            `${s.status === "done" ? "✓" : "○"} ${s.description}`
        ).join("\n");

        // NOTE: plan.progress is NOT a number — getPlanDetail returns { doneCount, totalSteps } as separate fields
        // Compute percentage manually:
        const pct = plan.steps?.length > 0
            ? Math.round((plan.steps.filter((s: any) => s.status === "done").length / plan.steps.length) * 100)
            : 0;

        return { content: [{ type: "text", text: `Plan: ${plan.title}\nProgress: ${pct}%\n\n${summary}` }] };
    }
);
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mcp): add planId to begin_implementation and plan step tracking tools"
```
