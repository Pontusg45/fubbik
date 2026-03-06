# Effect Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all try-catch error handling with Effect, giving typed errors from DB repositories through services to route boundary.

**Architecture:** Repositories return `Effect<T, DatabaseError>`. Services compose Effects and add `NotFoundError`/`AuthError`. Routes call
`Effect.runPromise` — thrown errors caught by a global Elysia `.onError` handler that maps `_tag` to HTTP status codes. Zero try-catch
blocks remain.

**Tech Stack:** Effect, Elysia, Drizzle ORM, Vitest

---

## Task 1: Install Effect dependency

**Files:**

- Modify: `packages/db/package.json`
- Modify: `packages/api/package.json`

**Step 1: Install effect in both packages**

```bash
cd /Users/pontus/GitHub/fubbik && bun add effect --filter @fubbik/db && bun add effect --filter @fubbik/api
```

**Step 2: Remove unused drizzle-orm from api package**

In `packages/api/package.json`, remove `"drizzle-orm": "^0.45.1"` from dependencies (repositories in db package handle all drizzle calls
now).

**Step 3: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && bun install`

**Step 4: Commit**

```bash
git add packages/db/package.json packages/api/package.json bun.lock
git commit -m "feat: add effect dependency to db and api packages"
```

---

## Task 2: Create DatabaseError in packages/db

**Files:**

- Create: `packages/db/src/errors.ts`

**Step 1: Create error type**

```typescript
// packages/db/src/errors.ts
import { Data } from "effect";

export class DatabaseError extends Data.TaggedError("DatabaseError")<{
    cause: unknown;
}> {}
```

**Step 2: Add package export**

In `packages/db/package.json`, add to `"exports"`:

```json
"./errors": "./src/errors.ts"
```

**Step 3: Commit**

```bash
git add packages/db/src/errors.ts packages/db/package.json
git commit -m "feat: add DatabaseError tagged error to db package"
```

---

## Task 3: Convert health repository to Effect

**Files:**

- Modify: `packages/db/src/repository/health.ts`

**Step 1: Rewrite to return Effect**

```typescript
// packages/db/src/repository/health.ts
import { Effect } from "effect";
import { sql } from "drizzle-orm";
import { db } from "../index";
import { DatabaseError } from "../errors";

export function checkDbConnectivity() {
    return Effect.tryPromise({
        try: async () => {
            await db.execute(sql`SELECT 1`);
            return true;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && turbo run test` Expected: All tests pass (health test uses HTTP boundary, not repository directly)

**Step 3: Commit**

```bash
git add packages/db/src/repository/health.ts
git commit -m "refactor: convert health repository to Effect"
```

---

## Task 4: Convert chunk repository to Effect

**Files:**

- Modify: `packages/db/src/repository/chunk.ts`

**Step 1: Rewrite all functions to return Effect**

```typescript
// packages/db/src/repository/chunk.ts
import { Effect } from "effect";
import { and, desc, eq, ilike, or, sql } from "drizzle-orm";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { DatabaseError } from "../errors";

export interface ListChunksParams {
    userId: string;
    type?: string;
    search?: string;
    limit: number;
    offset: number;
}

export function listChunks(params: ListChunksParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.userId, params.userId)];
            if (params.type) {
                conditions.push(eq(chunk.type, params.type));
            }
            if (params.search) {
                conditions.push(or(ilike(chunk.title, `%${params.search}%`), ilike(chunk.content, `%${params.search}%`))!);
            }
            const chunks = await db
                .select()
                .from(chunk)
                .where(and(...conditions))
                .orderBy(desc(chunk.updatedAt))
                .limit(params.limit)
                .offset(params.offset);

            const total = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, total: Number(total[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunkById(chunkId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(chunk)
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunkConnections(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkConnection.id,
                    targetId: chunkConnection.targetId,
                    sourceId: chunkConnection.sourceId,
                    relation: chunkConnection.relation,
                    title: chunk.title
                })
                .from(chunkConnection)
                .leftJoin(
                    chunk,
                    or(
                        and(eq(chunkConnection.targetId, chunk.id), eq(chunkConnection.sourceId, chunkId)),
                        and(eq(chunkConnection.sourceId, chunk.id), eq(chunkConnection.targetId, chunkId))
                    )
                )
                .where(or(eq(chunkConnection.sourceId, chunkId), eq(chunkConnection.targetId, chunkId))),
        catch: cause => new DatabaseError({ cause })
    });
}

export interface CreateChunkParams {
    id: string;
    title: string;
    content: string;
    type: string;
    tags: string[];
    userId: string;
}

export function createChunk(params: CreateChunkParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(chunk).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateChunkParams {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
}

export function updateChunk(chunkId: string, params: UpdateChunkParams) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunk)
                .set({
                    ...(params.title !== undefined && { title: params.title }),
                    ...(params.content !== undefined && { content: params.content }),
                    ...(params.type !== undefined && { type: params.type }),
                    ...(params.tags !== undefined && { tags: params.tags })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteChunk(chunkId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(chunk)
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && turbo run test`

**Step 3: Commit**

```bash
git add packages/db/src/repository/chunk.ts
git commit -m "refactor: convert chunk repository to Effect"
```

---

## Task 5: Convert stats repository to Effect

**Files:**

- Modify: `packages/db/src/repository/stats.ts`

**Step 1: Rewrite all functions to return Effect**

```typescript
// packages/db/src/repository/stats.ts
import { Effect } from "effect";
import { eq, sql } from "drizzle-orm";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { DatabaseError } from "../errors";

export function getChunkCount(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(eq(chunk.userId, userId));
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getConnectionCount(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunkConnection)
                .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
                .where(eq(chunk.userId, userId));
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagCount(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({
                    count: sql<number>`count(distinct tag)`
                })
                .from(sql`(select jsonb_array_elements_text(${chunk.tags}) as tag from ${chunk} where ${chunk.userId} = ${userId}) t`);
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && turbo run test`

**Step 3: Commit**

```bash
git add packages/db/src/repository/stats.ts
git commit -m "refactor: convert stats repository to Effect"
```

---

## Task 6: Create API error types and requireSession helper

**Files:**

- Create: `packages/api/src/errors.ts`
- Delete: `packages/api/src/error.ts`

**Step 1: Create API errors**

```typescript
// packages/api/src/errors.ts
import { Data } from "effect";

export class NotFoundError extends Data.TaggedError("NotFoundError")<{
    resource: string;
}> {}

export class AuthError extends Data.TaggedError("AuthError")<{}> {}
```

**Step 2: Delete old error helper**

```bash
rm packages/api/src/error.ts
```

**Step 3: Commit**

```bash
git add packages/api/src/errors.ts
git rm packages/api/src/error.ts
git commit -m "feat: add Effect error types, remove dbError helper"
```

---

## Task 7: Convert chunks service to Effect

**Files:**

- Modify: `packages/api/src/chunks/service.ts`

**Step 1: Rewrite service to compose Effects**

```typescript
// packages/api/src/chunks/service.ts
import { Effect } from "effect";
import {
    createChunk as createChunkRepo,
    deleteChunk as deleteChunkRepo,
    getChunkById,
    getChunkConnections,
    listChunks as listChunksRepo,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { NotFoundError } from "../errors";

export function listChunks(userId: string, query: { type?: string; search?: string; limit?: string; offset?: string }) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);
    return listChunksRepo({ userId, type: query.type, search: query.search, limit, offset }).pipe(
        Effect.map(result => ({ ...result, limit, offset }))
    );
}

export function getChunkDetail(chunkId: string, userId: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(found => getChunkConnections(chunkId).pipe(Effect.map(connections => ({ chunk: found, connections }))))
    );
}

export function createChunk(userId: string, body: { title: string; content?: string; type?: string; tags?: string[] }) {
    return createChunkRepo({
        id: crypto.randomUUID(),
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        tags: body.tags ?? [],
        userId
    });
}

export function updateChunk(chunkId: string, userId: string, body: { title?: string; content?: string; type?: string; tags?: string[] }) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => updateChunkRepo(chunkId, body))
    );
}

export function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}
```

**Step 2: Commit**

```bash
git add packages/api/src/chunks/service.ts
git commit -m "refactor: convert chunks service to Effect"
```

---

## Task 8: Convert stats service to Effect

**Files:**

- Modify: `packages/api/src/stats/service.ts`

**Step 1: Rewrite service to compose Effects**

```typescript
// packages/api/src/stats/service.ts
import { Effect } from "effect";
import { getChunkCount, getConnectionCount, getTagCount } from "@fubbik/db/repository";

export function getUserStats(userId: string) {
    return Effect.all(
        {
            chunks: getChunkCount(userId),
            connections: getConnectionCount(userId),
            tags: getTagCount(userId)
        },
        { concurrency: "unbounded" }
    );
}
```

**Step 2: Commit**

```bash
git add packages/api/src/stats/service.ts
git commit -m "refactor: convert stats service to Effect"
```

---

## Task 9: Add global error handler and requireSession, rewrite routes

**Files:**

- Modify: `packages/api/src/index.ts`
- Modify: `packages/api/src/health/routes.ts`
- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/stats/routes.ts`

**Step 1: Update api index with global error handler**

```typescript
// packages/api/src/index.ts
import { Elysia } from "elysia";
import { auth } from "@fubbik/auth";
import type { Session } from "./context";
import { healthRoutes } from "./health/routes";
import { chunkRoutes } from "./chunks/routes";
import { statsRoutes } from "./stats/routes";

const isDev = process.env.NODE_ENV !== "production";

const DEV_USER_ID = "dev-user";
const DEV_SESSION: Session = {
    session: {
        id: "dev-session",
        token: "dev-token",
        userId: DEV_USER_ID,
        expiresAt: new Date(Date.now() + 86400000),
        createdAt: new Date(),
        updatedAt: new Date(),
        ipAddress: null,
        userAgent: null
    },
    user: {
        id: DEV_USER_ID,
        name: "Dev User",
        email: "dev@localhost",
        emailVerified: false,
        image: null,
        createdAt: new Date(),
        updatedAt: new Date()
    }
};

async function getSession(headers: Headers): Promise<Session> {
    const session = await auth.api.getSession({ headers });
    if (!session && isDev) return DEV_SESSION;
    return session;
}

export const api = new Elysia({ prefix: "/api" })
    .use(healthRoutes)
    .onError(({ error, set }) => {
        if (typeof error === "object" && error !== null && "_tag" in error) {
            switch ((error as { _tag: string })._tag) {
                case "AuthError":
                    set.status = 401;
                    return { message: "Authentication required" };
                case "NotFoundError":
                    set.status = 404;
                    return { message: `${(error as { resource: string }).resource} not found` };
                case "DatabaseError":
                    set.status = 500;
                    console.error("Database error", (error as { cause: unknown }).cause);
                    return { message: "Internal server error" };
            }
        }
    })
    .resolve(async ({ headers }) => {
        const session = await getSession(new Headers(headers as Record<string, string>));
        return { session };
    })
    .get("/me", ({ session, set }) => {
        if (!session) {
            set.status = 401;
            return { message: "Authentication required" };
        }
        return { message: "This is private", user: session.user };
    })
    .use(chunkRoutes)
    .use(statsRoutes);

export type Api = typeof api;
```

**Step 2: Rewrite health routes**

```typescript
// packages/api/src/health/routes.ts
import { Effect } from "effect";
import { Elysia } from "elysia";
import { checkDbConnectivity } from "@fubbik/db/repository";

export const healthRoutes = new Elysia().get("/health", () =>
    Effect.runPromise(
        checkDbConnectivity().pipe(
            Effect.match({
                onSuccess: () => ({ status: "ok" as const, db: "connected" as const }),
                onFailure: () => ({ status: "degraded" as const, db: "disconnected" as const })
            })
        )
    )
);
```

Note: The health route uses `Effect.match` to handle both success and failure locally — it doesn't throw to the global error handler because
a DB-down health check is a 200 with degraded status, not an error. However, the original returned 503 for degraded. Let's preserve that:

```typescript
// packages/api/src/health/routes.ts
import { Effect } from "effect";
import { Elysia } from "elysia";
import { checkDbConnectivity } from "@fubbik/db/repository";

export const healthRoutes = new Elysia().get("/health", ({ set }) =>
    Effect.runPromise(
        checkDbConnectivity().pipe(
            Effect.match({
                onSuccess: () => ({ status: "ok" as const, db: "connected" as const }),
                onFailure: () => {
                    set.status = 503;
                    return { status: "degraded" as const, db: "disconnected" as const };
                }
            })
        )
    )
);
```

**Step 3: Rewrite chunks routes**

```typescript
// packages/api/src/chunks/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import type { Session } from "../context";
import { AuthError } from "../errors";
import * as chunkService from "./service";

function requireSession(ctx: unknown) {
    const session = (ctx as unknown as { session: Session }).session;
    return session ? Effect.succeed(session) : Effect.fail(new AuthError());
}

export const chunkRoutes = new Elysia()
    .get(
        "/chunks",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.listChunks(session.user.id, ctx.query)))),
        {
            query: t.Object({
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String())
            })
        }
    )
    .get("/chunks/:id", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => chunkService.getChunkDetail(ctx.params.id, session.user.id))))
    )
    .post(
        "/chunks",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => chunkService.createChunk(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                title: t.String(),
                content: t.Optional(t.String()),
                type: t.Optional(t.String()),
                tags: t.Optional(t.Array(t.String()))
            })
        }
    )
    .patch(
        "/chunks/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => chunkService.updateChunk(ctx.params.id, session.user.id, ctx.body)))
            ),
        {
            body: t.Object({
                title: t.Optional(t.String()),
                content: t.Optional(t.String()),
                type: t.Optional(t.String()),
                tags: t.Optional(t.Array(t.String()))
            })
        }
    )
    .delete("/chunks/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => chunkService.deleteChunk(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

**Step 4: Rewrite stats routes**

```typescript
// packages/api/src/stats/routes.ts
import { Effect } from "effect";
import { Elysia } from "elysia";
import type { Session } from "../context";
import { AuthError } from "../errors";
import * as statsService from "./service";

function requireSession(ctx: unknown) {
    const session = (ctx as unknown as { session: Session }).session;
    return session ? Effect.succeed(session) : Effect.fail(new AuthError());
}

export const statsRoutes = new Elysia().get("/stats", ctx =>
    Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => statsService.getUserStats(session.user.id))))
);
```

**Step 5: Run tests**

Run: `cd /Users/pontus/GitHub/fubbik && turbo run test` Expected: All 10 tests pass (8 API + 2 schema)

**Step 6: Commit**

```bash
git add packages/api/src/
git commit -m "refactor: rewrite routes with Effect, add global error handler"
```

---

## Task 10: Run full CI and fix any issues

**Files:**

- Any files that need fixes

**Step 1: Run full CI**

Run: `cd /Users/pontus/GitHub/fubbik && bun ci`

**Step 2: Fix any type errors or lint issues**

If CI fails, fix specific issues (likely type narrowing in routes or missing imports).

**Step 3: Commit fixes if needed**

```bash
git commit -m "fix: resolve CI issues after Effect integration"
```

---

## Summary

| Task | What changes                                                            |
| ---- | ----------------------------------------------------------------------- |
| 1    | Install `effect`, remove unused `drizzle-orm` from api                  |
| 2    | Create `DatabaseError` tagged error in db                               |
| 3-5  | Convert all repositories to return `Effect<T, DatabaseError>`           |
| 6    | Create `NotFoundError`/`AuthError` in api, delete `dbError` helper      |
| 7-8  | Convert services to compose Effects                                     |
| 9    | Global `.onError` handler + rewrite all routes with `Effect.runPromise` |
| 10   | Full CI verification                                                    |
