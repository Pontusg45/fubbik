# Richer Chunks Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich chunks with structured metadata (appliesTo), file references, decision context fields, and templates.

**Architecture:** Four vertical slices, each going schema → repository → service → routes → UI. Decision fields are JSONB columns on the chunk table. AppliesTo, file references, and templates each get their own tables. Built-in templates seeded via SQL migration.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router/Query, Vitest

**Spec:** `docs/superpowers/specs/2026-03-14-richer-chunks-design.md`

---

## File Structure

### New files
- `packages/db/src/schema/applies-to.ts` — chunk_applies_to table
- `packages/db/src/schema/file-ref.ts` — chunk_file_ref table
- `packages/db/src/schema/template.ts` — chunk_template table
- `packages/db/src/repository/applies-to.ts` — applies-to CRUD
- `packages/db/src/repository/file-ref.ts` — file-ref CRUD + reverse lookup
- `packages/db/src/repository/template.ts` — template CRUD
- `packages/db/src/__tests__/applies-to.test.ts` — schema test
- `packages/db/src/__tests__/file-ref.test.ts` — schema test
- `packages/db/src/__tests__/template.test.ts` — schema test
- `packages/api/src/applies-to/service.ts` — applies-to service
- `packages/api/src/applies-to/routes.ts` — applies-to routes
- `packages/api/src/file-refs/service.ts` — file-refs service
- `packages/api/src/file-refs/routes.ts` — file-refs routes
- `packages/api/src/templates/service.ts` — templates service
- `packages/api/src/templates/routes.ts` — templates routes
- `apps/web/src/routes/templates.tsx` — templates management page

### Modified files
- `packages/db/src/schema/chunk.ts` — add rationale, alternatives, consequences columns
- `packages/db/src/schema/index.ts` — export new schemas
- `packages/db/src/repository/index.ts` — export new repositories
- `packages/db/src/repository/chunk.ts` — include new columns in queries
- `packages/api/src/index.ts` — register new routes
- `packages/api/src/chunks/service.ts` — extend getChunkDetail, createChunk, updateChunk
- `packages/api/src/chunks/routes.ts` — add new body fields
- `apps/web/src/routes/__root.tsx` — add Templates nav link
- `apps/web/src/features/nav/mobile-nav.tsx` — add Templates nav link
- `apps/web/src/routes/chunks.new.tsx` — template selector, applies-to, file-refs, decision fields
- `apps/web/src/routes/chunks.$chunkId.tsx` — display applies-to, file-refs, decision context
- `apps/web/src/routes/chunks.$chunkId.edit.tsx` — edit applies-to, file-refs, decision context

---

## Chunk 1: Decision Context Fields

### Task 1: Add decision columns to chunk schema

**Files:**
- Modify: `packages/db/src/schema/chunk.ts`

- [ ] **Step 1: Add columns**

Add three new columns to the `chunk` table definition after the `scope` column:

```typescript
rationale: text("rationale"),
alternatives: jsonb("alternatives").$type<string[]>(),
consequences: text("consequences"),
```

These are all nullable — no `.notNull()`.

- [ ] **Step 2: Run tests**

Run: `cd packages/db && pnpm vitest run`
Expected: PASS (existing tests don't assert specific column lists exhaustively for chunk)

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/chunk.ts
git commit -m "feat(db): add rationale, alternatives, consequences columns to chunk"
```

---

### Task 2: Extend chunk service and routes for decision fields

**Files:**
- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Add decision fields to POST /chunks body schema**

In `packages/api/src/chunks/routes.ts`, add to the POST body `t.Object`:
```typescript
rationale: t.Optional(t.String({ maxLength: 5000 })),
alternatives: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 })),
consequences: t.Optional(t.String({ maxLength: 5000 })),
```

Same fields for the PATCH body.

- [ ] **Step 2: Pass decision fields through service to repository**

In `packages/api/src/chunks/service.ts`:
- `createChunk`: pass `rationale`, `alternatives`, `consequences` to `createChunkRepo`
- `updateChunk`: include them in the repo body (they're already passed through via spread, but verify the destructure doesn't strip them)

Read the current service code to understand the destructure pattern. The `updateChunk` does `const { tags: _tags, codebaseIds: _codebaseIds, ...repoBody } = body` — the new fields will flow through in `repoBody` automatically.

For `createChunk`, the repo call currently only passes `id, title, content, type, userId`. Add the three new fields:
```typescript
return createChunkRepo({
    id,
    title: body.title,
    content: body.content ?? "",
    type: body.type ?? "note",
    userId,
    rationale: body.rationale,
    alternatives: body.alternatives,
    consequences: body.consequences
})
```

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm vitest run src/codebases/normalize-url.test.ts`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/chunks/routes.ts packages/api/src/chunks/service.ts
git commit -m "feat(api): support decision context fields on chunk create/update"
```

---

## Chunk 2: Applies-To

### Task 3: Applies-to schema and repository

**Files:**
- Create: `packages/db/src/schema/applies-to.ts`
- Create: `packages/db/src/repository/applies-to.ts`
- Create: `packages/db/src/__tests__/applies-to.test.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write schema test**

```typescript
// packages/db/src/__tests__/applies-to.test.ts
import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";
import { chunkAppliesTo } from "../schema/applies-to";

describe("chunkAppliesTo table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkAppliesTo);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("pattern");
        expect(columns).toHaveProperty("note");
    });
});
```

- [ ] **Step 2: Write schema**

```typescript
// packages/db/src/schema/applies-to.ts
import { relations } from "drizzle-orm";
import { index, pgTable, text } from "drizzle-orm/pg-core";
import { chunk } from "./chunk";

export const chunkAppliesTo = pgTable(
    "chunk_applies_to",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        pattern: text("pattern").notNull(),
        note: text("note")
    },
    table => [index("chunk_applies_to_chunkId_idx").on(table.chunkId)]
);

export const chunkAppliesToRelations = relations(chunkAppliesTo, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkAppliesTo.chunkId], references: [chunk.id] })
}));
```

- [ ] **Step 3: Write repository**

```typescript
// packages/db/src/repository/applies-to.ts
import { eq } from "drizzle-orm";
import { Effect } from "effect";
import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkAppliesTo } from "../schema/applies-to";

export function getAppliesToForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({ id: chunkAppliesTo.id, pattern: chunkAppliesTo.pattern, note: chunkAppliesTo.note })
                .from(chunkAppliesTo)
                .where(eq(chunkAppliesTo.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function setAppliesToForChunk(chunkId: string, patterns: { pattern: string; note?: string | null }[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkAppliesTo).where(eq(chunkAppliesTo.chunkId, chunkId));
            if (patterns.length === 0) return [];
            return db
                .insert(chunkAppliesTo)
                .values(patterns.map(p => ({ id: crypto.randomUUID(), chunkId, pattern: p.pattern, note: p.note ?? null })))
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 4: Export from indexes**

Add `export * from "./applies-to";` to both `packages/db/src/schema/index.ts` and `packages/db/src/repository/index.ts`.

- [ ] **Step 5: Run tests, push schema**

Run: `cd packages/db && pnpm vitest run && cd ../.. && pnpm db:push`

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/applies-to.ts packages/db/src/repository/applies-to.ts packages/db/src/__tests__/applies-to.test.ts packages/db/src/schema/index.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add chunk_applies_to schema and repository"
```

---

### Task 4: Applies-to service and routes

**Files:**
- Create: `packages/api/src/applies-to/service.ts`
- Create: `packages/api/src/applies-to/routes.ts`
- Modify: `packages/api/src/index.ts`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Write service**

```typescript
// packages/api/src/applies-to/service.ts
import { getAppliesToForChunk, setAppliesToForChunk, getChunkById } from "@fubbik/db/repository";
import { Effect } from "effect";
import { NotFoundError } from "../errors";

export function getAppliesTo(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" }))),
        Effect.flatMap(() => getAppliesToForChunk(chunkId))
    );
}

export function setAppliesTo(chunkId: string, userId: string, patterns: { pattern: string; note?: string | null }[]) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" }))),
        Effect.flatMap(() => setAppliesToForChunk(chunkId, patterns))
    );
}
```

- [ ] **Step 2: Write routes**

```typescript
// packages/api/src/applies-to/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as appliesToService from "./service";

export const appliesToRoutes = new Elysia()
    .get("/chunks/:id/applies-to", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => appliesToService.getAppliesTo(ctx.params.id, session.user.id))
            )
        )
    )
    .put(
        "/chunks/:id/applies-to",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => appliesToService.setAppliesTo(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Array(
                t.Object({
                    pattern: t.String({ maxLength: 500 }),
                    note: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()]))
                }),
                { maxItems: 50 }
            )
        }
    );
```

- [ ] **Step 3: Register routes and extend getChunkDetail**

In `packages/api/src/index.ts`, import and `.use(appliesToRoutes)`.

In `packages/api/src/chunks/service.ts`, extend `getChunkDetail` to include applies-to:
```typescript
import { getAppliesToForChunk } from "@fubbik/db/repository";

// In getChunkDetail, add to Effect.all:
appliesTo: getAppliesToForChunk(chunkId)
```

- [ ] **Step 4: Run tests**

Run: `cd packages/db && pnpm vitest run`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/applies-to/ packages/api/src/index.ts packages/api/src/chunks/service.ts
git commit -m "feat(api): add applies-to service, routes, and include in chunk detail"
```

---

## Chunk 3: File References

### Task 5: File-ref schema and repository

**Files:**
- Create: `packages/db/src/schema/file-ref.ts`
- Create: `packages/db/src/repository/file-ref.ts`
- Create: `packages/db/src/__tests__/file-ref.test.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write schema test**

```typescript
// packages/db/src/__tests__/file-ref.test.ts
import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";
import { chunkFileRef } from "../schema/file-ref";

describe("chunkFileRef table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkFileRef);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("path");
        expect(columns).toHaveProperty("anchor");
        expect(columns).toHaveProperty("relation");
    });
});
```

- [ ] **Step 2: Write schema**

```typescript
// packages/db/src/schema/file-ref.ts
import { relations } from "drizzle-orm";
import { index, pgTable, text } from "drizzle-orm/pg-core";
import { chunk } from "./chunk";

export const chunkFileRef = pgTable(
    "chunk_file_ref",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        path: text("path").notNull(),
        anchor: text("anchor"),
        relation: text("relation").notNull().default("documents")
    },
    table => [
        index("chunk_file_ref_chunkId_idx").on(table.chunkId),
        index("chunk_file_ref_path_idx").on(table.path)
    ]
);

export const chunkFileRefRelations = relations(chunkFileRef, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkFileRef.chunkId], references: [chunk.id] })
}));
```

- [ ] **Step 3: Write repository**

```typescript
// packages/db/src/repository/file-ref.ts
import { and, eq } from "drizzle-orm";
import { Effect } from "effect";
import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkFileRef } from "../schema/file-ref";

export function getFileRefsForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkFileRef.id,
                    path: chunkFileRef.path,
                    anchor: chunkFileRef.anchor,
                    relation: chunkFileRef.relation
                })
                .from(chunkFileRef)
                .where(eq(chunkFileRef.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function setFileRefsForChunk(
    chunkId: string,
    refs: { path: string; anchor?: string | null; relation: string }[]
) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkFileRef).where(eq(chunkFileRef.chunkId, chunkId));
            if (refs.length === 0) return [];
            return db
                .insert(chunkFileRef)
                .values(
                    refs.map(r => ({
                        id: crypto.randomUUID(),
                        chunkId,
                        path: r.path,
                        anchor: r.anchor ?? null,
                        relation: r.relation
                    }))
                )
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function lookupChunksByFilePath(path: string, userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    chunkId: chunkFileRef.chunkId,
                    chunkTitle: chunk.title,
                    path: chunkFileRef.path,
                    anchor: chunkFileRef.anchor,
                    relation: chunkFileRef.relation
                })
                .from(chunkFileRef)
                .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
                .where(and(eq(chunkFileRef.path, path), eq(chunk.userId, userId))),
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 4: Export and push**

Add exports to schema and repository indexes. Run: `cd packages/db && pnpm vitest run && cd ../.. && pnpm db:push`

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/schema/file-ref.ts packages/db/src/repository/file-ref.ts packages/db/src/__tests__/file-ref.test.ts packages/db/src/schema/index.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add chunk_file_ref schema and repository with reverse lookup"
```

---

### Task 6: File-ref service and routes

**Files:**
- Create: `packages/api/src/file-refs/service.ts`
- Create: `packages/api/src/file-refs/routes.ts`
- Modify: `packages/api/src/index.ts`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Write service**

Same pattern as applies-to service: ownership check via `getChunkById` before mutations. Reverse lookup uses `lookupChunksByFilePath` which already filters by userId.

- [ ] **Step 2: Write routes**

```
GET /chunks/:id/file-refs
PUT /chunks/:id/file-refs (body: array of { path, anchor?, relation })
GET /file-refs/lookup?path=<path>
```

The `relation` field validation:
```typescript
relation: t.Union([
    t.Literal("documents"),
    t.Literal("configures"),
    t.Literal("tests"),
    t.Literal("implements")
])
```

IMPORTANT: Register `/file-refs/lookup` as a standalone route, not under `/chunks/:id`. It's a top-level reverse lookup endpoint.

- [ ] **Step 3: Register routes and extend getChunkDetail**

In `packages/api/src/index.ts`, import and `.use(fileRefRoutes)`.

In `packages/api/src/chunks/service.ts`, add `fileReferences: getFileRefsForChunk(chunkId)` to the `Effect.all` in `getChunkDetail`.

- [ ] **Step 4: Run tests**

Run: `cd packages/db && pnpm vitest run`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/file-refs/ packages/api/src/index.ts packages/api/src/chunks/service.ts
git commit -m "feat(api): add file-refs service, routes, reverse lookup, and include in chunk detail"
```

---

## Chunk 4: Templates

### Task 7: Template schema, repository, and seed migration

**Files:**
- Create: `packages/db/src/schema/template.ts`
- Create: `packages/db/src/repository/template.ts`
- Create: `packages/db/src/__tests__/template.test.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write schema test**

Test that `chunkTemplate` has columns: id, name, description, type, content, isBuiltIn, userId, createdAt.

- [ ] **Step 2: Write schema**

```typescript
// packages/db/src/schema/template.ts
import { relations, sql } from "drizzle-orm";
import { boolean, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";
import { user } from "./auth";

export const chunkTemplate = pgTable(
    "chunk_template",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        type: text("type").notNull().default("note"),
        content: text("content").notNull().default(""),
        isBuiltIn: boolean("is_built_in").notNull().default(false),
        userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        uniqueIndex("template_user_name_idx")
            .on(table.userId, table.name)
            .where(sql`"user_id" IS NOT NULL`),
        uniqueIndex("template_builtin_name_idx")
            .on(table.name)
            .where(sql`"user_id" IS NULL`)
    ]
);

export const chunkTemplateRelations = relations(chunkTemplate, ({ one }) => ({
    user: one(user, { fields: [chunkTemplate.userId], references: [user.id] })
}));
```

- [ ] **Step 3: Write repository**

Functions: `listTemplates(userId)` (returns built-in + user's), `getTemplateById(id)`, `createTemplate(params)`, `updateTemplate(id, userId, params)`, `deleteTemplate(id, userId)`.

`listTemplates` returns all where `isBuiltIn = true` OR `userId = <userId>`.
`deleteTemplate` must check `isBuiltIn = false` before deleting.

- [ ] **Step 4: Create seed migration**

Create a SQL migration file for built-in templates. Add it to the migration runner in `packages/db/src/run-sql-migrations.ts`.

The SQL uses `INSERT ... ON CONFLICT (name) WHERE user_id IS NULL DO UPDATE SET content = EXCLUDED.content, description = EXCLUDED.description, type = EXCLUDED.type`.

Include the 4 built-in templates from the spec: Convention, Architecture Decision, Runbook, API Endpoint.

- [ ] **Step 5: Export, test, push**

Add exports. Run tests. Push schema: `pnpm db:push`.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/template.ts packages/db/src/repository/template.ts packages/db/src/__tests__/template.test.ts packages/db/src/schema/index.ts packages/db/src/repository/index.ts packages/db/src/run-sql-migrations.ts
git commit -m "feat(db): add chunk_template schema, repository, and seed migration"
```

---

### Task 8: Template service and routes

**Files:**
- Create: `packages/api/src/templates/service.ts`
- Create: `packages/api/src/templates/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Write service**

Functions: `listTemplates(userId)`, `createTemplate(userId, body)`, `updateTemplate(id, userId, body)`, `deleteTemplate(id, userId)`.

`deleteTemplate` must check the template is not built-in (return `ValidationError` if it is).

- [ ] **Step 2: Write routes**

```
GET /templates — list all (built-in + user's)
POST /templates — create custom template
PATCH /templates/:id — update custom template
DELETE /templates/:id — delete (user-created only)
```

- [ ] **Step 3: Register routes**

In `packages/api/src/index.ts`, import and `.use(templateRoutes)`.

- [ ] **Step 4: Run tests**

Run: `cd packages/db && pnpm vitest run`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/templates/ packages/api/src/index.ts
git commit -m "feat(api): add template CRUD service and routes"
```

---

## Chunk 5: Web UI

### Task 9: Chunk detail — display new fields

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Read the current chunk detail page**

Read `apps/web/src/routes/chunks.$chunkId.tsx` to understand the layout.

- [ ] **Step 2: Add applies-to section**

After the content section, if `chunk.appliesTo` has items, render a section:
- Label: "Applies To"
- List of monospace code badges: `pattern` with optional note in parentheses

- [ ] **Step 3: Add file references section**

If `chunk.fileReferences` has items:
- Label: "File References"
- List with monospace path, anchor badge (if present), relation label

- [ ] **Step 4: Add decision context section**

If any of `rationale`, `alternatives`, `consequences` is present:
- Section with subtle background tint
- "Rationale" — paragraph
- "Alternatives Considered" — bullet list
- "Consequences" — paragraph

- [ ] **Step 5: Verify**

Run: `pnpm run check-types`

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/routes/chunks.$chunkId.tsx
git commit -m "feat(web): display applies-to, file-refs, and decision context on chunk detail"
```

---

### Task 10: Chunk create/edit — new form fields

**Files:**
- Modify: `apps/web/src/routes/chunks.new.tsx` (or wherever the create form is)
- Modify: `apps/web/src/routes/chunks.$chunkId.edit.tsx` (edit form)

- [ ] **Step 1: Read existing form files**

Read the create and edit pages to understand the form structure.

- [ ] **Step 2: Add template selector to create form**

At the top of the create form, add a dropdown that:
- Fetches templates via `api.api.templates.get()`
- On selection, pre-fills content and type fields
- Shows "(none)" as default option

- [ ] **Step 3: Add applies-to repeatable field**

After tags, add an "Applies To" section:
- List of pattern/note input pairs
- "Add pattern" button
- Remove button per row
- On submit, call `PUT /chunks/:id/applies-to` after chunk creation

- [ ] **Step 4: Add file references repeatable field**

Same pattern: path input + optional anchor + relation dropdown. "Add reference" button.
On submit, call `PUT /chunks/:id/file-refs` after chunk creation.

- [ ] **Step 5: Add decision context collapsible**

"Add decision context" link that expands:
- Rationale textarea
- Alternatives — repeatable text inputs
- Consequences textarea

These fields are submitted as part of the chunk body (JSONB columns).

- [ ] **Step 6: Apply same changes to edit form**

The edit form should pre-populate from existing chunk data including applies-to and file-refs (fetched from chunk detail response).

- [ ] **Step 7: Verify**

Run: `pnpm run check-types`

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/routes/chunks.new.tsx apps/web/src/routes/chunks.$chunkId.edit.tsx
git commit -m "feat(web): add template selector, applies-to, file-refs, and decision fields to chunk forms"
```

---

### Task 11: Templates management page

**Files:**
- Create: `apps/web/src/routes/templates.tsx`
- Modify: `apps/web/src/routes/__root.tsx`
- Modify: `apps/web/src/features/nav/mobile-nav.tsx`

- [ ] **Step 1: Read existing management pages for pattern**

Read `apps/web/src/routes/codebases.tsx` or `apps/web/src/routes/tags.tsx`.

- [ ] **Step 2: Create templates page**

`/templates` route with:
- List all templates (query `api.api.templates.get()`)
- Built-in templates: "Built-in" badge, read-only, "Duplicate" button
- User templates: editable, deletable
- Create form: name, description, type dropdown, content textarea
- Delete: confirmation
- Duplicate: fetches template, opens create form pre-filled with content + modified name

- [ ] **Step 3: Add nav links**

Add "Templates" link to nav in `__root.tsx` and `mobile-nav.tsx`.

- [ ] **Step 4: Verify**

Run: `pnpm run check-types`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/templates.tsx apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add templates management page with built-in and custom template support"
```

---

### Task 12: Final verification

- [ ] **Step 1: Run full CI**

Run: `pnpm ci`
Expected: Same baseline as before (only pre-existing errors)

- [ ] **Step 2: Fix any issues**

- [ ] **Step 3: Commit if needed**

```bash
git add -A && git commit -m "fix: resolve CI issues from richer chunks feature"
```
