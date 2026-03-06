# Feature-Based Folder Structure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure the monorepo to feature-based organization with routes → services → repositories layering.

**Architecture:** Bottom-up: create repositories in db first, then split api into feature folders with services calling repositories, then
reorganize web features, finally CLI. Each task preserves existing behavior — no new features, pure refactor. Tests must pass after every
task.

**Tech Stack:** Bun, Elysia, Drizzle ORM, TanStack Start/Router, Vitest

---

## Phase 1: Database Repositories

### Task 1: Create health repository

**Files:**

- Create: `packages/db/src/repository/health.ts`
- Create: `packages/db/src/repository/index.ts`
- Modify: `packages/db/package.json` (add export)

**Step 1: Create health repository**

```typescript
// packages/db/src/repository/health.ts
import { sql } from "drizzle-orm";
import { db } from "../index";

export async function checkDbConnectivity(): Promise<boolean> {
    await db.execute(sql`SELECT 1`);
    return true;
}
```

**Step 2: Create repository barrel export**

```typescript
// packages/db/src/repository/index.ts
export * from "./health";
```

**Step 3: Add package export**

In `packages/db/package.json`, add to the `"exports"` field:

```json
"./repository": "./src/repository/index.ts"
```

**Step 4: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types` Expected: No errors

**Step 5: Commit**

```bash
git add packages/db/src/repository/ packages/db/package.json
git commit -m "feat: add health repository to db package"
```

---

### Task 2: Create chunk repository

**Files:**

- Create: `packages/db/src/repository/chunk.ts`
- Modify: `packages/db/src/repository/index.ts`

**Step 1: Create chunk repository**

Extract all chunk DB queries from `packages/api/src/index.ts` into pure functions. Each function takes typed params, returns typed results.
No Elysia types.

```typescript
// packages/db/src/repository/chunk.ts
import { and, desc, eq, ilike, or, sql } from "drizzle-orm";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export interface ListChunksParams {
    userId: string;
    type?: string;
    search?: string;
    limit: number;
    offset: number;
}

export async function listChunks(params: ListChunksParams) {
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
}

export async function getChunkById(chunkId: string, userId: string) {
    const [found] = await db
        .select()
        .from(chunk)
        .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)));
    return found ?? null;
}

export async function getChunkConnections(chunkId: string) {
    return db
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
        .where(or(eq(chunkConnection.sourceId, chunkId), eq(chunkConnection.targetId, chunkId)));
}

export interface CreateChunkParams {
    id: string;
    title: string;
    content: string;
    type: string;
    tags: string[];
    userId: string;
}

export async function createChunk(params: CreateChunkParams) {
    const [created] = await db.insert(chunk).values(params).returning();
    return created;
}

export interface UpdateChunkParams {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
}

export async function updateChunk(chunkId: string, params: UpdateChunkParams) {
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
}

export async function deleteChunk(chunkId: string, userId: string) {
    const [deleted] = await db
        .delete(chunk)
        .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
        .returning();
    return deleted ?? null;
}
```

**Step 2: Add to barrel export**

```typescript
// packages/db/src/repository/index.ts
export * from "./health";
export * from "./chunk";
```

**Step 3: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types` Expected: No errors

**Step 4: Commit**

```bash
git add packages/db/src/repository/
git commit -m "feat: add chunk repository to db package"
```

---

### Task 3: Create stats repository

**Files:**

- Create: `packages/db/src/repository/stats.ts`
- Modify: `packages/db/src/repository/index.ts`

**Step 1: Create stats repository**

```typescript
// packages/db/src/repository/stats.ts
import { eq, sql } from "drizzle-orm";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export async function getChunkCount(userId: string) {
    const [result] = await db
        .select({ count: sql<number>`count(*)` })
        .from(chunk)
        .where(eq(chunk.userId, userId));
    return Number(result?.count ?? 0);
}

export async function getConnectionCount(userId: string) {
    const [result] = await db
        .select({ count: sql<number>`count(*)` })
        .from(chunkConnection)
        .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
        .where(eq(chunk.userId, userId));
    return Number(result?.count ?? 0);
}

export async function getTagCount(userId: string) {
    const [result] = await db
        .select({
            count: sql<number>`count(distinct tag)`
        })
        .from(sql`(select jsonb_array_elements_text(${chunk.tags}) as tag from ${chunk} where ${chunk.userId} = ${userId}) t`);
    return Number(result?.count ?? 0);
}
```

**Step 2: Add to barrel export**

```typescript
// packages/db/src/repository/index.ts
export * from "./health";
export * from "./chunk";
export * from "./stats";
```

**Step 3: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
git add packages/db/src/repository/
git commit -m "feat: add stats repository to db package"
```

---

## Phase 2: API Feature Split

### Task 4: Create health routes

**Files:**

- Create: `packages/api/src/health/routes.ts`

**Step 1: Create health routes**

```typescript
// packages/api/src/health/routes.ts
import { Elysia } from "elysia";
import { checkDbConnectivity } from "@fubbik/db/repository";

export const healthRoutes = new Elysia().get("/health", async ({ set }) => {
    try {
        await checkDbConnectivity();
        return { status: "ok", db: "connected" };
    } catch {
        set.status = 503;
        return { status: "degraded", db: "disconnected" };
    }
});
```

**Step 2: Commit**

```bash
git add packages/api/src/health/
git commit -m "feat: extract health routes into feature folder"
```

---

### Task 5: Create chunks service and routes

**Files:**

- Create: `packages/api/src/chunks/service.ts`
- Create: `packages/api/src/chunks/routes.ts`

**Step 1: Create chunks service**

The service contains business logic. It receives already-validated params (no Elysia types), calls repositories, and returns data or throws.

```typescript
// packages/api/src/chunks/service.ts
import {
    createChunk as createChunkRepo,
    deleteChunk as deleteChunkRepo,
    getChunkById,
    getChunkConnections,
    listChunks as listChunksRepo,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";

export async function listChunks(userId: string, query: { type?: string; search?: string; limit?: string; offset?: string }) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);
    const result = await listChunksRepo({
        userId,
        type: query.type,
        search: query.search,
        limit,
        offset
    });
    return { ...result, limit, offset };
}

export async function getChunkDetail(chunkId: string, userId: string) {
    const found = await getChunkById(chunkId, userId);
    if (!found) return null;
    const connections = await getChunkConnections(chunkId);
    return { chunk: found, connections };
}

export async function createChunk(userId: string, body: { title: string; content?: string; type?: string; tags?: string[] }) {
    return createChunkRepo({
        id: crypto.randomUUID(),
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        tags: body.tags ?? [],
        userId
    });
}

export async function updateChunk(
    chunkId: string,
    userId: string,
    body: { title?: string; content?: string; type?: string; tags?: string[] }
) {
    const existing = await getChunkById(chunkId, userId);
    if (!existing) return null;
    return updateChunkRepo(chunkId, body);
}

export async function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId);
}
```

**Step 2: Create chunks routes**

```typescript
// packages/api/src/chunks/routes.ts
import { Elysia, t } from "elysia";
import { dbError } from "../error";
import * as chunkService from "./service";

export const chunkRoutes = new Elysia()
    .get(
        "/chunks",
        async ({ session, set, query }) => {
            if (!session) {
                set.status = 401;
                return { message: "Authentication required" };
            }
            try {
                return await chunkService.listChunks(session.user.id, query);
            } catch (err) {
                return dbError(set, "Failed to fetch chunks", err);
            }
        },
        {
            query: t.Object({
                type: t.Optional(t.String()),
                search: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String())
            })
        }
    )
    .get("/chunks/:id", async ({ session, set, params }) => {
        if (!session) {
            set.status = 401;
            return { message: "Authentication required" };
        }
        try {
            const result = await chunkService.getChunkDetail(params.id, session.user.id);
            if (!result) {
                set.status = 404;
                return { message: "Chunk not found" };
            }
            return result;
        } catch (err) {
            return dbError(set, "Failed to fetch chunk", err);
        }
    })
    .post(
        "/chunks",
        async ({ session, set, body }) => {
            if (!session) {
                set.status = 401;
                return { message: "Authentication required" };
            }
            try {
                const created = await chunkService.createChunk(session.user.id, body);
                set.status = 201;
                return created;
            } catch (err) {
                return dbError(set, "Failed to create chunk", err);
            }
        },
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
        async ({ session, set, params, body }) => {
            if (!session) {
                set.status = 401;
                return { message: "Authentication required" };
            }
            try {
                const updated = await chunkService.updateChunk(params.id, session.user.id, body);
                if (!updated) {
                    set.status = 404;
                    return { message: "Chunk not found" };
                }
                return updated;
            } catch (err) {
                return dbError(set, "Failed to update chunk", err);
            }
        },
        {
            body: t.Object({
                title: t.Optional(t.String()),
                content: t.Optional(t.String()),
                type: t.Optional(t.String()),
                tags: t.Optional(t.Array(t.String()))
            })
        }
    )
    .delete("/chunks/:id", async ({ session, set, params }) => {
        if (!session) {
            set.status = 401;
            return { message: "Authentication required" };
        }
        try {
            const deleted = await chunkService.deleteChunk(params.id, session.user.id);
            if (!deleted) {
                set.status = 404;
                return { message: "Chunk not found" };
            }
            return { message: "Deleted" };
        } catch (err) {
            return dbError(set, "Failed to delete chunk", err);
        }
    });
```

**Step 3: Commit**

```bash
git add packages/api/src/chunks/
git commit -m "feat: extract chunks routes and service into feature folder"
```

---

### Task 6: Create stats service and routes

**Files:**

- Create: `packages/api/src/stats/service.ts`
- Create: `packages/api/src/stats/routes.ts`

**Step 1: Create stats service**

```typescript
// packages/api/src/stats/service.ts
import { getChunkCount, getConnectionCount, getTagCount } from "@fubbik/db/repository";

export async function getUserStats(userId: string) {
    const [chunks, connections, tags] = await Promise.all([getChunkCount(userId), getConnectionCount(userId), getTagCount(userId)]);
    return { chunks, connections, tags };
}
```

**Step 2: Create stats routes**

```typescript
// packages/api/src/stats/routes.ts
import { Elysia } from "elysia";
import { dbError } from "../error";
import * as statsService from "./service";

export const statsRoutes = new Elysia().get("/stats", async ({ session, set }) => {
    if (!session) {
        set.status = 401;
        return { message: "Authentication required" };
    }
    try {
        return await statsService.getUserStats(session.user.id);
    } catch (err) {
        return dbError(set, "Failed to fetch stats", err);
    }
});
```

**Step 3: Commit**

```bash
git add packages/api/src/stats/
git commit -m "feat: extract stats routes and service into feature folder"
```

---

### Task 7: Rewire api index.ts to compose feature routes

**Files:**

- Modify: `packages/api/src/index.ts` (rewrite)

**Step 1: Replace index.ts with composer**

Replace the entire `packages/api/src/index.ts` with:

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

**Step 2: Run tests to verify nothing broke**

Run: `cd /Users/pontus/GitHub/fubbik && turbo run test` Expected: All 10 tests pass (8 API + 2 schema)

**Step 3: Commit**

```bash
git add packages/api/src/index.ts
git commit -m "refactor: rewire api index to compose feature routes"
```

---

## Phase 3: Web App Features

### Task 8: Create features/auth folder and move auth components

**Files:**

- Create: `apps/web/src/features/auth/`
- Move: `apps/web/src/components/sign-in-form.tsx` → `apps/web/src/features/auth/sign-in-form.tsx`
- Move: `apps/web/src/components/sign-up-form.tsx` → `apps/web/src/features/auth/sign-up-form.tsx`
- Move: `apps/web/src/components/user-menu.tsx` → `apps/web/src/features/auth/user-menu.tsx`
- Update imports in any files that reference these components

**Step 1: Create feature folder and move files**

```bash
mkdir -p apps/web/src/features/auth
git mv apps/web/src/components/sign-in-form.tsx apps/web/src/features/auth/sign-in-form.tsx
git mv apps/web/src/components/sign-up-form.tsx apps/web/src/features/auth/sign-up-form.tsx
git mv apps/web/src/components/user-menu.tsx apps/web/src/features/auth/user-menu.tsx
```

**Step 2: Update imports**

Search for all imports of these components and update paths:

- `@/components/sign-in-form` → `@/features/auth/sign-in-form`
- `@/components/sign-up-form` → `@/features/auth/sign-up-form`
- `@/components/user-menu` → `@/features/auth/user-menu`

Check these files for imports:

- `apps/web/src/routes/login.tsx`
- `apps/web/src/routes/__root.tsx`

**Step 3: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types` Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/features/ apps/web/src/components/ apps/web/src/routes/
git commit -m "refactor: move auth components to features/auth"
```

---

### Task 9: Create features/chunks and features/dashboard (empty for now)

**Files:**

- Create: `apps/web/src/features/chunks/.gitkeep`
- Create: `apps/web/src/features/dashboard/.gitkeep`

**Step 1: Create empty feature folders**

These are placeholders. As chunk-specific and dashboard-specific components are built, they go here instead of the flat `components/`
folder.

```bash
mkdir -p apps/web/src/features/chunks apps/web/src/features/dashboard
touch apps/web/src/features/chunks/.gitkeep apps/web/src/features/dashboard/.gitkeep
```

**Step 2: Commit**

```bash
git add apps/web/src/features/
git commit -m "refactor: add empty chunks and dashboard feature folders"
```

---

## Phase 4: CLI Light Restructure

### Task 10: Move CLI store to lib/

**Files:**

- Move: `apps/cli/src/store.ts` → `apps/cli/src/lib/store.ts`
- Update imports in all commands

**Step 1: Move file**

```bash
mkdir -p apps/cli/src/lib
git mv apps/cli/src/store.ts apps/cli/src/lib/store.ts
```

**Step 2: Update imports**

In all files under `apps/cli/src/commands/`, update:

- `from "../store"` → `from "../lib/store"`

Files to update: `add.ts`, `get.ts`, `init.ts`, `list.ts`, `remove.ts`, `search.ts`, `update.ts`

**Step 3: Verify**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types` Expected: No errors

**Step 4: Commit**

```bash
git add apps/cli/src/
git commit -m "refactor: move CLI store to lib/"
```

---

## Phase 5: Cleanup

### Task 11: Update package exports and verify full CI

**Files:**

- Verify: `packages/db/package.json` exports include `"./repository"`
- Verify: `packages/api/package.json` exports are correct

**Step 1: Run full CI**

Run: `cd /Users/pontus/GitHub/fubbik && bun ci` Expected: type-check, lint, test, build, fmt:check, sherif all pass

**Step 2: Fix any issues found**

If CI fails, fix the specific issues (likely import paths or missing exports).

**Step 3: Commit any fixes**

```bash
git commit -m "fix: resolve CI issues after restructure"
```

---

## Summary

| Phase | Tasks | What changes                                                                     |
| ----- | ----- | -------------------------------------------------------------------------------- |
| 1     | 1-3   | Create repositories in `packages/db/src/repository/`                             |
| 2     | 4-7   | Split `packages/api/` into `health/`, `chunks/`, `stats/` with routes + services |
| 3     | 8-9   | Move web auth components to `features/auth/`, scaffold feature folders           |
| 4     | 10    | Move CLI `store.ts` to `lib/store.ts`                                            |
| 5     | 11    | Verify full CI passes                                                            |
