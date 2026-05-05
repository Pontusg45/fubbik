---
name: fubbik-dev
description: Use when writing code in the fubbik project — provides code patterns for every layer (repository, service, route, schema, test, frontend), file placement rules, and pitfalls that will break your code or produce silent bugs. Covers Effect, Elysia, Drizzle, vitest, base-ui, and Eden treaty patterns specific to this codebase.
---

# Fubbik Development Patterns

Code patterns, file placement, and pitfalls for the fubbik codebase. See CLAUDE.md for the full project reference (architecture, API endpoints, etc.).

## Quick Reference

```bash
pnpm install          # install deps
pnpm dev              # API (port 3000) + web (port 3001)
pnpm test             # vitest across all packages
pnpm run check-types  # tsgo typecheck — must pass before claiming done
```

## Where to Put Things

### New backend feature

```
1. Schema       → packages/db/src/schema/<name>.ts          (Drizzle pgTable)
2. Repository   → packages/db/src/repository/<name>.ts      (wrap queries in dbEffect)
3. Export        → packages/db/src/repository/index.ts       (add export * from "./<name>")
4. Service       → packages/api/src/<name>/service.ts        (compose Effects, business logic)
5. Routes        → packages/api/src/<name>/routes.ts         (Elysia + requireSession)
6. Register      → packages/api/src/index.ts                 (add .use(<name>Routes))
7. Tests         → packages/api/src/<name>/service.test.ts   (vitest + vi.mock)
```

### New frontend page

```
1. Route file    → apps/web/src/routes/<name>.tsx            (createFileRoute)
2. Feature dir   → apps/web/src/features/<name>/             (components, hooks)
3. API calls     → useApiQuery / useMutation with Eden treaty
```

### New CLI command

```
1. Command       → apps/cli/src/commands/<name>.ts           (Commander.js)
2. Register      → apps/cli/src/index.ts                     (program.addCommand)
```

### Context pipeline

```
Low-level retrieval  → packages/api/src/context-for-file/service.ts   (5 strategies)
Resolvers            → packages/api/src/context/resolvers.ts          (plan, concept, files)
Enrichment           → packages/api/src/context/resolvers.ts          (enrichChunks)
Scoring/budgeting    → packages/api/src/context/utils.ts              (scoreChunk, budgetChunks)
Formatting           → packages/api/src/context/formatter.ts          (structured markdown)
CLAUDE.md generation → packages/api/src/context/claude-md.ts          (tag-based export)
Snapshots            → packages/api/src/context/snapshot-service.ts   (frozen context)
```

## Code Patterns

### Repository

Wrap Drizzle queries in `dbEffect` (from `packages/db/src/effect.ts`) to get `Effect<T, DatabaseError>`:

```typescript
import { eq, inArray } from "drizzle-orm";
import { Effect } from "effect";
import { db, dbEffect } from "../index";
import { myTable } from "../schema/my-table";

export function getById(id: string) {
    return dbEffect(() =>
        db.select().from(myTable).where(eq(myTable.id, id))
    );
}

export function create(params: { id: string; name: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(myTable).values(params).returning();
        return created!;
    });
}

// CRITICAL: guard empty arrays — Drizzle generates invalid SQL for IN ()
export function getByIds(ids: string[]) {
    if (ids.length === 0) return Effect.succeed([]);
    return dbEffect(() =>
        db.select().from(myTable).where(inArray(myTable.id, ids))
    );
}
```

### Service

Compose repository Effects. Never throw — use `Effect.fail`:

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

Errors (`packages/api/src/errors.ts`): `NotFoundError`->404, `ValidationError`->400, `AuthError`->401, `AiError`->502. `DatabaseError` is in `packages/db/src/errors.ts`->500.

### Route

Elysia + `requireSession` + `Effect.runPromise`. Validate with Elysia `t`:

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
        { body: t.Object({ name: t.String(), description: t.Optional(t.String()) }) }
    );
```

### Schema

Drizzle `pgTable`. Use `text` for IDs (not `uuid`):

```typescript
import { pgTable, text, timestamp, jsonb, uniqueIndex } from "drizzle-orm/pg-core";
import { user } from "./auth";

export const myTable = pgTable("my_table", {
    id: text("id").primaryKey(),
    userId: text("user_id").notNull().references(() => user.id, { onDelete: "cascade" }),
    name: text("name").notNull(),
    metadata: jsonb("metadata"),
    createdAt: timestamp("created_at").notNull().defaultNow(),
}, (table) => [
    uniqueIndex("my_table_user_name_idx").on(table.userId, table.name),
]);
```

Export from `packages/db/src/schema/index.ts` after creating.

### Test

`vi.mock` MUST come before imports. Mock returns use `Effect.succeed`/`Effect.fail`:

```typescript
import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@fubbik/db/repository", () => ({
    getById: vi.fn(),
    listAll: vi.fn(),
}));

import { getById, listAll } from "@fubbik/db/repository";
import { myService } from "./service";

describe("myService", () => {
    beforeEach(() => { vi.clearAllMocks(); });

    it("returns item when found", async () => {
        const mock = getById as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed({ id: "1", name: "test" }));
        const result = await Effect.runPromise(myService.getById("1"));
        expect(result.name).toBe("test");
    });

    it("fails with NotFoundError when missing", async () => {
        (getById as ReturnType<typeof vi.fn>).mockReturnValue(Effect.succeed(null));
        await expect(Effect.runPromise(myService.getById("x"))).rejects.toThrow();
    });
});
```

### Frontend page

```tsx
import { createFileRoute } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useApiQuery, api, unwrapEden } from "~/lib/api";
import { PageContainer, PageHeader, PageLoading, PageEmpty } from "~/components/ui/page";

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
    const query = useApiQuery<Thing[]>({
        queryKey: ["things"],
        queryFn: () => api.api.things.get(),
        fallback: [],
    });
    const createMutation = useMutation({
        mutationFn: async (body: { name: string }) =>
            unwrapEden(await api.api.things.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["things"] });
            toast.success("Created");
        },
    });

    if (query.isLoading) return <PageLoading />;
    if (query.data.length === 0) return <PageEmpty title="No things yet" />;
    return <PageContainer><PageHeader title="Things" /></PageContainer>;
}
```

## Pitfalls

### Will break your code

| Pitfall | Correct pattern |
|---------|----------------|
| Using `asChild` on base-ui components | Use `render` prop: `<DropdownMenuTrigger render={<button>...</button>} />` |
| Using arktype for route validation | Use Elysia `t`: `t.Object({ name: t.String() })` |
| `inArray()` with empty array | Guard: `if (ids.length === 0) return Effect.succeed([])` |
| `vi.mock` after imports | Place `vi.mock(...)` BEFORE any imports of mocked modules |
| Throwing errors in services | Use `Effect.fail(new NotFoundError({ resource: "X" }))` |
| Using `uuid` column type in schema | Use `text("id").primaryKey()` — IDs via `crypto.randomUUID()` |
| `DropdownMenuSeparator`/`Label` as base-ui | Use plain HTML `<div>` elements |

### Will produce silent bugs

| Pitfall | Correct pattern |
|---------|----------------|
| Ollama-dependent code without fallback | Wrap in `Effect.catchAll(() => Effect.succeed([]))` |
| Manually applying feature overlays | Pipeline applies them automatically via `resolveFeatureOverlays` |
| Reimplementing token estimation | Use `estimateTokens()` from `context/utils.ts` (js-tiktoken) |
| Assuming tag filter is AND | Default is OR. Pass `tagMode: "all"` for AND semantics |
| Pre-normalizing paths for `globMatch` | `globMatch` normalizes internally (strips `./`, `/`, collapses `//`) |

### Will waste time

| Pitfall | Instead |
|---------|---------|
| Reading Swagger for context endpoints | Read CLAUDE.md Context Pipeline section |
| Writing custom scoring | Use `scoreChunk` from `context/utils.ts` |
| Mocking individual Drizzle calls | Mock at `@fubbik/db/repository` boundary |
| Missing import from `@fubbik/db/repository` | Check `packages/db/src/repository/index.ts` — all use `export *` |
