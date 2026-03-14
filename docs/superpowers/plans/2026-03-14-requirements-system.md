# Requirements System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a structured Given/When/Then requirements system with step validation, cross-referencing, and multi-format export.

**Architecture:** Separate `requirement` entity (not a chunk) with JSONB steps, linked to chunks via join table. Step validator enforces sequence rules. Cross-reference checker matches step text against known file refs and chunk titles. Export adapters generate Gherkin, Vitest, and markdown from the same steps data.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router/Query, Commander.js, Vitest

**Spec:** `docs/superpowers/specs/2026-03-14-requirements-system-design.md`

---

## File Structure

### New files
- `packages/db/src/schema/requirement.ts` — requirement + requirement_chunk tables
- `packages/db/src/repository/requirement.ts` — CRUD + chunk linking
- `packages/db/src/__tests__/requirement.test.ts` — schema test
- `packages/api/src/errors.ts` — add StepValidationError (modify)
- `packages/api/src/requirements/validator.ts` — step sequence validation
- `packages/api/src/requirements/validator.test.ts` — validator tests
- `packages/api/src/requirements/cross-ref.ts` — cross-reference checker
- `packages/api/src/requirements/export.ts` — Gherkin/Vitest/markdown adapters
- `packages/api/src/requirements/export.test.ts` — export adapter tests
- `packages/api/src/requirements/service.ts` — composes validation + cross-ref + CRUD
- `packages/api/src/requirements/routes.ts` — HTTP endpoints
- `apps/cli/src/commands/requirements.ts` — CLI command group
- `apps/web/src/routes/requirements.tsx` — list page
- `apps/web/src/routes/requirements.new.tsx` — create page
- `apps/web/src/routes/requirements.$requirementId.tsx` — detail page

### Modified files
- `packages/db/src/schema/index.ts` — export requirement schema
- `packages/db/src/repository/index.ts` — export requirement repository
- `packages/api/src/index.ts` — register routes, add StepValidationError to error handler
- `apps/cli/src/index.ts` — register requirements command
- `apps/web/src/routes/__root.tsx` — add Requirements nav link
- `apps/web/src/features/nav/mobile-nav.tsx` — add Requirements nav link

---

## Chunk 1: Schema + Validator

### Task 1: Requirement schema

**Files:**
- Create: `packages/db/src/schema/requirement.ts`
- Create: `packages/db/src/__tests__/requirement.test.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Write schema test**

```typescript
// packages/db/src/__tests__/requirement.test.ts
import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";
import { requirement, requirementChunk } from "../schema/requirement";

describe("requirement table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(requirement);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("title");
        expect(columns).toHaveProperty("description");
        expect(columns).toHaveProperty("steps");
        expect(columns).toHaveProperty("status");
        expect(columns).toHaveProperty("priority");
        expect(columns).toHaveProperty("codebaseId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("requirementChunk table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(requirementChunk);
        expect(columns).toHaveProperty("requirementId");
        expect(columns).toHaveProperty("chunkId");
    });
});
```

- [ ] **Step 2: Write schema**

```typescript
// packages/db/src/schema/requirement.ts
import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, primaryKey, text, timestamp } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";

export interface RequirementStep {
    keyword: "given" | "when" | "then" | "and" | "but";
    text: string;
    params?: Record<string, string>;
}

export const requirement = pgTable(
    "requirement",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        description: text("description"),
        steps: jsonb("steps").$type<RequirementStep[]>().notNull(),
        status: text("status").notNull().default("untested"),
        priority: text("priority"),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("requirement_userId_idx").on(table.userId),
        index("requirement_codebaseId_idx").on(table.codebaseId),
        index("requirement_status_idx").on(table.status)
    ]
);

export const requirementChunk = pgTable(
    "requirement_chunk",
    {
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.requirementId, table.chunkId] })]
);

export const requirementRelations = relations(requirement, ({ one, many }) => ({
    user: one(user, { fields: [requirement.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [requirement.codebaseId], references: [codebase.id] }),
    requirementChunks: many(requirementChunk)
}));

export const requirementChunkRelations = relations(requirementChunk, ({ one }) => ({
    requirement: one(requirement, { fields: [requirementChunk.requirementId], references: [requirement.id] }),
    chunk: one(chunk, { fields: [requirementChunk.chunkId], references: [chunk.id] })
}));
```

Note: The `CHECK (jsonb_array_length(steps) > 0)` constraint from the spec may need a raw SQL migration if Drizzle doesn't support CHECK constraints directly. Add it via `db:push` or a migration file.

- [ ] **Step 3: Export and run tests**

Add `export * from "./requirement";` to `packages/db/src/schema/index.ts`.

Run: `cd packages/db && pnpm vitest run`

- [ ] **Step 4: Push schema**

Run: `pnpm db:push`

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/schema/requirement.ts packages/db/src/__tests__/requirement.test.ts packages/db/src/schema/index.ts
git commit -m "feat(db): add requirement and requirement_chunk schema"
```

---

### Task 2: Step validator

**Files:**
- Create: `packages/api/src/requirements/validator.ts`
- Create: `packages/api/src/requirements/validator.test.ts`

- [ ] **Step 1: Write validator tests**

```typescript
// packages/api/src/requirements/validator.test.ts
import { describe, expect, it } from "vitest";
import { validateSteps } from "./validator";

describe("validateSteps", () => {
    it("accepts valid given-when-then sequence", () => {
        const result = validateSteps([
            { keyword: "given", text: "a user is logged in" },
            { keyword: "when", text: "they visit /dashboard" },
            { keyword: "then", text: "they see their chunks" }
        ]);
        expect(result).toEqual([]);
    });

    it("accepts and/but continuations", () => {
        const result = validateSteps([
            { keyword: "given", text: "a user is logged in" },
            { keyword: "and", text: "they have chunks" },
            { keyword: "when", text: "they visit /dashboard" },
            { keyword: "but", text: "they are on mobile" },
            { keyword: "then", text: "they see responsive layout" },
            { keyword: "and", text: "they see their chunks" }
        ]);
        expect(result).toEqual([]);
    });

    it("rejects empty steps", () => {
        const result = validateSteps([]);
        expect(result).toHaveLength(1);
        expect(result[0].error).toContain("at least one step");
    });

    it("rejects if first step is not given", () => {
        const result = validateSteps([
            { keyword: "when", text: "they visit /dashboard" },
            { keyword: "then", text: "they see chunks" }
        ]);
        expect(result[0].step).toBe(0);
        expect(result[0].error).toContain("given");
    });

    it("rejects missing when", () => {
        const result = validateSteps([
            { keyword: "given", text: "a user" },
            { keyword: "then", text: "they see chunks" }
        ]);
        expect(result.length).toBeGreaterThan(0);
    });

    it("rejects missing then", () => {
        const result = validateSteps([
            { keyword: "given", text: "a user" },
            { keyword: "when", text: "they visit" }
        ]);
        expect(result.length).toBeGreaterThan(0);
    });

    it("rejects when before given", () => {
        const result = validateSteps([
            { keyword: "given", text: "a user" },
            { keyword: "then", text: "they see" },
            { keyword: "when", text: "they visit" }
        ]);
        expect(result.length).toBeGreaterThan(0);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && pnpm vitest run src/requirements/validator.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement validator**

```typescript
// packages/api/src/requirements/validator.ts
import type { RequirementStep } from "@fubbik/db/schema/requirement";

interface StepError {
    step: number;
    error: string;
}

export function validateSteps(steps: RequirementStep[]): StepError[] {
    const errors: StepError[] = [];

    if (steps.length === 0) {
        errors.push({ step: -1, error: "Must have at least one step" });
        return errors;
    }

    if (steps[0].keyword !== "given") {
        errors.push({ step: 0, error: "First step must be 'given'" });
    }

    // Track which phase we're in: given -> when -> then
    type Phase = "given" | "when" | "then";
    let phase: Phase = "given";
    let hasWhen = false;
    let hasThen = false;

    for (let i = 0; i < steps.length; i++) {
        const { keyword } = steps[i];

        if (keyword === "and" || keyword === "but") {
            // Inherits current phase — valid in any phase after the first step
            if (i === 0) {
                errors.push({ step: i, error: "'and'/'but' cannot be the first step" });
            }
            continue;
        }

        if (keyword === "given") {
            if (phase !== "given") {
                errors.push({ step: i, error: "'given' must come before 'when' and 'then'" });
            }
        } else if (keyword === "when") {
            if (phase === "then") {
                errors.push({ step: i, error: "'when' must come before 'then'" });
            }
            phase = "when";
            hasWhen = true;
        } else if (keyword === "then") {
            phase = "then";
            hasThen = true;
        }
    }

    if (!hasWhen) {
        errors.push({ step: -1, error: "Must contain at least one 'when' step" });
    }
    if (!hasThen) {
        errors.push({ step: -1, error: "Must contain at least one 'then' step" });
    }

    return errors;
}
```

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run src/requirements/validator.test.ts`
Expected: PASS

- [ ] **Step 5: Add StepValidationError**

Add to `packages/api/src/errors.ts`:
```typescript
export class StepValidationError extends Data.TaggedError("StepValidationError")<{
    errors: Array<{ step: number; error: string }>;
}> {}
```

Add to the error handler in `packages/api/src/index.ts`:
```typescript
case "StepValidationError":
    set.status = 400;
    return { message: "Invalid steps", errors: effectError.errors };
```

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/requirements/validator.ts packages/api/src/requirements/validator.test.ts packages/api/src/errors.ts packages/api/src/index.ts
git commit -m "feat(api): add step sequence validator and StepValidationError"
```

---

## Chunk 2: Repository + Export Adapters

### Task 3: Requirement repository

**Files:**
- Create: `packages/db/src/repository/requirement.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write repository**

Functions needed (all return `Effect<T, DatabaseError>`):
- `createRequirement(params: { id, title, description?, steps, status?, priority?, codebaseId?, userId })`
- `getRequirementById(id, userId?)`
- `listRequirements(params: { userId, codebaseId?, status?, priority?, limit, offset })`
- `updateRequirement(id, userId, params: { title?, description?, steps?, status?, priority? })`
- `deleteRequirement(id, userId)`
- `updateRequirementStatus(id, userId, status)`
- `setRequirementChunks(requirementId, chunkIds[])` — replace-all pattern
- `getChunksForRequirement(requirementId)` — join through requirement_chunk + chunk
- `getRequirementStats(userId, codebaseId?)` — returns `{ total, passing, failing, untested }`

Follow the existing Effect.tryPromise + DatabaseError pattern. The `updateRequirement` function must handle empty body (skip update if no fields to set, like the fix we made for chunks).

- [ ] **Step 2: Export from repository index**

Add `export * from "./requirement";` to `packages/db/src/repository/index.ts`.

- [ ] **Step 3: Run tests**

Run: `cd packages/db && pnpm vitest run`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/requirement.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add requirement repository with CRUD, chunk linking, and stats"
```

---

### Task 4: Export adapters

**Files:**
- Create: `packages/api/src/requirements/export.ts`
- Create: `packages/api/src/requirements/export.test.ts`

- [ ] **Step 1: Write export tests**

```typescript
// packages/api/src/requirements/export.test.ts
import { describe, expect, it } from "vitest";
import { toGherkin, toVitest, toMarkdown } from "./export";

const steps = [
    { keyword: "given" as const, text: "a user is logged in" },
    { keyword: "when" as const, text: "they visit /dashboard" },
    { keyword: "then" as const, text: "they see their chunks" }
];
const title = "User views dashboard";

describe("toGherkin", () => {
    it("generates valid Gherkin", () => {
        const result = toGherkin(title, steps);
        expect(result).toContain("Feature: User views dashboard");
        expect(result).toContain("Given a user is logged in");
        expect(result).toContain("When they visit /dashboard");
        expect(result).toContain("Then they see their chunks");
    });
});

describe("toVitest", () => {
    it("generates vitest scaffold", () => {
        const result = toVitest(title, steps);
        expect(result).toContain('describe("User views dashboard"');
        expect(result).toContain("// Given a user is logged in");
        expect(result).toContain('throw new Error("Not implemented")');
    });
});

describe("toMarkdown", () => {
    it("generates checklist", () => {
        const result = toMarkdown(title, steps);
        expect(result).toContain("## User views dashboard");
        expect(result).toContain("- [ ] Given a user is logged in");
    });
});
```

- [ ] **Step 2: Implement export adapters**

```typescript
// packages/api/src/requirements/export.ts
import type { RequirementStep } from "@fubbik/db/schema/requirement";

function interpolateParams(text: string, params?: Record<string, string>): string {
    if (!params) return text;
    return text.replace(/\{(\w+)\}/g, (_, key) => params[key] ?? `{${key}}`);
}

function capitalize(keyword: string): string {
    return keyword.charAt(0).toUpperCase() + keyword.slice(1);
}

export function toGherkin(title: string, steps: RequirementStep[]): string {
    let out = `Feature: ${title}\n\n  Scenario: ${title}\n`;
    for (const step of steps) {
        const text = interpolateParams(step.text, step.params);
        out += `    ${capitalize(step.keyword)} ${text}\n`;
    }
    return out;
}

export function toVitest(title: string, steps: RequirementStep[]): string {
    const comments = steps
        .map(s => `        // ${capitalize(s.keyword)} ${interpolateParams(s.text, s.params)}`)
        .join("\n");
    return `import { describe, it } from "vitest";\n\ndescribe("${title}", () => {\n    it("should satisfy requirements", () => {\n${comments}\n        throw new Error("Not implemented");\n    });\n});\n`;
}

export function toMarkdown(title: string, steps: RequirementStep[]): string {
    let out = `## ${title}\n\n`;
    for (const step of steps) {
        const text = interpolateParams(step.text, step.params);
        out += `- [ ] ${capitalize(step.keyword)} ${text}\n`;
    }
    return out;
}
```

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm vitest run src/requirements/export.test.ts`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/requirements/export.ts packages/api/src/requirements/export.test.ts
git commit -m "feat(api): add Gherkin, Vitest, and markdown export adapters"
```

---

## Chunk 3: Cross-Reference + Service + Routes

### Task 5: Cross-reference checker

**Files:**
- Create: `packages/api/src/requirements/cross-ref.ts`

- [ ] **Step 1: Write cross-reference checker**

```typescript
// packages/api/src/requirements/cross-ref.ts
import { lookupChunksByFilePath } from "@fubbik/db/repository";
import { Effect } from "effect";
import type { RequirementStep } from "@fubbik/db/schema/requirement";

interface CrossRefWarning {
    step: number;
    type: "file_not_found" | "chunk_not_found";
    reference: string;
}

// Extract file paths from step text (patterns like src/auth/session.ts)
function extractFilePaths(text: string): string[] {
    const matches = text.match(/(?:^|\s)([\w./\-]+\.[\w]+)/g);
    return matches ? matches.map(m => m.trim()) : [];
}

// Extract quoted strings (potential chunk references)
function extractQuotedStrings(text: string): string[] {
    const matches = text.match(/['"]([^'"]+)['"]/g);
    return matches ? matches.map(m => m.slice(1, -1)) : [];
}

export function crossReferenceSteps(
    steps: RequirementStep[],
    userId: string
): Effect.Effect<CrossRefWarning[], never> {
    return Effect.tryPromise({
        try: async () => {
            const warnings: CrossRefWarning[] = [];

            for (let i = 0; i < steps.length; i++) {
                const { text } = steps[i];

                // Check file paths
                for (const path of extractFilePaths(text)) {
                    const result = await Effect.runPromise(
                        lookupChunksByFilePath(path, userId).pipe(
                            Effect.catchAll(() => Effect.succeed([]))
                        )
                    );
                    if (result.length === 0) {
                        warnings.push({ step: i, type: "file_not_found", reference: path });
                    }
                }
            }

            return warnings;
        },
        catch: () => [] as CrossRefWarning[]
    }).pipe(Effect.catchAll(() => Effect.succeed([] as CrossRefWarning[])));
}
```

Note: Cross-referencing is best-effort — errors are swallowed and return empty warnings. The implementer should verify the regex patterns work for the expected file path formats and adjust as needed.

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/requirements/cross-ref.ts
git commit -m "feat(api): add cross-reference checker for requirement steps"
```

---

### Task 6: Service

**Files:**
- Create: `packages/api/src/requirements/service.ts`

- [ ] **Step 1: Write service**

Compose validation + cross-ref + CRUD:

- `listRequirements(userId, query)` — delegates to repo
- `getRequirement(id, userId)` — returns requirement + linked chunks, NotFoundError if missing
- `createRequirement(userId, body)` — validate steps (fail with StepValidationError if invalid), create, cross-reference, return `{ requirement, warnings }`
- `updateRequirement(id, userId, body)` — validate steps only if `steps` present in body, update, cross-reference if steps changed, return `{ requirement, warnings }`
- `deleteRequirement(id, userId)` — check exists, delete
- `updateStatus(id, userId, status)` — delegates to repo
- `setChunks(requirementId, userId, chunkIds)` — verify requirement exists and belongs to user. Verify all chunk IDs belong to user (batch check). Then set.
- `getStats(userId, codebaseId?)` — delegates to repo
- `exportRequirement(id, userId, format)` — fetch, convert via adapter
- `exportAll(userId, codebaseId, format)` — fetch all, convert each

Import `validateSteps` from `./validator`, `crossReferenceSteps` from `./cross-ref`, export adapters from `./export`.

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/requirements/service.ts
git commit -m "feat(api): add requirements service composing validation, cross-ref, and CRUD"
```

---

### Task 7: Routes

**Files:**
- Create: `packages/api/src/requirements/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Write routes**

Declaration order (critical for Elysia):
1. `GET /requirements/stats`
2. `GET /requirements/export` (`?format=gherkin|vitest|markdown&codebaseId=`)
3. `GET /requirements` (list, `?codebaseId=&status=&priority=&limit=&offset=`)
4. `POST /requirements`
5. `GET /requirements/:id`
6. `PATCH /requirements/:id`
7. `DELETE /requirements/:id`
8. `PATCH /requirements/:id/status` (body: `{ status: "passing"|"failing"|"untested" }`)
9. `PUT /requirements/:id/chunks` (body: array of chunk IDs)
10. `GET /requirements/:id/export` (`?format=gherkin|vitest|markdown`)

Validation schemas:
- Steps: `t.Array(t.Object({ keyword: t.Union([t.Literal("given"), t.Literal("when"), t.Literal("then"), t.Literal("and"), t.Literal("but")]), text: t.String({ maxLength: 1000 }), params: t.Optional(t.Record(t.String(), t.String())) }))`
- Status: `t.Union([t.Literal("passing"), t.Literal("failing"), t.Literal("untested")])`
- Priority: `t.Optional(t.Union([t.Literal("must"), t.Literal("should"), t.Literal("could"), t.Literal("wont")]))`
- Format: `t.Union([t.Literal("gherkin"), t.Literal("vitest"), t.Literal("markdown")])`

- [ ] **Step 2: Register in API index**

Import and `.use(requirementRoutes)` in `packages/api/src/index.ts`.

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm vitest run`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/requirements/routes.ts packages/api/src/index.ts
git commit -m "feat(api): add requirements CRUD, export, and stats routes"
```

---

## Chunk 4: CLI

### Task 8: CLI requirements commands

**Files:**
- Create: `apps/cli/src/commands/requirements.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Write command group**

```
fubbik requirements list [--status <status>] [--codebase <name>] [--priority <priority>]
fubbik requirements add <title> --step "given: ..." --step "when: ..." --step "then: ..."
fubbik requirements status <id> passing|failing|untested
fubbik requirements export --format gherkin|vitest|markdown [--codebase <name>]
fubbik requirements verify [--codebase <name>]
```

Follow existing CLI patterns:
- Use `output(cmd, data, humanReadable)` and `outputQuiet(cmd, id)` from `../lib/output`
- Use `console.error()` for errors
- Use `getServerUrl()` from `../lib/store`
- Use `detectCodebase()` for `--codebase` resolution
- Parse `--step` flags: split on first `:` to get keyword and text

- [ ] **Step 2: Register in CLI index**

Add `import { requirementsCommand } from "./commands/requirements";` and `program.addCommand(requirementsCommand);` to `apps/cli/src/index.ts`.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/requirements.ts apps/cli/src/index.ts
git commit -m "feat(cli): add requirements command group (list, add, status, export, verify)"
```

---

## Chunk 5: Web UI

### Task 9: Requirements list page

**Files:**
- Create: `apps/web/src/routes/requirements.tsx`
- Modify: `apps/web/src/routes/__root.tsx`
- Modify: `apps/web/src/features/nav/mobile-nav.tsx`

- [ ] **Step 1: Read existing list pages for pattern**

Read `apps/web/src/routes/codebases.tsx` or `apps/web/src/routes/knowledge-health.tsx`.

- [ ] **Step 2: Create list page**

`/requirements` route with:
- Stats summary bar at top: total, passing (green badge), failing (red), untested (gray)
- Query: `api.api.requirements.get({ query: { codebaseId, status, priority } })`
- Filterable by status, priority, codebase (via `useActiveCodebase`)
- Each row: title, status badge (color-coded), priority badge (MoSCoW), step count, created date
- Click navigates to detail page
- Quick actions: status toggle dropdown, delete button

- [ ] **Step 3: Add nav links**

"Requirements" link in nav between "Health" and "Templates" in both desktop and mobile nav.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/requirements.tsx apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add requirements list page with stats, filters, and status controls"
```

---

### Task 10: Requirement create page

**Files:**
- Create: `apps/web/src/routes/requirements.new.tsx`

- [ ] **Step 1: Create the page**

`/requirements/new` route with form:
- Title input
- Description textarea
- Priority selector (must/should/could/won't dropdown)
- Codebase selector (from `useActiveCodebase` or dropdown)
- **Step builder:**
  - Ordered list of step rows
  - Each row: keyword dropdown (given/when/then/and/but) + text input
  - "Add step" button appends a new row
  - Remove button per row
  - Real-time validation: run `validateSteps` client-side, highlight errors inline
- Linked chunks: searchable multi-select (fetch chunks for the codebase)
- Submit: `POST /api/requirements`, then `PUT /api/requirements/:id/chunks` if chunks selected
- On success: navigate to detail page, show warnings if any

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/requirements.new.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add requirement create page with step builder and validation"
```

---

### Task 11: Requirement detail page

**Files:**
- Create: `apps/web/src/routes/requirements.$requirementId.tsx`

- [ ] **Step 1: Create the page**

`/requirements/:requirementId` route with:
- Title, description, priority badge, status badge (color-coded)
- Steps displayed as formatted Given/When/Then block (each step on its own line, keyword bold)
- Cross-reference warnings section (if any, fetched on load via the requirement response)
- Linked chunks as clickable links to `/chunks/:id`
- Export buttons: Gherkin, Vitest, Markdown — each calls `GET /api/requirements/:id/export?format=` and either downloads or copies to clipboard
- Status toggle: three buttons (passing/failing/untested) calling `PATCH /api/requirements/:id/status`
- Edit button → navigates to `/requirements/new` with pre-filled data (or inline editing if simpler)

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/requirements.$requirementId.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add requirement detail page with export, status toggle, and linked chunks"
```

---

### Task 12: Final verification

- [ ] **Step 1: Run type check**

Run: `pnpm run check-types`

- [ ] **Step 2: Run all tests**

Run: `pnpm test`

- [ ] **Step 3: Fix any issues and commit**

```bash
git add -A && git commit -m "fix: resolve issues from requirements system implementation"
```
