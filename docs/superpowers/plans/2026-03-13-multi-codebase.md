# Multi-Codebase Support Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow chunks to be organized per-codebase, with CLI auto-detection and a web UI codebase switcher.

**Architecture:** New `codebase` table + `chunk_codebase` join table in `packages/db`. New codebase repository, service, and routes following the existing Repository → Service → Route pattern with Effect. CLI gets a `codebase` command group. Web UI gets a codebase switcher in the nav and a `/codebases` management page.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router/Query, Commander.js, Vitest

**Spec:** `docs/superpowers/specs/2026-03-13-multi-codebase-design.md`

---

## File Structure

### New files
- `packages/db/src/schema/codebase.ts` — codebase + chunk_codebase schema
- `packages/db/src/repository/codebase.ts` — codebase CRUD + chunk association queries
- `packages/db/src/__tests__/codebase.test.ts` — schema tests
- `packages/api/src/codebases/service.ts` — codebase business logic
- `packages/api/src/codebases/service.test.ts` — service route tests
- `packages/api/src/codebases/routes.ts` — Elysia routes
- `packages/api/src/codebases/normalize-url.ts` — git remote URL normalization
- `packages/api/src/codebases/normalize-url.test.ts` — normalization tests
- `apps/cli/src/commands/codebase.ts` — CLI codebase command group
- `apps/cli/src/lib/detect-codebase.ts` — git remote detection + codebase resolution
- `apps/web/src/features/codebases/codebase-switcher.tsx` — nav dropdown
- `apps/web/src/features/codebases/use-active-codebase.ts` — hook for reading/setting active codebase from URL
- `apps/web/src/routes/codebases.tsx` — codebase management page

### Modified files
- `packages/db/src/schema/index.ts` — add codebase export
- `packages/db/src/repository/index.ts` — add codebase export
- `packages/db/src/repository/chunk.ts` — add `codebaseId` filtering to `listChunks`
- `packages/db/src/repository/graph.ts` — add codebase-scoped graph query
- `packages/api/src/index.ts` — register codebase routes
- `packages/api/src/chunks/routes.ts` — add `codebaseId` query param
- `packages/api/src/chunks/service.ts` — pass `codebaseId` to repository, handle `codebaseIds` on create/update
- `packages/api/src/graph/routes.ts` — add `codebaseId` query param
- `packages/api/src/graph/service.ts` — pass codebaseId to graph repo
- `apps/cli/src/index.ts` — register codebase command
- `apps/web/src/routes/__root.tsx` — add codebase switcher to nav

---

## Chunk 1: Database Schema + Repository

### Task 1: Codebase schema

**Files:**
- Create: `packages/db/src/schema/codebase.ts`
- Create: `packages/db/src/__tests__/codebase.test.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Write schema test**

```typescript
// packages/db/src/__tests__/codebase.test.ts
import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { codebase, chunkCodebase } from "../schema/codebase";

describe("codebase table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(codebase);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("name");
        expect(columns).toHaveProperty("remoteUrl");
        expect(columns).toHaveProperty("localPaths");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("chunkCodebase table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkCodebase);
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("codebaseId");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm vitest run src/__tests__/codebase.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Write codebase schema**

```typescript
// packages/db/src/schema/codebase.ts
import { relations, sql } from "drizzle-orm";
import { index, jsonb, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";

export const codebase = pgTable(
    "codebase",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        remoteUrl: text("remote_url"),
        localPaths: jsonb("local_paths").$type<string[]>().notNull().default([]),
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
        uniqueIndex("codebase_user_name_idx").on(table.userId, table.name),
        uniqueIndex("codebase_user_remote_idx")
            .on(table.userId, table.remoteUrl)
            .where(sql`"remote_url" IS NOT NULL`),
        index("codebase_userId_idx").on(table.userId)
    ]
);

export const chunkCodebase = pgTable(
    "chunk_codebase",
    {
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.chunkId, table.codebaseId] }),
        index("chunk_codebase_chunkId_idx").on(table.chunkId)
    ]
);

export const codebaseRelations = relations(codebase, ({ one, many }) => ({
    user: one(user, { fields: [codebase.userId], references: [user.id] }),
    chunkCodebases: many(chunkCodebase)
}));

export const chunkCodebaseRelations = relations(chunkCodebase, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkCodebase.chunkId], references: [chunk.id] }),
    codebase: one(codebase, { fields: [chunkCodebase.codebaseId], references: [codebase.id] })
}));
```

Note: If `uniqueIndex().where()` is not supported in the project's Drizzle version, fall back to the service-layer duplicate check in `createCodebase` (which calls `getCodebaseByRemoteUrl` before insert — see Task 7).

- [ ] **Step 4: Export from schema index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./codebase";
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/db && pnpm vitest run src/__tests__/codebase.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/codebase.ts packages/db/src/schema/index.ts packages/db/src/__tests__/codebase.test.ts
git commit -m "feat(db): add codebase and chunk_codebase schema"
```

---

### Task 2: URL normalization utility

**Files:**
- Create: `packages/api/src/codebases/normalize-url.ts`
- Create: `packages/api/src/codebases/normalize-url.test.ts`

- [ ] **Step 1: Write normalization tests**

```typescript
// packages/api/src/codebases/normalize-url.test.ts
import { describe, expect, it } from "vitest";

import { normalizeGitUrl } from "./normalize-url";

describe("normalizeGitUrl", () => {
    it("strips .git suffix", () => {
        expect(normalizeGitUrl("https://github.com/user/repo.git")).toBe("github.com/user/repo");
    });

    it("strips trailing slashes", () => {
        expect(normalizeGitUrl("https://github.com/user/repo/")).toBe("github.com/user/repo");
    });

    it("normalizes SSH to path form", () => {
        expect(normalizeGitUrl("git@github.com:user/repo.git")).toBe("github.com/user/repo");
    });

    it("normalizes HTTPS", () => {
        expect(normalizeGitUrl("https://github.com/user/repo")).toBe("github.com/user/repo");
    });

    it("normalizes HTTP", () => {
        expect(normalizeGitUrl("http://github.com/user/repo")).toBe("github.com/user/repo");
    });

    it("handles ssh:// protocol", () => {
        expect(normalizeGitUrl("ssh://git@github.com/user/repo.git")).toBe("github.com/user/repo");
    });

    it("SSH and HTTPS for same repo produce same result", () => {
        const ssh = normalizeGitUrl("git@github.com:user/repo.git");
        const https = normalizeGitUrl("https://github.com/user/repo.git");
        expect(ssh).toBe(https);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/codebases/normalize-url.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement normalizeGitUrl**

```typescript
// packages/api/src/codebases/normalize-url.ts
export function normalizeGitUrl(url: string): string {
    let normalized = url.trim();

    // Handle SSH format: git@host:user/repo
    const sshMatch = normalized.match(/^(?:ssh:\/\/)?git@([^:/]+)[:/](.+)$/);
    if (sshMatch) {
        normalized = `${sshMatch[1]}/${sshMatch[2]}`;
    } else {
        // Strip protocol (https://, http://, git://)
        normalized = normalized.replace(/^[a-z+]+:\/\//, "");
        // Strip user@ prefix if present
        normalized = normalized.replace(/^[^@]+@/, "");
    }

    // Strip .git suffix
    normalized = normalized.replace(/\.git$/, "");
    // Strip trailing slashes
    normalized = normalized.replace(/\/+$/, "");

    return normalized;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/api && pnpm vitest run src/codebases/normalize-url.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/codebases/normalize-url.ts packages/api/src/codebases/normalize-url.test.ts
git commit -m "feat(api): add git remote URL normalization utility"
```

---

### Task 3: Codebase repository

**Files:**
- Create: `packages/db/src/repository/codebase.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write codebase repository**

```typescript
// packages/db/src/repository/codebase.ts
import { and, eq, inArray, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { codebase, chunkCodebase } from "../schema/codebase";

export interface CreateCodebaseParams {
    id: string;
    name: string;
    remoteUrl?: string;
    localPaths?: string[];
    userId: string;
}

export function createCodebase(params: CreateCodebaseParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(codebase)
                .values({
                    id: params.id,
                    name: params.name,
                    remoteUrl: params.remoteUrl ?? null,
                    localPaths: params.localPaths ?? [],
                    userId: params.userId
                })
                .returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebaseById(codebaseId: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(codebase.id, codebaseId)];
            if (userId) conditions.push(eq(codebase.userId, userId));
            const [found] = await db.select().from(codebase).where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listCodebases(userId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(codebase).where(eq(codebase.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebaseByRemoteUrl(remoteUrl: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(codebase)
                .where(and(eq(codebase.remoteUrl, remoteUrl), eq(codebase.userId, userId)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebaseByLocalPath(localPath: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(codebase)
                .where(
                    and(
                        sql`${codebase.localPaths} @> ${JSON.stringify([localPath])}::jsonb`,
                        eq(codebase.userId, userId)
                    )
                );
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateCodebaseParams {
    name?: string;
    remoteUrl?: string | null;
    localPaths?: string[];
}

export function updateCodebase(codebaseId: string, userId: string, params: UpdateCodebaseParams) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(codebase)
                .set({
                    ...(params.name !== undefined && { name: params.name }),
                    ...(params.remoteUrl !== undefined && { remoteUrl: params.remoteUrl }),
                    ...(params.localPaths !== undefined && { localPaths: params.localPaths })
                })
                .where(and(eq(codebase.id, codebaseId), eq(codebase.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteCodebase(codebaseId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(codebase)
                .where(and(eq(codebase.id, codebaseId), eq(codebase.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function countChunksInCodebase(codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkCodebase)
                .where(eq(chunkCodebase.codebaseId, codebaseId));
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function setChunkCodebases(chunkId: string, codebaseIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkCodebase).where(eq(chunkCodebase.chunkId, chunkId));
            if (codebaseIds.length === 0) return [];
            return db
                .insert(chunkCodebase)
                .values(codebaseIds.map(codebaseId => ({ chunkId, codebaseId })))
                .onConflictDoNothing()
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebasesForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: codebase.id,
                    name: codebase.name
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id))
                .where(eq(chunkCodebase.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getCodebasesForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([]);
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    chunkId: chunkCodebase.chunkId,
                    codebaseId: codebase.id,
                    codebaseName: codebase.name
                })
                .from(chunkCodebase)
                .innerJoin(codebase, eq(chunkCodebase.codebaseId, codebase.id))
                .where(inArray(chunkCodebase.chunkId, chunkIds)),
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./codebase";
```

- [ ] **Step 3: Run existing tests to verify nothing breaks**

Run: `cd packages/db && pnpm vitest run`
Expected: All existing tests PASS

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/codebase.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add codebase repository with CRUD and chunk association queries"
```

---

### Task 4: Extend chunk repository with codebase filtering

**Files:**
- Modify: `packages/db/src/repository/chunk.ts`

- [ ] **Step 1: Read current `listChunks` implementation**

Read `packages/db/src/repository/chunk.ts` and find the `listChunks` function.

- [ ] **Step 2: Add `codebaseId` parameter to `ListChunksParams`**

Add `codebaseId?: string` to the params interface. When set, the query should return:
- Chunks that have a row in `chunk_codebase` with the given `codebaseId`
- Plus global chunks (chunks with no rows in `chunk_codebase` at all)

This requires importing `chunkCodebase` from the codebase schema and adding a LEFT JOIN + WHERE condition:

```typescript
// Add to imports:
import { chunkCodebase } from "../schema/codebase";

// In listChunks, when codebaseId is provided, add this condition:
// WHERE chunk.id IN (SELECT chunk_id FROM chunk_codebase WHERE codebase_id = ?)
//    OR chunk.id NOT IN (SELECT chunk_id FROM chunk_codebase)
```

The exact implementation depends on the current query structure. Use subqueries:
```typescript
if (params.codebaseId) {
    const inCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase).where(eq(chunkCodebase.codebaseId, params.codebaseId));
    const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
    conditions.push(or(sql`${chunk.id} IN (${inCodebase})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`));
}
```

- [ ] **Step 3: Run tests**

Run: `cd packages/db && pnpm vitest run`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/chunk.ts
git commit -m "feat(db): add codebaseId filtering to listChunks repository"
```

---

### Task 5: Extend graph repository with codebase scoping

**Files:**
- Modify: `packages/db/src/repository/graph.ts`

- [ ] **Step 1: Add codebaseId parameter to `getAllChunksMeta`**

Add optional `codebaseId` parameter. When set, return only:
- Chunks in the given codebase
- Global chunks connected to codebase chunks

```typescript
// Add to imports
import { chunkCodebase } from "../schema/codebase";

// In getAllChunksMeta, add codebaseId?: string param
// When codebaseId is set, filter to chunks in codebase + global chunks
export function getAllChunksMeta(userId?: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: () => {
            let query = db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    summary: chunk.summary,
                    createdAt: chunk.createdAt
                })
                .from(chunk);

            const conditions = [];
            if (userId) conditions.push(eq(chunk.userId, userId));
            if (codebaseId) {
                const inCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase).where(eq(chunkCodebase.codebaseId, codebaseId));
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(or(sql`${chunk.id} IN (${inCodebase})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`));
            }
            if (conditions.length > 0) return query.where(and(...conditions));
            return query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Run tests**

Run: `cd packages/db && pnpm vitest run`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/graph.ts
git commit -m "feat(db): add codebase scoping to graph repository"
```

---

### Task 6: Push schema to database

- [ ] **Step 1: Push schema changes**

Run: `pnpm db:push`
Expected: New tables `codebase` and `chunk_codebase` created

- [ ] **Step 2: Commit** (if drizzle generates migration files)

```bash
git add -A && git commit -m "chore(db): push codebase schema migration"
```

---

## Chunk 2: API Service + Routes

### Task 7: Codebase service

**Files:**
- Create: `packages/api/src/codebases/service.ts`

- [ ] **Step 1: Write codebase service**

```typescript
// packages/api/src/codebases/service.ts
import {
    countChunksInCodebase,
    createCodebase as createCodebaseRepo,
    deleteCodebase as deleteCodebaseRepo,
    getCodebaseById,
    getCodebaseByLocalPath,
    getCodebaseByRemoteUrl,
    listCodebases as listCodebasesRepo,
    setChunkCodebases,
    updateCodebase as updateCodebaseRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";
import { normalizeGitUrl } from "./normalize-url";

export function listCodebases(userId: string) {
    return listCodebasesRepo(userId);
}

export function getCodebase(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" }))
        )
    );
}

export function createCodebase(
    userId: string,
    body: { name: string; remoteUrl?: string; localPaths?: string[] }
) {
    const id = crypto.randomUUID();
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : undefined;

    return Effect.suspend(() => {
        if (!remoteUrl) return Effect.void;
        return getCodebaseByRemoteUrl(remoteUrl, userId).pipe(
            Effect.flatMap(existing =>
                existing
                    ? Effect.fail(new ValidationError({ message: "A codebase with this remote URL already exists" }))
                    : Effect.void
            )
        );
    }).pipe(
        Effect.flatMap(() =>
            createCodebaseRepo({
                id,
                name: body.name,
                remoteUrl,
                localPaths: body.localPaths ?? [],
                userId
            })
        )
    );
}

export function updateCodebase(
    codebaseId: string,
    userId: string,
    body: { name?: string; remoteUrl?: string | null; localPaths?: string[] }
) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" }))
        ),
        Effect.flatMap(() => {
            const remoteUrl = body.remoteUrl === null
                ? null
                : body.remoteUrl !== undefined
                  ? normalizeGitUrl(body.remoteUrl)
                  : undefined;
            return updateCodebaseRepo(codebaseId, userId, {
                name: body.name,
                remoteUrl,
                localPaths: body.localPaths
            });
        })
    );
}

export function deleteCodebase(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" }))
        ),
        Effect.flatMap(() => deleteCodebaseRepo(codebaseId, userId))
    );
}

export function detectCodebase(userId: string, query: { remoteUrl?: string; localPath?: string }) {
    if (query.remoteUrl) {
        const normalized = normalizeGitUrl(query.remoteUrl);
        return getCodebaseByRemoteUrl(normalized, userId);
    }
    if (query.localPath) {
        return getCodebaseByLocalPath(query.localPath, userId);
    }
    return Effect.succeed(null);
}

export function getCodebaseChunkCount(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" }))
        ),
        Effect.flatMap(() => countChunksInCodebase(codebaseId))
    );
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/codebases/service.ts
git commit -m "feat(api): add codebase service with CRUD, detection, and URL normalization"
```

---

### Task 8: Codebase routes

**Files:**
- Create: `packages/api/src/codebases/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Write codebase routes**

```typescript
// packages/api/src/codebases/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as codebaseService from "./service";

export const codebaseRoutes = new Elysia()
    // Static routes BEFORE dynamic :id route
    .get(
        "/codebases/detect",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => codebaseService.detectCodebase(session.user.id, ctx.query))
                )
            ),
        {
            query: t.Object({
                remoteUrl: t.Optional(t.String()),
                localPath: t.Optional(t.String())
            })
        }
    )
    .get("/codebases", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => codebaseService.listCodebases(session.user.id))
            )
        )
    )
    .post(
        "/codebases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => codebaseService.createCodebase(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 }),
                remoteUrl: t.Optional(t.String({ maxLength: 500 })),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .get("/codebases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => codebaseService.getCodebase(ctx.params.id, session.user.id))
            )
        )
    )
    .patch(
        "/codebases/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        codebaseService.updateCodebase(ctx.params.id, session.user.id, ctx.body)
                    )
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                remoteUrl: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .delete("/codebases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => codebaseService.deleteCodebase(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

- [ ] **Step 2: Register routes in API index**

Add to `packages/api/src/index.ts`:

```typescript
// Add import
import { codebaseRoutes } from "./codebases/routes";

// Add .use(codebaseRoutes) after .use(tagTypeRoutes)
```

- [ ] **Step 3: Write route tests**

```typescript
// packages/api/src/codebases/service.test.ts
import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

describe("Codebase routes", () => {
    it("GET /api/codebases returns 200", async () => {
        const { status } = await client.api.codebases.get();
        // In dev mode, should succeed with dev session
        expect(status).toBe(200);
    });

    it("GET /api/codebases/detect returns 200", async () => {
        const { status } = await client.api.codebases.detect.get({
            query: { remoteUrl: "https://github.com/test/repo" }
        });
        expect(status).toBe(200);
    });
});
```

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/codebases/routes.ts packages/api/src/codebases/service.test.ts packages/api/src/index.ts
git commit -m "feat(api): add codebase CRUD routes and register in API"
```

---

### Task 9: Add codebaseId to chunk routes and service

**Files:**
- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Add `codebaseId` to GET /chunks query param**

In `packages/api/src/chunks/routes.ts`, add to the query schema:
```typescript
codebaseId: t.Optional(t.String()),
```

Also add `global` param for filtering to global-only chunks:
```typescript
global: t.Optional(t.String()),
```

- [ ] **Step 2: Add `codebaseIds` to POST /chunks and PATCH /chunks/:id body**

In the POST body schema:
```typescript
codebaseIds: t.Optional(t.Array(t.String(), { maxItems: 20 })),
```

Same for the PATCH body.

- [ ] **Step 3: Update chunk service to pass codebaseId and handle codebaseIds**

In `packages/api/src/chunks/service.ts`:

```typescript
// Add import
import { getCodebasesForChunk, setChunkCodebases } from "@fubbik/db/repository";
```

For `listChunks`: pass `query.codebaseId` through to the repository call. If `query.global === "true"`, pass a `globalOnly: true` flag to the repo (filter to chunks with no rows in `chunk_codebase`).

For `createChunk`: add `codebaseIds?: string[]` to the body param. After creating the chunk, add:
```typescript
Effect.tap(() => {
    if (body.codebaseIds && body.codebaseIds.length > 0) {
        return setChunkCodebases(id, body.codebaseIds);
    }
    return Effect.void;
}),
```

For `updateChunk`: similar pattern — if `body.codebaseIds` is provided, call `setChunkCodebases`.

For `getChunkDetail`: after fetching the chunk and connections, also fetch codebase associations:
```typescript
Effect.flatMap(found =>
    Effect.all({
        chunk: Effect.succeed(found),
        connections: getChunkConnections(chunkId),
        codebases: getCodebasesForChunk(chunkId)
    })
)
```

This ensures `GET /chunks/:id` returns which codebases a chunk belongs to.

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/routes.ts packages/api/src/chunks/service.ts
git commit -m "feat(api): add codebase filtering and associations to chunk endpoints"
```

---

### Task 10: Add codebaseId to graph routes and service

**Files:**
- Modify: `packages/api/src/graph/routes.ts`
- Modify: `packages/api/src/graph/service.ts`

- [ ] **Step 1: Add `codebaseId` to GET /graph query param**

In `packages/api/src/graph/routes.ts`:
```typescript
export const graphRoutes = new Elysia().get(
    "/graph",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => graphService.getUserGraph(session.user.id, ctx.query.codebaseId))
            )
        ),
    {
        query: t.Object({
            codebaseId: t.Optional(t.String())
        })
    }
);
```

- [ ] **Step 2: Update graph service to pass `codebaseId` through to `getAllChunksMeta`**

In `packages/api/src/graph/service.ts`:
```typescript
export function getUserGraph(userId?: string, codebaseId?: string) {
    return Effect.all(
        {
            chunks: getAllChunksMeta(userId, codebaseId),
            connections: getAllConnectionsForUser(userId),
            chunkTags: getAllTagsWithTypes(userId),
            tagTypes: getTagTypesForGraph(userId)
        },
        { concurrency: "unbounded" }
    );
}
```

Note: `getAllConnectionsForUser` stays unscoped — we fetch all user connections, then the frontend can filter to only show edges between visible chunks. This avoids a complex codebase-aware connection query.

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm vitest run`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/graph/routes.ts packages/api/src/graph/service.ts
git commit -m "feat(api): add codebase scoping to graph endpoint"
```

---

## Chunk 3: CLI

### Task 11: Codebase detection utility

**Files:**
- Create: `apps/cli/src/lib/detect-codebase.ts`

- [ ] **Step 1: Write detection utility**

```typescript
// apps/cli/src/lib/detect-codebase.ts
import { execSync } from "node:child_process";

import { getServerUrl } from "./store";

export function getGitRemoteUrl(): string | null {
    try {
        return execSync("git remote get-url origin", { encoding: "utf-8", stdio: ["pipe", "pipe", "pipe"] }).trim() || null;
    } catch {
        return null;
    }
}

export async function detectCodebase(): Promise<{ id: string; name: string } | null> {
    const serverUrl = getServerUrl();
    if (!serverUrl) return null;

    const remoteUrl = getGitRemoteUrl();
    const localPath = process.cwd();

    const params = new URLSearchParams();
    if (remoteUrl) params.set("remoteUrl", remoteUrl);
    else params.set("localPath", localPath);

    try {
        const res = await fetch(`${serverUrl}/api/codebases/detect?${params}`);
        if (!res.ok) return null;
        const data = await res.json();
        return data && data.id ? { id: data.id, name: data.name } : null;
    } catch {
        return null;
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/cli/src/lib/detect-codebase.ts
git commit -m "feat(cli): add codebase detection via git remote and local path"
```

---

### Task 12: CLI codebase commands

**Files:**
- Create: `apps/cli/src/commands/codebase.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Write codebase command group**

```typescript
// apps/cli/src/commands/codebase.ts
import { Command } from "commander";

import { getGitRemoteUrl } from "../lib/detect-codebase";
import { output, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

const addCmd = new Command("add")
    .description("Register a codebase")
    .argument("<name>", "codebase name")
    .option("--path <path>", "local path (defaults to cwd)")
    .option("--remote <url>", "git remote URL (auto-detected if not provided)")
    .action(async (name: string, opts: { path?: string; remote?: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const localPath = opts.path ?? process.cwd();
        const remoteUrl = opts.remote ?? getGitRemoteUrl();

        const body: Record<string, unknown> = { name, localPaths: [localPath] };
        if (remoteUrl) body.remoteUrl = remoteUrl;

        const res = await fetch(`${serverUrl}/api/codebases`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body)
        });

        if (!res.ok) {
            const err = await res.json().catch(() => null);
            console.error(`Failed to register codebase: ${err?.message ?? res.statusText}`);
            process.exit(1);
        }

        const created = await res.json();
        outputQuiet(cmd, created.id);
        output(cmd, created, `Registered codebase "${name}" (${created.id})`);
    });

const listCmd = new Command("list")
    .description("List registered codebases")
    .action(async (_opts: unknown, cmd: Command) => {
        const serverUrl = requireServer();
        const res = await fetch(`${serverUrl}/api/codebases`);
        if (!res.ok) {
            console.error(`Failed to list codebases: ${res.statusText}`);
            process.exit(1);
        }
        const codebases = await res.json();
        outputQuiet(cmd, codebases.map((c: { id: string }) => c.id).join("\n"));
        if (codebases.length === 0) {
            output(cmd, codebases, "No codebases registered.");
        } else {
            const lines = codebases.map((c: { name: string; id: string; remoteUrl?: string }) =>
                `  ${c.name} (${c.id})${c.remoteUrl ? ` — ${c.remoteUrl}` : ""}`
            );
            output(cmd, codebases, lines.join("\n"));
        }
    });

const removeCmd = new Command("remove")
    .description("Unregister a codebase")
    .argument("<name>", "codebase name")
    .option("--force", "skip confirmation")
    .action(async (name: string, opts: { force?: boolean }, cmd: Command) => {
        const serverUrl = requireServer();

        // Find codebase by name
        const listRes = await fetch(`${serverUrl}/api/codebases`);
        if (!listRes.ok) {
            console.error("Failed to list codebases");
            process.exit(1);
        }
        const codebases = await listRes.json();
        const target = codebases.find((c: { name: string }) => c.name === name);
        if (!target) {
            console.error(`Codebase "${name}" not found.`);
            process.exit(1);
        }

        if (!opts.force) {
            // Prompt for confirmation
            const readline = await import("node:readline");
            const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
            const answer = await new Promise<string>(resolve => {
                rl.question(`This will unlink all chunks from codebase "${name}". Chunks will not be deleted. Continue? [y/N] `, resolve);
            });
            rl.close();
            if (answer.toLowerCase() !== "y") {
                console.error("Aborted.");
                process.exit(1);
            }
        }

        const res = await fetch(`${serverUrl}/api/codebases/${target.id}`, { method: "DELETE" });
        if (!res.ok) {
            console.error(`Failed to remove codebase: ${res.statusText}`);
            process.exit(1);
        }
        output(cmd, target, `Removed codebase "${name}".`);
    });

const currentCmd = new Command("current")
    .description("Show detected codebase for current directory")
    .action(async (_opts: unknown, cmd: Command) => {
        const serverUrl = requireServer();
        const remoteUrl = getGitRemoteUrl();
        const localPath = process.cwd();

        const params = new URLSearchParams();
        if (remoteUrl) params.set("remoteUrl", remoteUrl);
        else params.set("localPath", localPath);

        const res = await fetch(`${serverUrl}/api/codebases/detect?${params}`);
        if (!res.ok) {
            console.error("Failed to detect codebase.");
            process.exit(1);
        }
        const codebase = await res.json();
        if (!codebase || !codebase.id) {
            output(cmd, null, "No codebase detected for current directory.");
        } else {
            outputQuiet(cmd, codebase.id);
            output(cmd, codebase, `${codebase.name} (${codebase.id})`);
        }
    });

export const codebaseCommand = new Command("codebase")
    .description("Manage codebases")
    .addCommand(addCmd)
    .addCommand(listCmd)
    .addCommand(removeCmd)
    .addCommand(currentCmd);
```

- [ ] **Step 2: Register in CLI index**

In `apps/cli/src/index.ts`, add:
```typescript
import { codebaseCommand } from "./commands/codebase";
// ...
program.addCommand(codebaseCommand);
```

- [ ] **Step 3: Verify CLI parses correctly**

Run: `cd apps/cli && pnpm build && node dist/index.js codebase --help`
Expected: Shows help for codebase command group

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/commands/codebase.ts apps/cli/src/index.ts
git commit -m "feat(cli): add codebase command group (add, list, remove, current)"
```

---

### Task 13: Add implicit codebase scoping to CLI commands

**Files:**
- Modify: `apps/cli/src/commands/list.ts`
- Modify: `apps/cli/src/commands/add.ts`
- Modify: `apps/cli/src/commands/search.ts`

- [ ] **Step 1: Add --global and --codebase flags to list/add/search commands**

For each command, add:
```typescript
.option("--global", "ignore codebase context")
.option("--codebase <name>", "override codebase detection")
```

- [ ] **Step 2: In commands that call the server API, detect codebase and pass as query param**

Use `detectCodebase()` from `detect-codebase.ts` to resolve the active codebase, then pass `codebaseId` as a query param to the API. If `--global` is passed, skip detection. If `--codebase <name>` is passed, look up that codebase by name first.

The exact changes depend on how each command currently calls the API. Read each file and add the codebase resolution at the top of the action handler.

- [ ] **Step 3: Verify with `fubbik list --help`**

Run: `cd apps/cli && pnpm build && node dist/index.js list --help`
Expected: Shows --global and --codebase flags

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/commands/list.ts apps/cli/src/commands/add.ts apps/cli/src/commands/search.ts
git commit -m "feat(cli): add implicit codebase scoping to list, add, and search commands"
```

---

## Chunk 4: Web UI

### Task 14: Active codebase hook

**Files:**
- Create: `apps/web/src/features/codebases/use-active-codebase.ts`

- [ ] **Step 1: Write the hook**

```typescript
// apps/web/src/features/codebases/use-active-codebase.ts
import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback } from "react";

export function useActiveCodebase() {
    const search = useSearch({ strict: false }) as { codebase?: string };
    const navigate = useNavigate();

    const codebaseId = search.codebase ?? null;

    const setCodebaseId = useCallback(
        (id: string | null) => {
            navigate({
                search: (prev: Record<string, unknown>) => {
                    if (id === null) {
                        const { codebase: _, ...rest } = prev as { codebase?: string } & Record<string, unknown>;
                        return rest;
                    }
                    return { ...prev, codebase: id };
                }
            });
        },
        [navigate]
    );

    return { codebaseId, setCodebaseId };
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/codebases/use-active-codebase.ts
git commit -m "feat(web): add useActiveCodebase hook for URL-based codebase state"
```

---

### Task 15: Codebase switcher component

**Files:**
- Create: `apps/web/src/features/codebases/codebase-switcher.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Write the switcher component**

```tsx
// apps/web/src/features/codebases/codebase-switcher.tsx
import { useQuery } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { useActiveCodebase } from "./use-active-codebase";

export function CodebaseSwitcher() {
    const { codebaseId, setCodebaseId } = useActiveCodebase();

    const { data: codebases } = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.codebases.get());
            } catch {
                return [];
            }
        }
    });

    const activeName = codebaseId === "global"
        ? "Global"
        : codebaseId
          ? codebases?.find((c: { id: string }) => c.id === codebaseId)?.name ?? "..."
          : "All";

    return (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <Button variant="outline" size="sm" className="max-w-[150px] truncate">
                    {activeName}
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
                <DropdownMenuItem onClick={() => setCodebaseId(null)}>All</DropdownMenuItem>
                <DropdownMenuItem onClick={() => setCodebaseId("global")}>Global</DropdownMenuItem>
                <DropdownMenuSeparator />
                {codebases?.map((c: { id: string; name: string }) => (
                    <DropdownMenuItem key={c.id} onClick={() => setCodebaseId(c.id)}>
                        {c.name}
                    </DropdownMenuItem>
                ))}
                {(!codebases || codebases.length === 0) && (
                    <DropdownMenuItem disabled>No codebases registered</DropdownMenuItem>
                )}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
```

- [ ] **Step 2: Add switcher to nav in `__root.tsx`**

In `apps/web/src/routes/__root.tsx`, import and add `<CodebaseSwitcher />` next to the logo in the header:

```tsx
import { CodebaseSwitcher } from "@/features/codebases/codebase-switcher";

// In the header, after the logo link:
<Link to="/" className="flex items-center gap-2">
    <FubbikLogo className="size-6" />
    <span className="font-bold">fubbik</span>
</Link>
<CodebaseSwitcher />
```

- [ ] **Step 3: Run dev server and verify visually**

Run: `pnpm dev`
Expected: Codebase switcher dropdown visible in the nav bar

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/codebases/codebase-switcher.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(web): add codebase switcher dropdown to nav"
```

---

### Task 16: Wire codebase filtering into chunks page

**Files:**
- Modify: `apps/web/src/routes/chunks.tsx` (or wherever the chunks list route is)

- [ ] **Step 1: Read the current chunks route file**

Read the file and understand how it fetches chunks.

- [ ] **Step 2: Add codebaseId to the query**

In the query that fetches chunks, pass the active codebaseId:

```tsx
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";

// In the component:
const { codebaseId } = useActiveCodebase();

// In the useQuery call, add codebaseId to the query key and API params:
const chunksQuery = useQuery({
    queryKey: ["chunks", { ...filters, codebaseId }],
    queryFn: async () => {
        const params = { ...existingParams };
        if (codebaseId === "global") {
            params.global = "true";
        } else if (codebaseId) {
            params.codebaseId = codebaseId;
        }
        return unwrapEden(await api.api.chunks.get({ query: params }));
    }
});
```

- [ ] **Step 3: Verify chunks filter by codebase**

Run: `pnpm dev`
Expected: Switching codebase in dropdown refetches chunks list

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.tsx
git commit -m "feat(web): filter chunks list by active codebase"
```

---

### Task 17: Wire codebase filtering into graph page

**Files:**
- Modify: `apps/web/src/routes/graph.tsx`

- [ ] **Step 1: Add codebaseId to graph query**

Same pattern as Task 15 — read the graph route, pass `codebaseId` to the API query.

- [ ] **Step 2: Verify graph scopes by codebase**

Run: `pnpm dev`
Expected: Graph shows only chunks in active codebase + global chunks

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/graph.tsx
git commit -m "feat(web): scope graph view to active codebase"
```

---

### Task 18: Codebase management page

**Files:**
- Create: `apps/web/src/routes/codebases.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Write the codebases management page**

Simple CRUD page following the pattern of existing pages (like `/tags`). List all codebases with name, remote URL, local paths. Add/edit/delete actions.

Read `apps/web/src/routes/tags.tsx` first to follow the existing pattern for a management page.

- [ ] **Step 2: Add "Codebases" link to nav in `__root.tsx`**

Add a nav link between "Tags" and the right-side controls:
```tsx
<Link
    to="/codebases"
    className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
>
    Codebases
</Link>
```

- [ ] **Step 3: Verify the page works**

Run: `pnpm dev`
Expected: `/codebases` page loads and displays the management UI

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/codebases.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(web): add codebase management page and nav link"
```

---

### Task 19: Final verification

- [ ] **Step 1: Run full CI**

Run: `pnpm ci`
Expected: Type-check, lint, test, build all pass

- [ ] **Step 2: Fix any issues found**

- [ ] **Step 3: Final commit if needed**

```bash
git add -A && git commit -m "fix: resolve CI issues from multi-codebase feature"
```
