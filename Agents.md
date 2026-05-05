# Agent Onboarding Guide

Patterns, examples, and pitfalls for AI agents working on fubbik. Read this before writing code.

See [CLAUDE.md](./CLAUDE.md) for the full project reference (architecture, API endpoints, schema, etc.).

## Quick Start

**What this is:** A knowledge management system for codebases. Chunks = units of knowledge. Connections = links between them. Codebases = projects. Plans = work items.

**Before changing anything:**

```bash
pnpm install          # install deps
pnpm dev              # start API (port 3000) + web (port 3001)
pnpm test             # run vitest across all packages
pnpm run check-types  # typecheck (uses tsgo)
```

**Verify before claiming done:**

```bash
pnpm run check-types  # must pass — 7/7 packages
pnpm test             # must pass — ~325 tests across 5 suites
```

---

## Where to Put Things

### Adding a new backend feature

```
1. Schema       → packages/db/src/schema/<name>.ts          (Drizzle pgTable)
2. Repository   → packages/db/src/repository/<name>.ts      (wrap queries in dbEffect)
3. Export        → packages/db/src/repository/index.ts       (add export * from "./<name>")
4. Service       → packages/api/src/<name>/service.ts        (compose Effects, business logic)
5. Routes        → packages/api/src/<name>/routes.ts         (Elysia + requireSession)
6. Register      → packages/api/src/index.ts                 (add .use(<name>Routes))
7. Tests         → packages/api/src/<name>/service.test.ts   (vitest + vi.mock)
```

### Adding a new frontend page

```
1. Route file    → apps/web/src/routes/<name>.tsx            (createFileRoute)
2. Feature dir   → apps/web/src/features/<name>/             (components, hooks)
3. API calls     → use useApiQuery / useMutation with Eden treaty
```

### Adding a CLI command

```
1. Command file  → apps/cli/src/commands/<name>.ts           (Commander.js)
2. Register      → apps/cli/src/index.ts                     (program.addCommand)
```

### Modifying the context pipeline

```
Low-level retrieval  → packages/api/src/context-for-file/service.ts   (5 strategies)
Resolvers            → packages/api/src/context/resolvers.ts          (plan, concept, files)
Enrichment           → packages/api/src/context/resolvers.ts          (enrichChunks)
Scoring/budgeting    → packages/api/src/context/utils.ts              (scoreChunk, budgetChunks)
Formatting           → packages/api/src/context/formatter.ts          (structured markdown)
CLAUDE.md generation → packages/api/src/context/claude-md.ts          (tag-based export)
Snapshots            → packages/api/src/context/snapshot-service.ts   (frozen context)
```

---

## Code Patterns

### Repository (data access layer)

Every DB function wraps its Drizzle query in `dbEffect` (defined in `packages/db/src/effect.ts`). This converts async DB calls into `Effect<T, DatabaseError>`.

```typescript
import { eq, inArray } from "drizzle-orm";
import { Effect } from "effect";
import { db, dbEffect } from "../index";
import { myTable } from "../schema/my-table";

// Simple select
export function getById(id: string) {
    return dbEffect(() =>
        db.select().from(myTable).where(eq(myTable.id, id))
    );
}

// Insert with returning
export function create(params: { id: string; name: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(myTable).values(params).returning();
        return created!;
    });
}

// CRITICAL: guard empty arrays to avoid invalid SQL `IN ()`
export function getByIds(ids: string[]) {
    if (ids.length === 0) return Effect.succeed([]);
    return dbEffect(() =>
        db.select().from(myTable).where(inArray(myTable.id, ids))
    );
}
```

**Reference files:** `packages/db/src/repository/tag-new.ts`, `packages/db/src/repository/template.ts`

### Service (business logic layer)

Services compose repository Effects and introduce typed business errors. Never throw — use `Effect.fail`:

```typescript
import { Effect } from "effect";
import { getById, update } from "@fubbik/db/repository";
import { NotFoundError, ValidationError } from "../errors";

export function updateThing(id: string, userId: string, body: { name?: string }) {
    return getById(id).pipe(
        Effect.flatMap(found =>
            found
                ? Effect.succeed(found)
                : Effect.fail(new NotFoundError({ resource: "Thing" }))
        ),
        Effect.flatMap(existing => update(id, userId, body))
    );
}
```

Error classes (from `packages/api/src/errors.ts`):

```typescript
import { Data } from "effect";

export class NotFoundError extends Data.TaggedError("NotFoundError")<{ resource: string }> {}
export class ValidationError extends Data.TaggedError("ValidationError")<{ message: string }> {}
export class AuthError extends Data.TaggedError("AuthError")<{}> {}
export class AiError extends Data.TaggedError("AiError")<{ cause?: unknown }> {}
// DatabaseError is in packages/db/src/errors.ts (separate package)
```

The global error handler in `packages/api/src/index.ts` maps `_tag` to HTTP status codes:
`NotFoundError`->404, `AuthError`->401, `ValidationError`->400, `AiError`->502, `DatabaseError`->500

**Reference files:** `packages/api/src/tags/service-new.ts`, `packages/api/src/codebases/service.ts`

### Routes (HTTP layer)

Routes use Elysia with `requireSession` for auth and `Effect.runPromise` to bridge into the Effect world:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as service from "./service";

export const myRoutes = new Elysia()
    .get("/things", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => service.list(session.user.id))
            )
        )
    )
    .post("/things", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => service.create(session.user.id, ctx.body)),
                Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
            )
        ),
        {
            body: t.Object({
                name: t.String(),
                description: t.Optional(t.String()),
            })
        }
    )
    .delete("/things/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => service.remove(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        ),
        { params: t.Object({ id: t.String() }) }
    );
```

Validation uses Elysia's `t` schema (NOT arktype, NOT zod). Schema goes in the route options object as `body`, `query`, or `params`.

**Reference files:** `packages/api/src/tags/routes.ts`, `packages/api/src/scope-keys/routes.ts`

### Schema (database tables)

Drizzle ORM schemas use `pgTable` from `drizzle-orm/pg-core`:

```typescript
import { pgTable, text, timestamp, jsonb, boolean, integer, uniqueIndex } from "drizzle-orm/pg-core";
import { relations, sql } from "drizzle-orm";
import { user } from "./auth";

export const myTable = pgTable("my_table", {
    id: text("id").primaryKey(),                // text IDs, NOT uuid
    userId: text("user_id").notNull().references(() => user.id, { onDelete: "cascade" }),
    name: text("name").notNull(),
    metadata: jsonb("metadata"),                // JSONB for flexible data
    tags: text("tags").array(),                 // PostgreSQL arrays
    isActive: boolean("is_active").notNull().default(true),
    priority: integer("priority").notNull().default(0),
    createdAt: timestamp("created_at").notNull().defaultNow(),
    updatedAt: timestamp("updated_at").notNull().defaultNow(),
}, (table) => [
    uniqueIndex("my_table_user_name_idx").on(table.userId, table.name),
]);

// Relations for Drizzle relational queries
export const myTableRelations = relations(myTable, ({ one }) => ({
    user: one(user, { fields: [myTable.userId], references: [user.id] }),
}));
```

After creating a schema, export it from `packages/db/src/schema/index.ts`.

**Reference files:** `packages/db/src/schema/template.ts`, `packages/db/src/schema/scope-key.ts`

### Tests

Tests use vitest. `vi.mock` calls MUST come before imports of the mocked modules. Mock return values use `Effect.succeed`/`Effect.fail`:

```typescript
import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

// Mocks MUST be before imports of mocked modules
vi.mock("@fubbik/db/repository", () => ({
    getById: vi.fn(),
    listAll: vi.fn(),
}));

vi.mock("../ollama/client", () => ({
    generateQueryEmbedding: vi.fn(),
}));

// Now import the mocked symbols
import { getById, listAll } from "@fubbik/db/repository";
import { generateQueryEmbedding } from "../ollama/client";
import { myService } from "./service";

describe("myService", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("returns item when found", async () => {
        const mock = getById as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed({ id: "1", name: "test" }));

        const result = await Effect.runPromise(myService.getById("1"));

        expect(result.name).toBe("test");
        expect(mock).toHaveBeenCalledWith("1");
    });

    it("fails with NotFoundError when missing", async () => {
        const mock = getById as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed(null));

        await expect(
            Effect.runPromise(myService.getById("missing"))
        ).rejects.toThrow();
    });

    it("handles Ollama being unavailable", async () => {
        const embMock = generateQueryEmbedding as ReturnType<typeof vi.fn>;
        embMock.mockReturnValue(Effect.fail(new Error("Ollama down")));

        // Service should handle this gracefully (not throw)
        const result = await Effect.runPromise(myService.search("query"));
        expect(result).toEqual([]);
    });
});
```

**Reference files:** `packages/api/src/context-for-file/service.test.ts`, `packages/api/src/context-export/service.test.ts`

### Frontend pages

```tsx
import { createFileRoute } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useApiQuery, api, unwrapEden } from "~/lib/api";
import { PageContainer, PageHeader, PageLoading, PageEmpty } from "~/components/ui/page";
import { toast } from "sonner";

export const Route = createFileRoute("/things")({
    component: ThingsPage,
    beforeLoad: async () => {
        let session = null;
        try { session = await getUser(); } catch {}
        return { session };
    },
});

function ThingsPage() {
    const queryClient = useQueryClient();

    // Fetching data — Eden treaty + TanStack Query
    const query = useApiQuery<Thing[]>({
        queryKey: ["things"],
        queryFn: () => api.api.things.get(),
        fallback: [],
    });

    // Mutations — always unwrapEden, invalidate queries on success
    const createMutation = useMutation({
        mutationFn: async (body: { name: string }) =>
            unwrapEden(await api.api.things.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["things"] });
            toast.success("Created");
        },
        onError: (err: unknown) =>
            toast.error(err instanceof Error ? err.message : "Failed"),
    });

    if (query.isLoading) return <PageLoading />;
    if (query.data.length === 0) return <PageEmpty title="No things yet" />;

    return (
        <PageContainer>
            <PageHeader title="Things" />
            {/* Use render prop for base-ui components, NOT asChild */}
            <DropdownMenuTrigger
                render={<button className="...">Actions</button>}
            />
        </PageContainer>
    );
}
```

**Reference files:** `apps/web/src/routes/tags.tsx`, `apps/web/src/routes/codebases.tsx`

---

## Pitfalls & Gotchas

### Will break your code

- **base-ui uses `render` prop, NOT `asChild`**: `<DropdownMenuTrigger render={<button>...</button>} />` — Radix's `asChild` does not exist here
- **Elysia `t` for validation, NOT arktype**: Arktype was removed from the project. Route body/query schemas use `t.Object`, `t.String`, `t.Optional`, etc. from `elysia`
- **Empty array guard in repositories**: Always check `if (ids.length === 0) return Effect.succeed([])` before `inArray()` queries — Drizzle generates invalid SQL for empty `IN ()` clauses
- **`vi.mock` must come before imports**: Vitest hoists `vi.mock` calls but only if they appear before the import statements of the mocked modules in source order
- **Effect errors are not exceptions**: Service functions fail via `Effect.fail(new SomeError(...))`, not `throw`. The global error handler in `index.ts` extracts `_tag` from `FiberFailure` and maps to HTTP status codes
- **Schema uses `text` for IDs, not `uuid`**: Primary keys are `text("id")` with IDs generated via `crypto.randomUUID()` at the application layer
- **`DropdownMenuSeparator` and `DropdownMenuLabel`**: Use plain HTML `<div>` elements, NOT base-ui primitives — they require a `Menu.Group` context that doesn't exist in dropdown usage

### Will produce wrong results silently

- **Embeddings require Ollama**: Semantic search, enrichment, and duplicate detection need Ollama running locally. All Ollama-dependent code must gracefully fall back using `Effect.catchAll(() => Effect.succeed([]))` — never let it crash
- **Active features affect context**: The context pipeline applies feature deltas automatically via `resolveFeatureOverlays` in the enrichment step. Don't manually apply overlays in context-consuming code
- **Token estimation uses js-tiktoken**: `estimateTokens()` in `context/utils.ts` uses BPE tokenization with `chars/4` fallback — don't implement your own approximation
- **Tag filtering defaults to OR**: `listChunks({ tags: ["a", "b"] })` returns chunks with tag "a" OR "b". Use `tagMode: "all"` for AND semantics
- **Global chunks appear in all codebases**: When filtering by `codebaseId`, chunks with no codebase association (global) are always included — this is intentional
- **Path normalization in glob matching**: `globMatch` normalizes both pattern and path (strips leading `./`, `/`, collapses `//`). Don't pre-normalize inputs

### Will waste your time

- **Don't read the Swagger docs for context endpoints**: The `/docs` endpoint exists but the context system documentation in CLAUDE.md is more accurate and complete
- **Don't write scoring logic**: `scoreChunk` in `context/utils.ts` is the single source of truth for chunk scoring. Strategy bonuses (+20 file-ref, +10 applies-to, +5 semantic, +3 dependency, +2 connected) are in `context-for-file/service.ts`
- **Don't mock deeply**: Mock at the `@fubbik/db/repository` boundary, not individual Drizzle calls. The repository is the seam between DB and business logic
- **Check `packages/db/src/repository/index.ts`**: All repository functions are re-exported via `export *`. If your import from `@fubbik/db/repository` can't find a function, it's probably not exported from the module's own file, not a missing re-export
