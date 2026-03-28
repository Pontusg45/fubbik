# Plan & Task Expansions Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add recurring tasks, plan branching, and auto-generated plan retrospectives.

**Architecture:** Recurring tasks add a `recurringInterval` to plans. Plan branching creates a new plan linked to a parent. Retrospectives are chunks auto-generated on plan completion.

**Tech Stack:** Elysia, Effect, Drizzle, React

---

## Task 1: Recurring Tasks (#4)

**Files:**
- Modify: `packages/db/src/schema/plan.ts` — add `recurringInterval` column
- Modify: `packages/api/src/plans/service.ts` — recreate task on completion
- Modify: `packages/api/src/tasks/routes.ts` — accept `recurring` param
- Modify: `apps/cli/src/commands/task.ts` — add `--recurring` flag

- [ ] **Step 1:** Add `recurringInterval: text("recurring_interval")` to the `plan` table (values: "daily", "weekly", "monthly", null). Push schema.

- [ ] **Step 2:** In `updatePlan` service, when a plan with `recurringInterval` is set to "completed", auto-create a new plan with the same title/description/steps (all reset to "pending") and status "active".

- [ ] **Step 3:** In task routes, accept `recurring` in POST body. Pass to plan creation.

- [ ] **Step 4:** In CLI `task add`, add `--recurring <interval>` option (daily/weekly/monthly).

- [ ] **Step 5:** Commit.

---

## Task 2: Plan Branching (#5)

**Files:**
- Modify: `packages/db/src/schema/plan.ts` — add `parentPlanId` column
- Modify: `packages/api/src/plans/service.ts` — add `branchPlan` function
- Modify: `packages/api/src/plans/routes.ts` — add `POST /plans/:id/branch`
- Modify: `apps/web/src/routes/plans.$planId.tsx` — add "Branch from here" button

- [ ] **Step 1:** Add `parentPlanId: text("parent_plan_id").references(() => plan.id, { onDelete: "set null" })` to the `plan` table. Push schema.

- [ ] **Step 2:** Create `branchPlan` service function: copies the plan with all steps from a given step onward, links to parent, sets new title with "(branch)" suffix.

- [ ] **Step 3:** Add `POST /plans/:id/branch` route accepting `{ fromStepOrder: number, title?: string }`.

- [ ] **Step 4:** On plan detail page, add a "Branch from here" button on each step that creates a branch starting from that step.

- [ ] **Step 5:** Show "Branched from: [parent plan link]" on the branched plan's header.

- [ ] **Step 6:** Commit.

---

## Task 3: Plan Retrospective (#6)

**Files:**
- Create: `packages/api/src/plans/retrospective.ts` — generate retro content
- Modify: `packages/api/src/plans/service.ts` — trigger on plan completion
- Modify: `apps/web/src/routes/plans.$planId.tsx` — show retro link

- [ ] **Step 1:** Create `generateRetrospective(plan)` that produces a markdown summary:
- Title: "Retrospective: {plan title}"
- Sections: Summary (total steps, done/skipped/blocked counts), Timeline (first step started → last step completed), Steps Added During Execution (steps not in original), Assumptions (from linked sessions), Lessons Learned (placeholder for manual input)

- [ ] **Step 2:** In `updatePlan`, when status changes to "completed", auto-generate a retrospective chunk with `originSource: "plan"`, `originRef: planId`, tagged "retrospective".

- [ ] **Step 3:** On plan detail, when status is "completed", show a link to the retrospective chunk.

- [ ] **Step 4:** Commit.
