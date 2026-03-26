# Workflow Automation — Plan Generation + MCP Full Loop

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate plans from requirements with template steps, add requirement-driven plan templates, and create MCP tools for the full requirement→plan→session workflow.

**Architecture:** New API endpoint to generate a plan from selected requirements. Each requirement becomes a group of steps (verify/implement/test/document) linked via `requirementId`. New MCP tools wrap the full workflow. Templates are server-side constants (like plan templates).

**Tech Stack:** Elysia, Effect, MCP (Zod), existing plan and requirement services

**Depends on:** Plan-requirement bridge (backend plan) must be implemented first — `planStep.requirementId` must exist.

---

## File Structure

### New files:
- `packages/api/src/plans/generate-from-requirements.ts` — Service for requirement-to-plan generation
- `packages/mcp/src/requirement-tools.ts` — MCP tools for requirement operations

### Files to modify:
- `packages/api/src/plans/routes.ts` — Add plan generation endpoint
- `packages/api/src/plans/service.ts` — Add requirement-driven template logic
- `packages/mcp/src/index.ts` — Register requirement tools
- `packages/mcp/src/plan-tools.ts` — Add `create_plan_from_requirements` tool

---

## Task 1: Plan Generation from Requirements

**Files:**
- Create: `packages/api/src/plans/generate-from-requirements.ts`
- Modify: `packages/api/src/plans/routes.ts`

- [ ] **Step 1: Create generation service**

```ts
// packages/api/src/plans/generate-from-requirements.ts
import { Effect } from "effect";
import { NotFoundError, ValidationError } from "../errors";

interface RequirementInput {
    id: string;
    title: string;
    steps?: Array<{ keyword: string; text: string }>;
}

interface GeneratedStep {
    description: string;
    requirementId: string;
    order: number;
}

export function generatePlanFromRequirements(params: {
    title: string;
    description?: string;
    requirements: RequirementInput[];
    template?: "standard" | "detailed";
}) {
    return Effect.gen(function* () {
        if (params.requirements.length === 0) {
            return yield* Effect.fail(new ValidationError({ message: "At least one requirement is required" }));
        }

        const steps: GeneratedStep[] = [];
        let order = 0;

        for (const req of params.requirements) {
            if (params.template === "detailed") {
                // Detailed: verify + implement + test + document per requirement
                steps.push({ description: `Verify: understand requirement "${req.title}"`, requirementId: req.id, order: order++ });
                steps.push({ description: `Implement: ${req.title}`, requirementId: req.id, order: order++ });
                steps.push({ description: `Test: verify "${req.title}" passes`, requirementId: req.id, order: order++ });
                steps.push({ description: `Document: update chunks for "${req.title}"`, requirementId: req.id, order: order++ });
            } else {
                // Standard: one step per requirement, plus BDD sub-steps
                steps.push({ description: `Implement: ${req.title}`, requirementId: req.id, order: order++ });

                // Add BDD steps as sub-items if present
                if (req.steps?.length) {
                    for (const bddStep of req.steps) {
                        steps.push({
                            description: `  ${bddStep.keyword}: ${bddStep.text}`,
                            requirementId: req.id,
                            order: order++,
                        });
                    }
                }

                steps.push({ description: `Verify: ${req.title} passes`, requirementId: req.id, order: order++ });
            }
        }

        return {
            title: params.title,
            description: params.description ?? `Plan generated from ${params.requirements.length} requirement(s)`,
            steps,
        };
    });
}
```

- [ ] **Step 2: Add API endpoint**

In `packages/api/src/plans/routes.ts`, add (BEFORE `/:id` routes):

```ts
.post("/plans/generate-from-requirements", ctx => Effect.runPromise(
    requireSession(ctx).pipe(
        Effect.flatMap(session => {
            return Effect.gen(function* () {
                // Fetch full requirements to get BDD steps
                const reqs = [];
                for (const id of ctx.body.requirementIds) {
                    const req = yield* getRequirementById(id);
                    if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));
                    reqs.push(req);
                }

                const generated = yield* generatePlanFromRequirements({
                    title: ctx.body.title,
                    description: ctx.body.description,
                    requirements: reqs.map(r => ({ id: r.id, title: r.title, steps: r.steps })),
                    template: ctx.body.template,
                });

                // Create the plan with generated steps
                return yield* createPlan(session.user.id, {
                    title: generated.title,
                    description: generated.description,
                    codebaseId: ctx.body.codebaseId,
                    steps: generated.steps.map(s => ({
                        description: s.description,
                        order: s.order,
                        requirementId: s.requirementId,
                    })),
                });
            });
        })
    )
), {
    body: t.Object({
        title: t.String({ maxLength: 500 }),
        description: t.Optional(t.String()),
        requirementIds: t.Array(t.String()),
        codebaseId: t.Optional(t.String()),
        template: t.Optional(t.Union([t.Literal("standard"), t.Literal("detailed")])),
    }),
})
```

Import `generatePlanFromRequirements` and whatever requirement repo function fetches by ID. Read the requirement repo to find the correct function name.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add /plans/generate-from-requirements endpoint"
```

---

## Task 2: Requirement-Driven Plan Templates (#6)

**Files:**
- Modify: `packages/api/src/plans/service.ts`

- [ ] **Step 1: Read the existing PLAN_TEMPLATES**

Find the `PLAN_TEMPLATES` constant in `service.ts` (feature-dev, bug-fix, migration).

- [ ] **Step 2: Add requirement-driven templates**

Add to `PLAN_TEMPLATES`:

```ts
"requirement-standard": {
    title: "Requirement Implementation (Standard)",
    description: "One implementation + verification step per requirement",
    steps: [], // Steps are generated dynamically from requirements
    requirementDriven: true,
},
"requirement-detailed": {
    title: "Requirement Implementation (Detailed)",
    description: "Verify → Implement → Test → Document per requirement",
    steps: [],
    requirementDriven: true,
},
```

When `createPlan` is called with a `requirementDriven` template and `requirementIds`, delegate to `generatePlanFromRequirements`.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add requirement-driven plan templates"
```

---

## Task 3: MCP Full-Loop Tools (#10)

**Files:**
- Create: `packages/mcp/src/requirement-tools.ts`
- Modify: `packages/mcp/src/plan-tools.ts`
- Modify: `packages/mcp/src/index.ts`

- [ ] **Step 1: Create requirement MCP tools**

Read existing MCP tools (`packages/mcp/src/tools.ts` and `plan-tools.ts`) for the exact pattern. **CRITICAL:** All tools must be wrapped in an exported function matching the pattern:

```ts
// packages/mcp/src/requirement-tools.ts
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch } from "./api-client";

export function registerRequirementTools(server: McpServer): void {
    // 1. list_requirements — GET /requirements (with status/priority filters)
    server.tool("list_requirements", "List requirements", { ... }, async (...) => { ... });

    // 2. create_requirement — POST /requirements
    server.tool("create_requirement", "Create a requirement", { ... }, async (...) => { ... });

    // 3. update_requirement_status — PATCH /requirements/:id with { status }
    server.tool("update_requirement_status", "Update requirement status", { ... }, async (...) => { ... });
}
```

Each tool follows the existing `apiFetch` + Zod schema + `{ content: [{ type: "text", text }] }` return pattern.

- [ ] **Step 2: Add create_plan_from_requirements tool**

In `packages/mcp/src/plan-tools.ts`, add:

```ts
server.tool(
    "create_plan_from_requirements",
    "Generate a plan from selected requirements with implementation steps",
    {
        title: z.string().describe("Plan title"),
        requirementIds: z.array(z.string()).describe("Requirement IDs to include"),
        template: z.enum(["standard", "detailed"]).optional().describe("Step template: standard (implement+verify) or detailed (verify+implement+test+document)"),
        codebaseId: z.string().optional(),
    },
    async ({ title, requirementIds, template, codebaseId }) => {
        const plan = await apiFetch("/plans/generate-from-requirements", {
            method: "POST",
            body: JSON.stringify({ title, requirementIds, template, codebaseId }),
        });
        return {
            content: [{
                type: "text",
                text: `Created plan "${plan.title}" with ${plan.steps?.length ?? 0} steps from ${requirementIds.length} requirement(s). Plan ID: ${plan.id}`
            }]
        };
    }
);
```

- [ ] **Step 3: Register requirement tools**

In `packages/mcp/src/index.ts`:
```ts
import { registerRequirementTools } from "./requirement-tools";
registerRequirementTools(server);
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mcp): add requirement tools and create_plan_from_requirements"
```
