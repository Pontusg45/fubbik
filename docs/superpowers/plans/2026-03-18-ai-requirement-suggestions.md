# AI Requirement Suggestions — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MCP tools for AI-driven requirement suggestions with context-aware batch creation and automatic use case resolution.

**Architecture:** New `suggest-context` GET endpoint gathers focused knowledge base context. New `batch` POST endpoint creates multiple requirements with auto use case lookup/creation. Two new MCP tools wrap these endpoints. No schema changes needed.

**Tech Stack:** Elysia, Effect, Drizzle ORM, @modelcontextprotocol/sdk, Zod

---

### Task 1: Suggest context — service and route

Gather focused context from the knowledge base for AI suggestion generation.

**Files:**
- Create: `packages/api/src/requirements/suggest-context-service.ts`
- Modify: `packages/api/src/requirements/routes.ts`

- [ ] **Step 1: Create suggest context service**

Create `packages/api/src/requirements/suggest-context-service.ts`:

```typescript
import {
    listUseCases,
    listRequirementsByUseCase,
    listRequirements,
    getChunkCoverage,
    getOrphanChunks,
    getStaleChunks,
    getThinChunks,
    listChunks
} from "@fubbik/db/repository";
import { Effect } from "effect";

interface SuggestContext {
    useCases: Array<{
        id: string;
        name: string;
        parentId: string | null;
        requirementCount: number;
        requirements: Array<{ id: string; title: string; status: string }>;
    }>;
    coverageGaps: Array<{ id: string; title: string; type: string }>;
    healthIssues: { orphanCount: number; staleCount: number; thinCount: number };
    relevantChunks: Array<{ id: string; title: string; content: string }>;
}

export function getSuggestContext(
    userId: string,
    query: { focus?: string; codebaseId?: string }
) {
    return Effect.gen(function* () {
        // 1. Use cases with requirements
        const useCaseList = yield* listUseCases(userId, query.codebaseId);
        const useCasesWithReqs = [];
        for (const uc of useCaseList) {
            const reqs = yield* listRequirementsByUseCase(uc.id, userId);
            const filtered = query.focus
                ? reqs.filter(r => r.title.toLowerCase().includes(query.focus!.toLowerCase()))
                : reqs;
            if (query.focus && filtered.length === 0 && reqs.length > 0) continue;
            useCasesWithReqs.push({
                id: uc.id,
                name: uc.name,
                parentId: (uc as any).parentId ?? null,
                requirementCount: uc.requirementCount ?? reqs.length,
                requirements: (query.focus ? filtered : reqs).map(r => ({
                    id: r.id,
                    title: r.title,
                    status: r.status
                }))
            });
        }

        // Also include ungrouped requirements
        const allReqs = yield* listRequirements({
            userId,
            codebaseId: query.codebaseId,
            limit: 200,
            offset: 0
        });
        const ungrouped = allReqs.requirements.filter(r => !r.useCaseId);
        const filteredUngrouped = query.focus
            ? ungrouped.filter(r => r.title.toLowerCase().includes(query.focus!.toLowerCase()))
            : ungrouped;
        if (filteredUngrouped.length > 0) {
            useCasesWithReqs.push({
                id: "__ungrouped__",
                name: "Ungrouped",
                parentId: null,
                requirementCount: filteredUngrouped.length,
                requirements: filteredUngrouped.map(r => ({
                    id: r.id,
                    title: r.title,
                    status: r.status
                }))
            });
        }

        // 2. Coverage gaps
        const coverage = yield* getChunkCoverage(userId, query.codebaseId);
        let gaps = coverage.filter((c: any) => Number(c.requirementCount) === 0);
        if (query.focus) {
            gaps = gaps.filter((c: any) =>
                c.title.toLowerCase().includes(query.focus!.toLowerCase())
            );
        }
        const coverageGaps = gaps.slice(0, query.focus ? 20 : 10).map((c: any) => ({
            id: c.id,
            title: c.title,
            type: "uncovered"
        }));

        // 3. Health issues (counts only)
        const [orphans, stale, thin] = yield* Effect.all([
            getOrphanChunks(userId, query.codebaseId),
            getStaleChunks(userId, query.codebaseId),
            getThinChunks(userId, query.codebaseId)
        ]);
        const healthIssues = {
            orphanCount: orphans.count,
            staleCount: stale.count,
            thinCount: thin.count
        };

        // 4. Relevant chunks (only when focus provided)
        let relevantChunks: Array<{ id: string; title: string; content: string }> = [];
        if (query.focus) {
            const chunks = yield* listChunks({
                userId,
                codebaseId: query.codebaseId,
                search: query.focus,
                limit: 20,
                offset: 0
            });
            relevantChunks = chunks.chunks.map((c: any) => ({
                id: c.id,
                title: c.title,
                content: (c.content ?? "").slice(0, 300)
            }));
        }

        return { useCases: useCasesWithReqs, coverageGaps, healthIssues, relevantChunks } as SuggestContext;
    });
}
```

Note: The exact function names and signatures may differ from what's shown. Read the actual repository files (`packages/db/src/repository/chunk.ts`, `packages/db/src/repository/use-case.ts`, etc.) to get the correct names and parameter shapes. The `listChunks` function might be called `listChunksForUser` or similar — check the chunk repository barrel export.

- [ ] **Step 2: Add route to requirements routes**

In `packages/api/src/requirements/routes.ts`, add the endpoint before the `/:id` routes (alongside stats, bulk, reorder):

```typescript
import * as suggestContextService from "./suggest-context-service";

// Suggest context (chain after reorder, before export/list)
.get(
    "/requirements/suggest-context",
    ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                return yield* suggestContextService.getSuggestContext(session.user.id, ctx.query);
            })
        ),
    {
        query: t.Object({
            focus: t.Optional(t.String()),
            codebaseId: t.Optional(t.String())
        })
    }
)
```

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/api/src/requirements/suggest-context-service.ts packages/api/src/requirements/routes.ts
git commit -m "feat: add suggest-context endpoint for AI requirement suggestions"
```

---

### Task 2: Batch creation — service and route

Batch create requirements with automatic use case resolution.

**Files:**
- Create: `packages/api/src/requirements/batch-service.ts`
- Modify: `packages/api/src/requirements/routes.ts`
- Modify: `packages/db/src/repository/use-case.ts` (add `getUseCaseByName`)

- [ ] **Step 1: Add use case lookup by name to repository**

In `packages/db/src/repository/use-case.ts`, add:

```typescript
export function getUseCaseByName(userId: string, name: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(useCase)
                .where(and(eq(useCase.userId, userId), eq(useCase.name, name)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Create batch service**

Create `packages/api/src/requirements/batch-service.ts`:

```typescript
import {
    createRequirement,
    createUseCase,
    getUseCaseByName,
    getUseCaseById
} from "@fubbik/db/repository";
import { Effect } from "effect";
import { StepValidationError } from "../errors";
import { validateSteps } from "./validator";

interface BatchRequirement {
    title: string;
    description?: string;
    steps: Array<{ keyword: "given"|"when"|"then"|"and"|"but"; text: string }>;
    priority?: string;
    useCaseId?: string;
    useCaseName?: string;
    parentUseCaseName?: string;
}

export function batchCreateRequirements(
    userId: string,
    body: { requirements: BatchRequirement[]; codebaseId?: string }
) {
    return Effect.gen(function* () {
        // 1. Validate all steps upfront
        const allErrors: Array<{ requirementIndex: number; errors: Array<{ step: number; error: string }> }> = [];
        for (let i = 0; i < body.requirements.length; i++) {
            const errors = validateSteps(body.requirements[i]!.steps);
            if (errors.length > 0) {
                allErrors.push({ requirementIndex: i, errors });
            }
        }
        if (allErrors.length > 0) {
            return yield* Effect.fail(new StepValidationError({ errors: allErrors as any }));
        }

        // 2. Resolve use cases (cache by name to avoid duplicates)
        const useCaseCache = new Map<string, string>(); // name → id
        const useCasesCreated: Array<{ id: string; name: string; parentId: string | null }> = [];

        async function resolveUseCaseId(req: BatchRequirement): Promise<string | undefined> {
            // If explicit ID provided, use it
            if (req.useCaseId) return req.useCaseId;
            if (!req.useCaseName) return undefined;

            const cacheKey = `${req.parentUseCaseName ?? ""}::${req.useCaseName}`;
            if (useCaseCache.has(cacheKey)) return useCaseCache.get(cacheKey)!;

            // Resolve parent if needed
            let parentId: string | undefined;
            if (req.parentUseCaseName) {
                const parentKey = `::${req.parentUseCaseName}`;
                if (useCaseCache.has(parentKey)) {
                    parentId = useCaseCache.get(parentKey)!;
                } else {
                    // Look up or create parent
                    const existing = yield* getUseCaseByName(userId, req.parentUseCaseName);
                    if (existing) {
                        parentId = existing.id;
                    } else {
                        const id = crypto.randomUUID();
                        yield* createUseCase({
                            id, name: req.parentUseCaseName, userId,
                            codebaseId: body.codebaseId
                        });
                        parentId = id;
                        useCasesCreated.push({ id, name: req.parentUseCaseName, parentId: null });
                    }
                    useCaseCache.set(parentKey, parentId);
                }
            }

            // Look up or create the use case itself
            const existing = yield* getUseCaseByName(userId, req.useCaseName);
            if (existing) {
                useCaseCache.set(cacheKey, existing.id);
                return existing.id;
            }

            const id = crypto.randomUUID();
            yield* createUseCase({
                id, name: req.useCaseName, userId,
                codebaseId: body.codebaseId,
                parentId
            });
            useCasesCreated.push({ id, name: req.useCaseName, parentId: parentId ?? null });
            useCaseCache.set(cacheKey, id);
            return id;
        }

        // 3. Create requirements
        const created: Array<{ id: string; title: string; useCaseId: string | null }> = [];
        for (const req of body.requirements) {
            const useCaseId = yield* Effect.tryPromise({
                try: () => resolveUseCaseId(req),
                catch: cause => cause
            }) as unknown as string | undefined;

            const id = crypto.randomUUID();
            yield* createRequirement({
                id,
                title: req.title,
                description: req.description,
                steps: req.steps,
                priority: req.priority,
                useCaseId,
                codebaseId: body.codebaseId,
                userId,
                origin: "ai",
                reviewStatus: "draft"
            });
            created.push({ id, title: req.title, useCaseId: useCaseId ?? null });
        }

        return { created: created.length, requirements: created, useCasesCreated };
    });
}
```

IMPORTANT: The `resolveUseCaseId` function above mixes async/yield patterns. In practice, the implementer should use pure Effect.gen patterns throughout — no nested async functions. Use the cache Map with Effect operations. Read the existing service patterns in `packages/api/src/requirements/service.ts` to match the style.

- [ ] **Step 3: Add batch route**

In `packages/api/src/requirements/routes.ts`, add before `/:id` routes:

```typescript
import * as batchService from "./batch-service";

// Batch create
.post(
    "/requirements/batch",
    ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                ctx.set.status = 201;
                return yield* batchService.batchCreateRequirements(session.user.id, ctx.body);
            })
        ),
    {
        body: t.Object({
            requirements: t.Array(
                t.Object({
                    title: t.String({ maxLength: 200 }),
                    description: t.Optional(t.String({ maxLength: 5000 })),
                    steps: t.Array(StepSchema, { minItems: 1 }),
                    priority: PrioritySchema,
                    useCaseId: t.Optional(t.String()),
                    useCaseName: t.Optional(t.String()),
                    parentUseCaseName: t.Optional(t.String())
                }),
                { minItems: 1, maxItems: 50 }
            ),
            codebaseId: t.Optional(t.String())
        })
    }
)
```

Note: `StepSchema` and `PrioritySchema` are already defined in the routes file.

- [ ] **Step 4: Verify types and commit**

```bash
pnpm run check-types
git add packages/db/src/repository/use-case.ts packages/api/src/requirements/batch-service.ts packages/api/src/requirements/routes.ts
git commit -m "feat: add batch requirement creation with use case resolution"
```

---

### Task 3: MCP tools — suggest and batch create

Add the two new MCP tools.

**Files:**
- Create: `packages/mcp/src/suggestion-tools.ts`
- Modify: `packages/mcp/src/index.ts`

- [ ] **Step 1: Create suggestion tools**

Create `packages/mcp/src/suggestion-tools.ts`:

```typescript
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { apiFetch, truncate } from "./api-client.js";

export function registerSuggestionTools(server: McpServer): void {
    // 1. suggest_requirements
    server.tool(
        "suggest_requirements",
        "Get context from the knowledge base to suggest new requirements. Returns existing requirements, coverage gaps, health issues, and relevant chunks for a focus area. Use this context to generate requirement suggestions for the developer.",
        {
            focus: z.string().optional().describe("Focus area (e.g., 'auth', 'error handling', 'testing'). Omit for broad overview."),
            codebaseId: z.string().optional().describe("Codebase ID to scope suggestions")
        },
        async ({ focus, codebaseId }) => {
            const params = new URLSearchParams();
            if (focus) params.set("focus", focus);
            if (codebaseId) params.set("codebaseId", codebaseId);

            const data = await apiFetch(`/requirements/suggest-context?${params}`) as {
                useCases: Array<{
                    id: string; name: string; parentId: string | null;
                    requirementCount: number;
                    requirements: Array<{ id: string; title: string; status: string }>;
                }>;
                coverageGaps: Array<{ id: string; title: string; type: string }>;
                healthIssues: { orphanCount: number; staleCount: number; thinCount: number };
                relevantChunks: Array<{ id: string; title: string; content: string }>;
            };

            const sections: string[] = [];
            sections.push("# Knowledge Base Context for Requirement Suggestions\n");

            if (focus) sections.push(`**Focus area:** ${focus}\n`);

            // Existing requirements by use case
            sections.push("## Existing Requirements\n");
            for (const uc of data.useCases) {
                const parent = uc.parentId ? " (sub use case)" : "";
                sections.push(`### ${uc.name}${parent} (${uc.requirementCount} requirements)`);
                for (const r of uc.requirements) {
                    sections.push(`- [${r.status}] ${r.title}`);
                }
                sections.push("");
            }

            // Coverage gaps
            if (data.coverageGaps.length > 0) {
                sections.push("## Uncovered Chunks (no requirements linked)\n");
                for (const gap of data.coverageGaps) {
                    sections.push(`- ${gap.title} (${gap.id})`);
                }
                sections.push("");
            }

            // Health issues
            sections.push("## Knowledge Health\n");
            sections.push(`- Orphan chunks (no connections): ${data.healthIssues.orphanCount}`);
            sections.push(`- Stale chunks (>30 days old, neighbors updated): ${data.healthIssues.staleCount}`);
            sections.push(`- Thin chunks (<100 chars): ${data.healthIssues.thinCount}`);
            sections.push("");

            // Relevant chunks
            if (data.relevantChunks.length > 0) {
                sections.push("## Relevant Chunks\n");
                for (const c of data.relevantChunks) {
                    sections.push(`### ${c.title} (${c.id})`);
                    sections.push(c.content);
                    sections.push("");
                }
            }

            sections.push("---\n");
            sections.push("Based on this context, suggest new requirements organized into use cases.");
            sections.push("For each requirement, provide: title, Given/When/Then steps, priority, and which use case it belongs to.");
            sections.push("When ready, call `create_requirements_batch` to create the approved requirements.");

            return {
                content: [{ type: "text" as const, text: sections.join("\n") }]
            };
        }
    );

    // 2. create_requirements_batch
    server.tool(
        "create_requirements_batch",
        "Batch create multiple requirements with automatic use case resolution. Use cases are created automatically if they don't exist.",
        {
            requirements: z.array(z.object({
                title: z.string().describe("Requirement title"),
                description: z.string().optional().describe("Requirement description"),
                steps: z.array(z.object({
                    keyword: z.enum(["given", "when", "then", "and", "but"]),
                    text: z.string()
                })).min(1).describe("Given/When/Then steps"),
                priority: z.enum(["must", "should", "could", "wont"]).optional().describe("MoSCoW priority"),
                useCaseId: z.string().optional().describe("Existing use case ID"),
                useCaseName: z.string().optional().describe("Use case name (created if doesn't exist)"),
                parentUseCaseName: z.string().optional().describe("Parent use case name (created if doesn't exist)")
            })).min(1).max(50).describe("Array of requirements to create"),
            codebaseId: z.string().optional().describe("Codebase ID to associate with")
        },
        async ({ requirements, codebaseId }) => {
            const body: Record<string, unknown> = { requirements };
            if (codebaseId) body.codebaseId = codebaseId;

            const data = await apiFetch("/requirements/batch", {
                method: "POST",
                body: JSON.stringify(body)
            }) as {
                created: number;
                requirements: Array<{ id: string; title: string; useCaseId: string | null }>;
                useCasesCreated: Array<{ id: string; name: string; parentId: string | null }>;
            };

            const lines: string[] = [];
            lines.push(`# Created ${data.created} Requirements\n`);

            if (data.useCasesCreated.length > 0) {
                lines.push(`**Use cases auto-created:** ${data.useCasesCreated.map(uc => uc.name).join(", ")}\n`);
            }

            lines.push("## Requirements Created\n");
            for (const r of data.requirements) {
                lines.push(`- ${r.title} (${r.id})`);
            }

            return {
                content: [{ type: "text" as const, text: lines.join("\n") }]
            };
        }
    );
}
```

- [ ] **Step 2: Register in MCP server**

In `packages/mcp/src/index.ts`, add:
```typescript
import { registerSuggestionTools } from "./suggestion-tools.js";
```

After existing registrations:
```typescript
registerSuggestionTools(server);
```

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/mcp/src/suggestion-tools.ts packages/mcp/src/index.ts
git commit -m "feat: add suggest_requirements and create_requirements_batch MCP tools"
```

---

### Task 4: Final verification

- [ ] **Step 1: Run type check**

Run: `pnpm run check-types`

- [ ] **Step 2: Run tests**

Run: `pnpm test`

- [ ] **Step 3: Manual verification**

Test the MCP tools:
```bash
# Start the API server
pnpm dev

# In another terminal, test suggest context
curl http://localhost:3000/api/requirements/suggest-context | jq .

# Test with focus
curl "http://localhost:3000/api/requirements/suggest-context?focus=auth" | jq .

# Test batch creation
curl -X POST http://localhost:3000/api/requirements/batch \
  -H "Content-Type: application/json" \
  -d '{
    "requirements": [{
      "title": "Test batch requirement",
      "steps": [
        {"keyword": "given", "text": "a test setup"},
        {"keyword": "when", "text": "the action is performed"},
        {"keyword": "then", "text": "the result is correct"}
      ],
      "priority": "should",
      "useCaseName": "Test Use Case"
    }]
  }' | jq .
```

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore: final cleanup for AI requirement suggestions"
```
