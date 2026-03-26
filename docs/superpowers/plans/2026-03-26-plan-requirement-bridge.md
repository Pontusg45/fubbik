# Plan ↔ Requirement Bridge — Backend Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `requirementId` to plan steps and auto-sync requirement status when sessions complete — the foundation for full traceability.

**Architecture:** Add `requirementId` FK to `planStep` table. Extend plan service to accept and validate requirement links on steps. Modify `completeSession` to auto-update requirement statuses for addressed requirements. Extend plan detail API to include requirement info per step.

**Tech Stack:** Drizzle ORM, Effect, Elysia, PostgreSQL

**Codebase state (verified):**
- `planStep` has: id, planId, description, status, order, parentStepId, note, chunkId — NO `requirementId`
- `completeSession` auto-completes plans but does NOT touch requirement status
- `reviewSession` is the only path that updates requirements, and it's manual
- `sessionRequirementRef` tracks `{ sessionId, requirementId, stepsAddressed }` — which steps were addressed but doesn't auto-update status
- Requirement status is a plain text column: "untested" | "passing" | "failing"

---

## File Structure

### Files to modify:
- `packages/db/src/schema/plan.ts` — Add `requirementId` FK to `planStep`
- `packages/api/src/plans/service.ts` — Accept `requirementId` in step CRUD, validate
- `packages/api/src/plans/routes.ts` — Add `requirementId` to step body schemas
- `packages/api/src/sessions/service.ts` — Auto-update requirement status on `completeSession`
- `packages/db/src/repository/requirement.ts` — Expose `updateRequirementStatus` if not already exported

---

## Task 1: Add requirementId to planStep Schema

**Files:**
- Modify: `packages/db/src/schema/plan.ts`

- [ ] **Step 1: Read the plan schema**

Read `packages/db/src/schema/plan.ts`. Find the `planStep` table definition.

- [ ] **Step 2: Add requirementId column**

Import `requirement` from `"./requirement"` and add to `planStep`:
```ts
requirementId: text("requirement_id").references(() => requirement.id, { onDelete: "set null" }),
```

Add an index:
```ts
index("plan_step_requirementId_idx").on(table.requirementId),
```

Add to `planStepRelations`:
```ts
requirement: one(requirement, { fields: [planStep.requirementId], references: [requirement.id] }),
```

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: add requirementId FK to planStep table"
```

---

## Task 2: Extend Plan Service and Routes for requirementId

**Files:**
- Modify: `packages/api/src/plans/service.ts`
- Modify: `packages/api/src/plans/routes.ts`

- [ ] **Step 1: Read both files**

Understand `addStep`, `updateStep`, `createPlan` (which accepts initial steps), and the corresponding route body schemas.

- [ ] **Step 2: Add requirementId to step creation**

In `service.ts`, extend `addStep` to accept `requirementId?: string`. If provided, validate the requirement exists (import from requirement repo). Pass to `createStepRepo`.

In `routes.ts`, add `requirementId: t.Optional(t.String())` to:
- POST `/plans/:id/steps` body
- POST `/plans` body → steps array items
- PATCH `/plans/:id/steps/:stepId` body

- [ ] **Step 3: Include requirement info in plan detail**

In `getPlanDetail`, for each step that has a `requirementId`, include the requirement title and status. Read the steps, then batch-fetch requirement titles:

```ts
// After fetching steps:
const reqIds = steps.filter(s => s.requirementId).map(s => s.requirementId!);
const reqs = reqIds.length > 0 ? yield* getRequirementsByIds(reqIds) : [];
const reqMap = new Map(reqs.map(r => [r.id, { title: r.title, status: r.status }]));

// Enrich steps:
const enrichedSteps = steps.map(s => ({
    ...s,
    requirement: s.requirementId ? reqMap.get(s.requirementId) ?? null : null,
}));
```

**CRITICAL:** `getRequirementsByIds` exists but only selects `id` and `useCaseId` — NOT `title` or `status`. You must either:
(a) Extend the existing function's select to include `requirement.title, requirement.status`, or
(b) Create a new `getRequirementTitlesByIds` function with the correct select.

Also: `getRequirementsByIds` requires a `userId` second argument: `getRequirementsByIds(reqIds, userId)`. Pass the user's ID.

**ALSO CRITICAL:** You must extend `CreateStepParams` in `packages/db/src/repository/plan.ts` to include `requirementId?: string`. Without this, the repo insert will silently ignore requirementId even after the schema has the column. The full chain is: route body → service param → repo `CreateStepParams` → DB insert — all three must include `requirementId`.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: support requirementId on plan steps with validation and enrichment"
```

---

## Task 3: Auto-Sync Requirement Status on Session Complete

**Files:**
- Modify: `packages/api/src/sessions/service.ts`

- [ ] **Step 1: Read completeSession**

Read `packages/api/src/sessions/service.ts`, find `completeSession`. Understand the current flow:
1. Fetches session detail
2. Generates review brief
3. Updates session status to "completed"
4. If planId, checks if all plan steps are done → auto-completes plan

- [ ] **Step 2: Add auto-requirement-status-sync**

After the plan auto-complete check, add requirement status sync.

**CRITICAL notes:**
- Do NOT call `getSessionRequirementRefs` — that function doesn't exist.
- `completeSession` already calls `getSessionDetail(sessionId)` earlier in the function, which returns `detail.requirementRefs`. Use that instead.
- `updateRequirementStatus` takes THREE arguments: `(id, userId, status)` — NOT two.

```ts
// After plan auto-complete (around line 285-291):

// Auto-update requirement statuses for addressed requirements
// detail.requirementRefs is already in scope from the getSessionDetail call above
if (detail.requirementRefs.length > 0) {
    for (const ref of detail.requirementRefs) {
        yield* updateRequirementStatus(ref.requirementId, userId, "passing");
    }
}
```

Import `updateRequirementStatus` from `@fubbik/db/repository`. Its exact signature is `updateRequirementStatus(id: string, userId: string, status: string)`.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: auto-sync requirement status to passing on session completion"
```
