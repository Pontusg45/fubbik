# Tag Type Grouping Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize tags into structured tables with user-defined tag types, and add visual tag-type-based grouping to the graph view with convex hull background regions and cross-group edge dimming.

**Architecture:** Three new DB tables (`tag_type`, `tag`, `chunk_tag`) replace the JSONB `tags` column. The graph API returns tag/type data alongside chunks. The force layout gains tag-based attractor forces, and an SVG overlay renders convex hull regions behind grouped nodes. A new filter panel section lets users toggle grouping by tag type.

**Tech Stack:** Drizzle ORM (schema + migration), Elysia (routes), Effect (error handling), @xyflow/react (graph rendering), SVG (convex hull regions)

**Spec:** `docs/superpowers/specs/2026-03-12-tag-type-grouping-design.md`

---

## Chunk 1: Database Schema & Repositories

### Task 1: Create tag_type schema

**Files:**
- Create: `packages/db/src/schema/tag.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create the schema file**

```typescript
// packages/db/src/schema/tag.ts
import { relations } from "drizzle-orm";
import { pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";

export const tagType = pgTable(
    "tag_type",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        color: text("color").notNull().default("#8b5cf6"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [uniqueIndex("tag_type_user_name_idx").on(table.userId, table.name)]
);

export const tag = pgTable(
    "tag",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        tagTypeId: text("tag_type_id").references(() => tagType.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [uniqueIndex("tag_user_name_idx").on(table.userId, table.name)]
);

export const chunkTag = pgTable(
    "chunk_tag",
    {
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        tagId: text("tag_id")
            .notNull()
            .references(() => tag.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.chunkId, table.tagId] })]
);

// Relations
export const tagTypeRelations = relations(tagType, ({ one, many }) => ({
    user: one(user, { fields: [tagType.userId], references: [user.id] }),
    tags: many(tag)
}));

export const tagRelations = relations(tag, ({ one, many }) => ({
    user: one(user, { fields: [tag.userId], references: [user.id] }),
    tagType: one(tagType, { fields: [tag.tagTypeId], references: [tagType.id] }),
    chunkTags: many(chunkTag)
}));

export const chunkTagRelations = relations(chunkTag, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkTag.chunkId], references: [chunk.id] }),
    tag: one(tag, { fields: [chunkTag.tagId], references: [tag.id] })
}));
```


- [ ] **Step 2: Export from schema index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./tag";
```

- [ ] **Step 3: Push schema to database**

Run: `pnpm db:push`
Expected: Tables `tag_type`, `tag`, `chunk_tag` created successfully.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/tag.ts packages/db/src/schema/index.ts
git commit -m "feat: add tag_type, tag, and chunk_tag schema tables"
```

---

### Task 2: Create tag type repository

**Files:**
- Create: `packages/db/src/repository/tag-type.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the repository file**

```typescript
// packages/db/src/repository/tag-type.ts
import { and, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { tagType } from "../schema/tag";

export function createTagType(params: { id: string; name: string; color: string; userId: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(tagType).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagTypesForUser(userId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(tagType).where(eq(tagType.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateTagType(id: string, userId: string, data: { name?: string; color?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(tagType)
                .set(data)
                .where(and(eq(tagType.id, id), eq(tagType.userId, userId)))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteTagType(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(tagType)
                .where(and(eq(tagType.id, id), eq(tagType.userId, userId)))
                .returning();
            return deleted;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./tag-type";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/tag-type.ts packages/db/src/repository/index.ts
git commit -m "feat: add tag type repository with CRUD operations"
```

---

### Task 3: Create tag repository (with chunk_tag join operations)

**Files:**
- Create: `packages/db/src/repository/tag-new.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the repository file**

```typescript
// packages/db/src/repository/tag-new.ts
import { and, eq, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { tag, chunkTag, tagType } from "../schema/tag";

export function createTag(params: { id: string; name: string; tagTypeId?: string; userId: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(tag).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagsForUser(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: tag.id,
                    name: tag.name,
                    tagTypeId: tag.tagTypeId,
                    tagTypeName: tagType.name,
                    tagTypeColor: tagType.color
                })
                .from(tag)
                .leftJoin(tagType, eq(tag.tagTypeId, tagType.id))
                .where(eq(tag.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateTag(id: string, userId: string, data: { name?: string; tagTypeId?: string | null }) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(tag)
                .set(data)
                .where(and(eq(tag.id, id), eq(tag.userId, userId)))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteTag(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(tag)
                .where(and(eq(tag.id, id), eq(tag.userId, userId)))
                .returning();
            return deleted;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

// --- chunk_tag join operations ---

export function setChunkTags(chunkId: string, tagIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            await db.delete(chunkTag).where(eq(chunkTag.chunkId, chunkId));
            if (tagIds.length === 0) return [];
            return db
                .insert(chunkTag)
                .values(tagIds.map(tagId => ({ chunkId, tagId })))
                .onConflictDoNothing()
                .returning();
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagsForChunk(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({ id: tag.id, name: tag.name, tagTypeId: tag.tagTypeId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(eq(chunkTag.chunkId, chunkId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagsForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([]);
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    chunkId: chunkTag.chunkId,
                    tagId: tag.id,
                    tagName: tag.name,
                    tagTypeId: tag.tagTypeId
                })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(inArray(chunkTag.chunkId, chunkIds)),
        catch: cause => new DatabaseError({ cause })
    });
}

/** Find or create a tag by name for a user, returning the tag. */
export function findOrCreateTag(name: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const existing = await db
                .select()
                .from(tag)
                .where(and(eq(tag.name, name), eq(tag.userId, userId)))
                .limit(1);
            if (existing.length > 0) return existing[0]!;
            const id = crypto.randomUUID();
            const [created] = await db.insert(tag).values({ id, name, userId }).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./tag-new";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/tag-new.ts packages/db/src/repository/index.ts
git commit -m "feat: add tag and chunk_tag repository with join operations"
```

---

### Task 4: Migrate existing JSONB tags to new tables

**Files:**
- Create: `packages/db/src/migrate-tags.ts` (one-time migration script)

- [ ] **Step 1: Write the migration script**

```typescript
// packages/db/src/migrate-tags.ts
import { db } from "./index";
import { chunk } from "./schema/chunk";
import { tag, chunkTag } from "./schema/tag";
import { eq, sql } from "drizzle-orm";

async function migrateTags() {
    console.log("Starting tag migration...");

    // 1. Get all chunks with their JSONB tags
    const chunks = await db
        .select({ id: chunk.id, tags: chunk.tags, userId: chunk.userId })
        .from(chunk);

    // 2. Collect unique tag names per user
    const userTags = new Map<string, Set<string>>();
    for (const c of chunks) {
        const tags = (c.tags as string[]) ?? [];
        if (!userTags.has(c.userId)) userTags.set(c.userId, new Set());
        for (const t of tags) userTags.get(c.userId)!.add(t);
    }

    // 3. Insert unique tags
    const tagNameToId = new Map<string, string>(); // "userId::tagName" -> tagId
    for (const [userId, names] of userTags) {
        for (const name of names) {
            const id = crypto.randomUUID();
            await db.insert(tag).values({ id, name, userId }).onConflictDoNothing();
            // Fetch to get the actual ID (might already exist)
            const [existing] = await db
                .select({ id: tag.id })
                .from(tag)
                .where(sql`${tag.name} = ${name} AND ${tag.userId} = ${userId}`)
                .limit(1);
            if (existing) tagNameToId.set(`${userId}::${name}`, existing.id);
        }
    }

    // 4. Insert chunk_tag associations
    let associations = 0;
    for (const c of chunks) {
        const tags = (c.tags as string[]) ?? [];
        for (const tagName of tags) {
            const tagId = tagNameToId.get(`${c.userId}::${tagName}`);
            if (tagId) {
                await db.insert(chunkTag).values({ chunkId: c.id, tagId }).onConflictDoNothing();
                associations++;
            }
        }
    }

    console.log(`Migrated ${tagNameToId.size} unique tags, ${associations} associations`);
}

migrateTags()
    .then(() => {
        console.log("Migration complete");
        process.exit(0);
    })
    .catch(err => {
        console.error("Migration failed:", err);
        process.exit(1);
    });
```

- [ ] **Step 2: Run the migration**

Run: `cd packages/db && npx tsx src/migrate-tags.ts`
Expected: "Migration complete" with count of tags and associations.

- [ ] **Step 3: Verify migration**

Run: `pnpm db:studio` — check that `tag` and `chunk_tag` tables have rows matching the old JSONB data.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/migrate-tags.ts
git commit -m "feat: add one-time migration script for JSONB tags to normalized tables"
```

---

## Chunk 2: API Routes & Services

### Task 5: Create tag type API routes

**Files:**
- Create: `packages/api/src/tag-types/service.ts`
- Create: `packages/api/src/tag-types/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create tag type service**

```typescript
// packages/api/src/tag-types/service.ts
import { createTagType as createTagTypeRepo, deleteTagType as deleteTagTypeRepo, getTagTypesForUser, updateTagType as updateTagTypeRepo } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function listTagTypes(userId: string) {
    return getTagTypesForUser(userId);
}

export function createTagType(userId: string, body: { name: string; color?: string }) {
    const id = crypto.randomUUID();
    return createTagTypeRepo({ id, name: body.name, color: body.color ?? "#8b5cf6", userId });
}

export function updateTagType(id: string, userId: string, body: { name?: string; color?: string }) {
    return updateTagTypeRepo(id, userId, body).pipe(
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "TagType" }))))
    );
}

export function deleteTagType(id: string, userId: string) {
    return deleteTagTypeRepo(id, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "TagType" }))))
    );
}
```

- [ ] **Step 2: Create tag type routes**

```typescript
// packages/api/src/tag-types/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as tagTypeService from "./service";

export const tagTypeRoutes = new Elysia()
    .get("/tag-types", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => tagTypeService.listTagTypes(session.user.id))))
    )
    .post(
        "/tag-types",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagTypeService.createTagType(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 50 }),
                color: t.Optional(t.String({ maxLength: 7 }))
            })
        }
    )
    .patch(
        "/tag-types/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagTypeService.updateTagType(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 50 })),
                color: t.Optional(t.String({ maxLength: 7 }))
            })
        }
    )
    .delete("/tag-types/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => tagTypeService.deleteTagType(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

- [ ] **Step 3: Register routes in API index**

Add import and `.use(tagTypeRoutes)` to `packages/api/src/index.ts`.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/tag-types/
git commit -m "feat: add tag type CRUD API routes"
```

---

### Task 6: Update tag API routes to use new tables

**Files:**
- Create: `packages/api/src/tags/service-new.ts` (replaces current service)
- Modify: `packages/api/src/tags/routes.ts`

- [ ] **Step 1: Create updated tag service**

```typescript
// packages/api/src/tags/service-new.ts
import {
    createTag as createTagRepo,
    deleteTag as deleteTagRepo,
    findOrCreateTag,
    getTagsForUser,
    setChunkTags,
    updateTag as updateTagRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function getUserTags(userId: string) {
    return getTagsForUser(userId);
}

export function createUserTag(userId: string, body: { name: string; tagTypeId?: string }) {
    const id = crypto.randomUUID();
    return createTagRepo({ id, name: body.name, tagTypeId: body.tagTypeId, userId });
}

export function updateUserTag(id: string, userId: string, body: { name?: string; tagTypeId?: string | null }) {
    return updateTagRepo(id, userId, body).pipe(
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Tag" }))))
    );
}

export function deleteUserTag(id: string, userId: string) {
    return deleteTagRepo(id, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Tag" }))))
    );
}

export { setChunkTags, findOrCreateTag };
```

- [ ] **Step 2: Delete old tag service**

Delete `packages/api/src/tags/service.ts` (the old service that used JSONB queries).

- [ ] **Step 3: Update tag routes**

Replace `packages/api/src/tags/routes.ts` to add full CRUD and use the new service:

```typescript
// packages/api/src/tags/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as tagService from "./service-new";

export const tagRoutes = new Elysia()
    .get("/tags", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => tagService.getUserTags(session.user.id))))
    )
    .post(
        "/tags",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagService.createUserTag(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 50 }),
                tagTypeId: t.Optional(t.String())
            })
        }
    )
    .patch(
        "/tags/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagService.updateUserTag(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 50 })),
                tagTypeId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    .delete("/tags/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => tagService.deleteUserTag(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

- [ ] **Step 4: Delete old tags repository**

Delete `packages/db/src/repository/tags.ts` and remove `export * from "./tags"` from `packages/db/src/repository/index.ts`.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/tags/ packages/db/src/repository/tags.ts packages/db/src/repository/index.ts
git commit -m "feat: update tag routes to use normalized tag tables, remove old JSONB tag code"
```

---

### Task 7: Extend graph API to include tag data

**Files:**
- Modify: `packages/api/src/graph/service.ts`
- Modify: `packages/db/src/repository/graph.ts`

- [ ] **Step 1: Add tag data fetching to graph repository**

Add to `packages/db/src/repository/graph.ts`:

```typescript
import { tag, tagType, chunkTag } from "../schema/tag";

export function getAllTagsWithTypes(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db
                .select({
                    chunkId: chunkTag.chunkId,
                    tagId: tag.id,
                    tagName: tag.name,
                    tagTypeId: tag.tagTypeId,
                    tagTypeName: tagType.name,
                    tagTypeColor: tagType.color
                })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .leftJoin(tagType, eq(tag.tagTypeId, tagType.id));

            if (userId) {
                return query.where(eq(tag.userId, userId));
            }
            return query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTagTypesForGraph(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db.select().from(tagType);
            if (userId) return query.where(eq(tagType.userId, userId));
            return query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

Add the necessary imports (`eq` from `drizzle-orm`, schema imports).

- [ ] **Step 2: Update graph service**

Update `packages/api/src/graph/service.ts`:

```typescript
import { getAllChunksMeta, getAllConnectionsForUser, getAllTagsWithTypes, getTagTypesForGraph } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserGraph(userId?: string) {
    return Effect.all(
        {
            chunks: getAllChunksMeta(userId),
            connections: getAllConnectionsForUser(userId),
            chunkTags: getAllTagsWithTypes(userId),
            tagTypes: getTagTypesForGraph(userId)
        },
        { concurrency: "unbounded" }
    );
}
```

- [ ] **Step 3: Verify the API returns tag data**

Run: `pnpm dev` then `curl http://localhost:3000/api/graph | jq '.tagTypes, .chunkTags[0:2]'`
Expected: Tag types array and chunk tag associations with type info.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/graph.ts packages/api/src/graph/service.ts
git commit -m "feat: extend graph API to include tag type and chunk tag data"
```

---

### Task 8: Update chunk create/update to use normalized tags

**Files:**
- Modify: `packages/api/src/chunks/service.ts`

The chunk create/update routes currently accept `tags: string[]` in the body. We keep that interface but internally resolve tag names to the `tag` table and create `chunk_tag` associations.

- [ ] **Step 1: Update createChunk**

In `packages/api/src/chunks/service.ts`, update `createChunk` to resolve tag names after creating the chunk:

```typescript
import { findOrCreateTag, setChunkTags } from "@fubbik/db/repository";

export function createChunk(userId: string, body: { title: string; content?: string; type?: string; tags?: string[] }) {
    const id = crypto.randomUUID();
    return createChunkRepo({
        id,
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        tags: body.tags ?? [], // dual-write to JSONB until Task 9 drops the column
        userId
    }).pipe(
        Effect.tap(() => {
            // Sync to normalized tag tables
            if (body.tags && body.tags.length > 0) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(id, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            Effect.runPromise(enrichChunkIfEmpty(id)).catch(() => {});
            return Effect.void;
        })
    );
}
```

- [ ] **Step 2: Update updateChunk**

Similarly update `updateChunk` to sync tags when `body.tags` is provided:

After the existing `updateChunkRepo(chunkId, body)` call, add tag syncing:

```typescript
Effect.flatMap(() => updateChunkRepo(chunkId, body)),
Effect.tap(() => {
    if (body.tags) {
        return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
            Effect.flatMap(tags => setChunkTags(chunkId, tags.map(t => t.id)))
        );
    }
    return Effect.void;
}),
```

Note: `updateChunk` needs `userId` passed through for `findOrCreateTag`. It already receives `userId` as a parameter.

- [ ] **Step 3: Verify chunk creation still works**

Run: `pnpm dev` then test creating a chunk with tags via the UI or curl.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/chunks/service.ts
git commit -m "feat: sync chunk tags to normalized tables on create/update"
```

---

### Task 9: Drop JSONB tags column from chunk table

**Files:**
- Modify: `packages/db/src/schema/chunk.ts`
- Modify: `packages/db/src/repository/chunk.ts`
- Modify: `packages/db/src/repository/graph.ts`
- Modify: `packages/api/src/chunks/service.ts`
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Remove `tags` column from chunk schema**

In `packages/db/src/schema/chunk.ts`, remove the line:
```typescript
tags: jsonb("tags").$type<string[]>().notNull().default([]),
```

- [ ] **Step 2: Update chunk repository**

In `packages/db/src/repository/chunk.ts`:
- Remove `tags` from all `select()` calls and `insert().values()` params
- Remove any `tags`-related filtering (e.g., JSONB contains queries)
- Update `CreateChunkParams` type to remove `tags`

- [ ] **Step 3: Update graph repository**

In `packages/db/src/repository/graph.ts`, remove `tags: chunk.tags` from the `getAllChunksMeta` select. Tags now come from `getAllTagsWithTypes`.

- [ ] **Step 4: Update chunk service**

In `packages/api/src/chunks/service.ts`:
- Remove `tags: body.tags ?? []` from `createChunkRepo()` calls
- Remove `tags: existing.tags as string[]` from `createVersion()` calls
- Keep the `findOrCreateTag` + `setChunkTags` syncing logic (this is now the only source of truth)

- [ ] **Step 5: Update chunk routes**

In `packages/api/src/chunks/routes.ts`, keep `tags` in the request body schema (it's still accepted as input), but it's only used for the normalized tables now, not stored in JSONB.

- [ ] **Step 6: Update chunk-version schema if it references tags**

Check `packages/db/src/schema/chunk-version.ts` — if it has a `tags` column, keep it (versions are historical snapshots). If the version `createVersion` call references `existing.tags`, update it to fetch tags from the `chunk_tag` join instead.

- [ ] **Step 7: Push schema changes**

Run: `pnpm db:push`
Expected: `tags` column dropped from `chunk` table.

- [ ] **Step 8: Run type checks**

Run: `pnpm run check-types`
Expected: No type errors (fix any remaining references to `chunk.tags`).

- [ ] **Step 9: Commit**

```bash
git add packages/db/src/schema/chunk.ts packages/db/src/repository/ packages/api/src/chunks/
git commit -m "feat: drop JSONB tags column from chunk table, use normalized tables only"
```

---

## Chunk 3: Frontend — Force Layout & Tag Grouping State

### Task 10: Update force layout to support tag-based clustering

**Files:**
- Modify: `apps/web/src/features/graph/force-layout.ts`
- Modify: `apps/web/src/features/graph/layout.worker.ts`

- [ ] **Step 1: Add tag group parameter to force layout**

Update `runForceLayout` signature and add tag centroid attraction:

```typescript
// Updated signature
export function runForceLayout(
    nodes: { id: string; type: string }[],
    edges: { source: string; target: string; relation: string }[],
    tagGroups?: Map<string, string[]> // tagValue -> nodeIds in that group
): Record<string, { x: number; y: number }> {
```

Add tag-based clustering force after the existing type centroid block (around line 102):

```typescript
// Tag-based clustering (when grouping is active)
if (tagGroups && tagGroups.size > 0) {
    const TAG_CLUSTER_K = 0.003; // Stronger than type clustering
    const tagCentroids = new Map<string, { x: number; y: number; count: number }>();
    for (const [tagValue, nodeIds] of tagGroups) {
        let cx = 0, cy = 0, count = 0;
        for (const nid of nodeIds) {
            const p = pos.get(nid);
            if (p) { cx += p.x; cy += p.y; count++; }
        }
        if (count > 0) tagCentroids.set(tagValue, { x: cx / count, y: cy / count, count });
    }
    for (const [tagValue, nodeIds] of tagGroups) {
        const c = tagCentroids.get(tagValue);
        if (!c) continue;
        for (const nid of nodeIds) {
            const p = pos.get(nid);
            if (!p) continue;
            p.vx += (c.x - p.x) * TAG_CLUSTER_K * temp;
            p.vy += (c.y - p.y) * TAG_CLUSTER_K * temp;
        }
    }
}
```

- [ ] **Step 2: Update worker interface**

Update `apps/web/src/features/graph/layout.worker.ts`:

```typescript
export interface LayoutWorkerInput {
    requestId: number;
    nodes: { id: string; type: string }[];
    edges: { source: string; target: string; relation: string }[];
    tagGroups?: Record<string, string[]>; // Serializable version
}

self.onmessage = (e: MessageEvent<LayoutWorkerInput>) => {
    const { requestId, nodes, edges, tagGroups } = e.data;
    const tagGroupsMap = tagGroups ? new Map(Object.entries(tagGroups)) : undefined;
    const positions = runForceLayout(nodes, edges, tagGroupsMap);
    self.postMessage({ requestId, positions } satisfies LayoutWorkerOutput);
};
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/force-layout.ts apps/web/src/features/graph/layout.worker.ts
git commit -m "feat: add tag-based clustering forces to force layout"
```

---

### Task 11: Add tag grouping state and filter UI

**Files:**
- Modify: `apps/web/src/features/graph/graph-filters.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Extend GraphFilters with tag type section**

Update `apps/web/src/features/graph/graph-filters.tsx` to accept tag type props and render a "Group by Tag Type" section:

```typescript
import { Badge } from "@/components/ui/badge";

interface TagTypeInfo {
    id: string;
    name: string;
    color: string;
    tagCount: number;
}

export function GraphFilters({
    types,
    relations,
    activeTypes,
    activeRelations,
    onToggleType,
    onToggleRelation,
    tagTypes,
    activeTagTypeIds,
    onToggleTagType
}: {
    types: string[];
    relations: string[];
    activeTypes: Set<string>;
    activeRelations: Set<string>;
    onToggleType: (type: string) => void;
    onToggleRelation: (relation: string) => void;
    tagTypes?: TagTypeInfo[];
    activeTagTypeIds?: Set<string>;
    onToggleTagType?: (id: string) => void;
}) {
    return (
        <div className="bg-background/80 absolute top-4 left-4 z-10 max-w-[200px] space-y-3 rounded-lg border p-3 backdrop-blur-sm">
            <div>
                <p className="text-muted-foreground mb-1.5 text-[10px] font-medium uppercase">Types</p>
                <div className="flex flex-wrap gap-1">
                    {types.map(t => (
                        <Badge
                            key={t}
                            variant={activeTypes.has(t) ? "default" : "outline"}
                            size="sm"
                            className="cursor-pointer text-[10px]"
                            onClick={() => onToggleType(t)}
                        >
                            {t}
                        </Badge>
                    ))}
                </div>
            </div>
            {relations.length > 0 && (
                <div>
                    <p className="text-muted-foreground mb-1.5 text-[10px] font-medium uppercase">Relations</p>
                    <div className="flex flex-wrap gap-1">
                        {relations.map(r => (
                            <Badge
                                key={r}
                                variant={activeRelations.has(r) ? "default" : "outline"}
                                size="sm"
                                className="cursor-pointer text-[10px]"
                                onClick={() => onToggleRelation(r)}
                            >
                                {r}
                            </Badge>
                        ))}
                    </div>
                </div>
            )}
            {tagTypes && tagTypes.length > 0 && onToggleTagType && activeTagTypeIds && (
                <div className="border-t pt-3">
                    <p className="text-muted-foreground mb-1.5 text-[10px] font-medium uppercase">Group by Tag Type</p>
                    <div className="flex flex-wrap gap-1">
                        {tagTypes.map(tt => (
                            <Badge
                                key={tt.id}
                                variant={activeTagTypeIds.has(tt.id) ? "default" : "outline"}
                                size="sm"
                                className="cursor-pointer text-[10px]"
                                style={
                                    activeTagTypeIds.has(tt.id)
                                        ? { backgroundColor: tt.color, borderColor: tt.color, color: "#fff" }
                                        : { borderColor: tt.color, color: tt.color }
                                }
                                onClick={() => onToggleTagType(tt.id)}
                            >
                                {tt.name}
                            </Badge>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Add tag grouping state to graph-view.tsx**

In `GraphViewInner`, add state and computed values. After the existing filter state declarations (around line 198-200):

```typescript
// Tag grouping state
const [activeTagTypeIds, setActiveTagTypeIds] = useState<Set<string>>(new Set());

function toggleTagType(id: string) {
    setActiveTagTypeIds(prev => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
    });
}
```

Add memoized tag type info and tag group mapping from the graph data. After `allRelations` memo:

```typescript
// Tag types for the filter panel
const tagTypeInfos = useMemo(() => {
    if (!data?.tagTypes) return [];
    const tagCountByType = new Map<string, number>();
    for (const ct of data.chunkTags ?? []) {
        if (ct.tagTypeId) {
            tagCountByType.set(ct.tagTypeId, (tagCountByType.get(ct.tagTypeId) ?? 0) + 1);
        }
    }
    return data.tagTypes.map(tt => ({
        id: tt.id,
        name: tt.name,
        color: tt.color,
        tagCount: tagCountByType.get(tt.id) ?? 0
    }));
}, [data]);

// Build tag groups: tagName -> chunkIds (for active tag types only)
const tagGroups = useMemo(() => {
    if (activeTagTypeIds.size === 0 || !data?.chunkTags) return null;
    const groups = new Map<string, string[]>();
    for (const ct of data.chunkTags) {
        if (!ct.tagTypeId || !activeTagTypeIds.has(ct.tagTypeId)) continue;
        if (!groups.has(ct.tagName)) groups.set(ct.tagName, []);
        groups.get(ct.tagName)!.push(ct.chunkId);
    }
    return groups;
}, [activeTagTypeIds, data]);

// Build chunk-to-tag-group lookup for edge opacity
const chunkTagGroupMap = useMemo(() => {
    if (!tagGroups) return null;
    const map = new Map<string, Set<string>>(); // chunkId -> Set of tagNames
    for (const [tagName, chunkIds] of tagGroups) {
        for (const cid of chunkIds) {
            if (!map.has(cid)) map.set(cid, new Set());
            map.get(cid)!.add(tagName);
        }
    }
    return map;
}, [tagGroups]);
```

- [ ] **Step 3: Pass tag groups to layout worker**

In the `useEffect` that posts to the worker (around line 354), add `tagGroups` to the worker message:

```typescript
// Inside the force layout branch:
const tagGroupsObj = tagGroups ? Object.fromEntries(tagGroups) : undefined;

// Worker path:
workerRef.current.postMessage({
    requestId: requestIdRef.current,
    nodes: workerNodes,
    edges: workerEdges,
    tagGroups: tagGroupsObj
} satisfies LayoutWorkerInput);

// Main thread path:
const tagGroupsMap = tagGroups ?? undefined;
const positions = runForceLayout(workerNodes, workerEdges, tagGroupsMap);
```

Add `tagGroups` to the dependency array of this effect.

- [ ] **Step 4: Pass tag type props to GraphFilters**

Update the `<GraphFilters>` usage in the JSX (around line 1147):

```tsx
<GraphFilters
    types={allTypes}
    relations={allRelations}
    activeTypes={filterTypes}
    activeRelations={filterRelations}
    onToggleType={toggleType}
    onToggleRelation={toggleRelation}
    tagTypes={tagTypeInfos}
    activeTagTypeIds={activeTagTypeIds}
    onToggleTagType={toggleTagType}
/>
```

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/graph/graph-filters.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "feat: add tag type grouping state and filter panel UI"
```

---

## Chunk 4: Frontend — Edge Opacity & Convex Hull Regions

### Task 12: Apply cross-group edge opacity

**Files:**
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Add tag grouping to edge styling logic**

In the consolidated node/edge styling `useEffect` (around line 671), add a new condition for tag grouping. In the edge styling section, add a branch before the final `else` fallback (before `styledEdges = layoutEdges`):

```typescript
// After the existing selectedEdgeIds branch, before the final else:
} else if (chunkTagGroupMap && chunkTagGroupMap.size > 0) {
    // Tag grouping active: dim cross-group edges
    styledEdges = layoutEdges.map(edge => {
        const sourceGroups = chunkTagGroupMap.get(edge.source);
        const targetGroups = chunkTagGroupMap.get(edge.target);
        // Both in same group = full opacity
        let sameGroup = false;
        if (sourceGroups && targetGroups) {
            for (const g of sourceGroups) {
                if (targetGroups.has(g)) { sameGroup = true; break; }
            }
        }
        return {
            ...edge,
            style: {
                ...(edge.style as Record<string, unknown>),
                opacity: sameGroup ? 1 : 0.15,
                transition: "opacity 0.3s ease"
            }
        };
    });
} else {
    styledEdges = layoutEdges;
}
```

- [ ] **Step 2: Override opacity for selected node's direct connections**

After the tag grouping edge styling, add an override when a node is selected:

```typescript
// After computing styledEdges with tag grouping, override for selected node
if (chunkTagGroupMap && chunkTagGroupMap.size > 0 && selectedChunkId) {
    const selectedDirectEdgeIds = new Set<string>();
    const selectedDirectNodeIds = new Set<string>([selectedChunkId]);
    for (const edge of layoutEdges) {
        if (edge.source === selectedChunkId || edge.target === selectedChunkId) {
            selectedDirectEdgeIds.add(edge.id);
            selectedDirectNodeIds.add(edge.source);
            selectedDirectNodeIds.add(edge.target);
        }
    }
    styledEdges = styledEdges.map(edge => {
        if (selectedDirectEdgeIds.has(edge.id)) {
            return { ...edge, style: { ...(edge.style as Record<string, unknown>), opacity: 1 } };
        }
        return edge;
    });
}
```

Also ensure the node styling dims ungrouped nodes slightly when grouping is active. Add a branch for nodes:

```typescript
// In node styling, before the final else:
} else if (chunkTagGroupMap && chunkTagGroupMap.size > 0) {
    styledNodes = layoutNodes.map(node => {
        if (node.id === MAIN_NODE_ID) return node;
        const inGroup = chunkTagGroupMap.has(node.id);
        return {
            ...node,
            style: {
                ...(node.style as Record<string, unknown>),
                opacity: inGroup ? 1 : 0.4,
                transition: "opacity 0.2s"
            }
        };
    });
}
```

Add `chunkTagGroupMap` and `selectedChunkId` to the effect's dependency array.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/graph-view.tsx
git commit -m "feat: apply cross-group edge dimming and selected node override"
```

---

### Task 13: Create convex hull SVG background regions

**Files:**
- Create: `apps/web/src/features/graph/graph-tag-regions.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create the convex hull region component**

```typescript
// apps/web/src/features/graph/graph-tag-regions.tsx
import { useReactFlow } from "@xyflow/react";
import { useMemo } from "react";

interface TagRegion {
    tagName: string;
    color: string;
    nodeIds: string[];
}

function convexHull(points: { x: number; y: number }[]): { x: number; y: number }[] {
    if (points.length <= 1) return points;
    const sorted = [...points].sort((a, b) => a.x - b.x || a.y - b.y);

    function cross(o: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }) {
        return (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    }

    const lower: { x: number; y: number }[] = [];
    for (const p of sorted) {
        while (lower.length >= 2 && cross(lower[lower.length - 2]!, lower[lower.length - 1]!, p) <= 0) lower.pop();
        lower.push(p);
    }

    const upper: { x: number; y: number }[] = [];
    for (const p of sorted.reverse()) {
        while (upper.length >= 2 && cross(upper[upper.length - 2]!, upper[upper.length - 1]!, p) <= 0) upper.pop();
        upper.push(p);
    }

    lower.pop();
    upper.pop();
    return lower.concat(upper);
}

function hullToPath(hull: { x: number; y: number }[], padding: number): string {
    if (hull.length === 0) return "";
    if (hull.length === 1) {
        const p = hull[0]!;
        return `M ${p.x - padding} ${p.y} A ${padding} ${padding} 0 1 0 ${p.x + padding} ${p.y} A ${padding} ${padding} 0 1 0 ${p.x - padding} ${p.y} Z`;
    }
    if (hull.length === 2) {
        const [a, b] = hull as [{ x: number; y: number }, { x: number; y: number }];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const len = Math.sqrt(dx * dx + dy * dy);
        const nx = (-dy / len) * padding;
        const ny = (dx / len) * padding;
        return `M ${a.x + nx} ${a.y + ny} L ${b.x + nx} ${b.y + ny} A ${padding} ${padding} 0 0 1 ${b.x - nx} ${b.y - ny} L ${a.x - nx} ${a.y - ny} A ${padding} ${padding} 0 0 1 ${a.x + nx} ${a.y + ny} Z`;
    }

    // Expand hull outward by padding
    const expanded: { x: number; y: number }[] = [];
    for (let i = 0; i < hull.length; i++) {
        const prev = hull[(i - 1 + hull.length) % hull.length]!;
        const curr = hull[i]!;
        const next = hull[(i + 1) % hull.length]!;

        const dx1 = curr.x - prev.x;
        const dy1 = curr.y - prev.y;
        const dx2 = next.x - curr.x;
        const dy2 = next.y - curr.y;

        const nx = -(dy1 + dy2) / 2;
        const ny = (dx1 + dx2) / 2;
        const len = Math.sqrt(nx * nx + ny * ny) || 1;

        expanded.push({ x: curr.x + (nx / len) * padding, y: curr.y + (ny / len) * padding });
    }

    // Create smooth path with rounded corners
    const pts = expanded;
    let d = `M ${pts[0]!.x} ${pts[0]!.y}`;
    for (let i = 1; i < pts.length; i++) {
        d += ` L ${pts[i]!.x} ${pts[i]!.y}`;
    }
    d += " Z";
    return d;
}

export function GraphTagRegions({
    regions,
    nodePositions
}: {
    regions: TagRegion[];
    nodePositions: Map<string, { x: number; y: number; width: number; height: number }>;
}) {
    const { getViewport } = useReactFlow();

    const paths = useMemo(() => {
        return regions
            .map(region => {
                const points: { x: number; y: number }[] = [];
                for (const nid of region.nodeIds) {
                    const pos = nodePositions.get(nid);
                    if (pos) {
                        // Use center of node
                        points.push({ x: pos.x + pos.width / 2, y: pos.y + pos.height / 2 });
                    }
                }
                if (points.length === 0) return null;
                const hull = convexHull(points);
                const path = hullToPath(hull, 60); // 60px padding around hull
                return { ...region, path };
            })
            .filter(Boolean) as (TagRegion & { path: string })[];
    }, [regions, nodePositions]);

    const viewport = getViewport();

    return (
        <svg
            className="pointer-events-none absolute inset-0 z-0"
            style={{
                transform: `translate(${viewport.x}px, ${viewport.y}px) scale(${viewport.zoom})`,
                transformOrigin: "0 0"
            }}
        >
            {paths.map(region => (
                <g key={region.tagName}>
                    <path d={region.path} fill={region.color} fillOpacity={0.06} stroke={region.color} strokeOpacity={0.2} strokeWidth={1 / viewport.zoom} />
                    {/* Label */}
                    <text
                        x={region.nodeIds.length > 0 ? (() => {
                            let minX = Infinity;
                            let minY = Infinity;
                            for (const nid of region.nodeIds) {
                                const pos = nodePositions.get(nid);
                                if (pos && pos.x < minX) { minX = pos.x; minY = pos.y; }
                            }
                            return minX;
                        })() : 0}
                        y={(() => {
                            let minY = Infinity;
                            for (const nid of region.nodeIds) {
                                const pos = nodePositions.get(nid);
                                if (pos && pos.y < minY) minY = pos.y;
                            }
                            return minY - 40;
                        })()}
                        fill={region.color}
                        fillOpacity={0.6}
                        fontSize={12 / viewport.zoom}
                        fontWeight={600}
                        textTransform="uppercase"
                        letterSpacing="0.1em"
                    >
                        {region.tagName}
                    </text>
                </g>
            ))}
        </svg>
    );
}
```

- [ ] **Step 2: Integrate into graph-view.tsx**

Import and render `GraphTagRegions` inside the ReactFlow container. Add it inside the ReactFlow panel area, before the `<Background>` component.

First, compute node positions as a Map for the region component. Add a memo:

```typescript
const nodePositionMap = useMemo(() => {
    const map = new Map<string, { x: number; y: number; width: number; height: number }>();
    for (const node of nodes) {
        map.set(node.id, {
            x: node.position.x,
            y: node.position.y,
            width: node.measured?.width ?? 150,
            height: node.measured?.height ?? 40
        });
    }
    return map;
}, [nodes]);

const tagRegions = useMemo(() => {
    if (!tagGroups || !data?.chunkTags) return [];
    // Build tagTypeId -> color lookup
    const typeColorMap = new Map<string, string>();
    for (const tt of data.tagTypes ?? []) {
        typeColorMap.set(tt.id, tt.color);
    }
    // Build regions
    const regions: { tagName: string; color: string; nodeIds: string[] }[] = [];
    for (const [tagName, nodeIds] of tagGroups) {
        // Find the tag type color for this tag
        const tagEntry = data.chunkTags.find(ct => ct.tagName === tagName && ct.tagTypeId);
        const color = tagEntry?.tagTypeColor ?? "#8b5cf6";
        regions.push({ tagName, color, nodeIds });
    }
    return regions;
}, [tagGroups, data]);
```

Render as a sibling *outside* the `<ReactFlow>` component but inside the same wrapper div (`relative flex-1`). The SVG applies its own viewport transform, so it must NOT be inside `<ReactFlow>` (which would double-apply the transform). Place it after the `</ReactFlow>` closing tag but inside the wrapper div:

```tsx
{tagRegions.length > 0 && (
    <GraphTagRegions regions={tagRegions} nodePositions={nodePositionMap} />
)}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/graph-tag-regions.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "feat: add convex hull SVG background regions for tag groups"
```

---

### Task 14: Handle node duplication for multi-tag chunks

**Files:**
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Add ghost node generation**

When tag grouping is active, chunks belonging to multiple tag groups need duplicate "ghost" nodes. Add this logic in the `filteredGraph` memo or the node generation memo.

In the `{ layoutNodes, layoutEdges }` memo (around line 417), after building `rawNodes`, insert ghost node logic:

```typescript
// After building rawNodes from chunks (line ~480), before rawEdges:
// Ghost nodes for multi-tag chunks
if (tagGroups && tagGroups.size > 0) {
    const chunkTagCount = new Map<string, string[]>(); // chunkId -> tagNames
    for (const [tagName, nodeIds] of tagGroups) {
        for (const nid of nodeIds) {
            if (!chunkTagCount.has(nid)) chunkTagCount.set(nid, []);
            chunkTagCount.get(nid)!.push(tagName);
        }
    }
    const ghostNodes: typeof rawNodes = [];
    for (const [chunkId, tagNames] of chunkTagCount) {
        if (tagNames.length <= 1) continue;
        const original = rawNodes.find(n => n.id === chunkId);
        if (!original) continue;
        // First tag keeps the original node; subsequent tags get ghosts
        for (let i = 1; i < tagNames.length; i++) {
            const ghostId = `${chunkId}__ghost__${tagNames[i]}`;
            ghostNodes.push({
                ...original,
                id: ghostId,
                data: { ...original.data, isGhost: true, originalId: chunkId },
                style: {
                    ...(original.style as Record<string, unknown>),
                    borderStyle: "dashed",
                    opacity: 0.65
                }
            });
        }
    }
    rawNodes.push(...ghostNodes);
}
```

- [ ] **Step 2: Handle ghost node click to select original**

In the `onNodeClick` handler (around line 983), add ghost resolution:

```typescript
onNodeClick={(event, node) => {
    // Resolve ghost node to original
    const actualId = (node.data as { originalId?: string }).originalId ?? node.id;
    if (actualId === MAIN_NODE_ID) return;
    // ... rest of handler uses actualId instead of node.id
```

- [ ] **Step 3: Create ghost-aware tag groups for layout**

Add a memo that extends `tagGroups` with ghost node IDs so they cluster correctly. Place this after both `tagGroups` and the ghost node generation:

```typescript
const tagGroupsWithGhosts = useMemo(() => {
    if (!tagGroups) return null;
    const groups = new Map(tagGroups); // clone
    // Ghost nodes use ID format: `${chunkId}__ghost__${tagName}`
    // Add each ghost to its tag group
    for (const [tagName, nodeIds] of tagGroups) {
        const withGhosts = [...nodeIds];
        // Find chunks that have multiple tags — their ghosts for THIS tag need adding
        for (const [otherTagName, otherNodeIds] of tagGroups) {
            if (otherTagName === tagName) continue;
            for (const nid of otherNodeIds) {
                if (nodeIds.includes(nid)) {
                    // This node is in both groups; ghost for tagName exists on the other group's copy
                    // But the ghost ID for THIS group is: `${nid}__ghost__${tagName}` (only if tagName is not the first tag)
                }
            }
        }
        groups.set(tagName, withGhosts);
    }
    // Simpler approach: scan layoutNodes for ghost nodes and add them
    for (const node of layoutNodes) {
        const ghostTag = node.id.match(/__ghost__(.+)$/)?.[1];
        if (ghostTag && groups.has(ghostTag)) {
            groups.get(ghostTag)!.push(node.id);
        }
    }
    return groups;
}, [tagGroups, layoutNodes]);
```

Pass `tagGroupsWithGhosts` (instead of `tagGroups`) to the layout worker and to edge opacity calculations.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/graph-view.tsx
git commit -m "feat: add ghost node duplication for multi-tag chunks"
```

---

## Chunk 5: Integration & Polish

### Task 15: Update graph-view.tsx to track viewport for SVG regions

**Files:**
- Modify: `apps/web/src/features/graph/graph-view.tsx`
- Modify: `apps/web/src/features/graph/graph-tag-regions.tsx`

- [ ] **Step 1: Pass viewport to tag regions**

The `GraphTagRegions` component needs to update when the viewport changes (pan/zoom). Use the `onMoveEnd` callback to trigger re-renders, or use `useStore` from @xyflow/react to subscribe to viewport changes.

Update `GraphTagRegions` to use `useStore` for reactive viewport:

```typescript
import { useStore } from "@xyflow/react";

// Inside the component:
const viewport = useStore(s => ({ x: s.transform[0], y: s.transform[1], zoom: s.transform[2] }));
```

This replaces the `getViewport()` call and makes the SVG update reactively.

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/graph/graph-tag-regions.tsx
git commit -m "fix: use reactive viewport store for tag region SVG transforms"
```

---

### Task 16: End-to-end verification

- [ ] **Step 1: Run type checks**

Run: `pnpm run check-types`
Expected: No type errors.

- [ ] **Step 2: Run tests**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 3: Run lint**

Run: `pnpm ci`
Expected: All checks pass.

- [ ] **Step 4: Manual verification**

1. Start dev server: `pnpm dev`
2. Create tag types via API: `curl -X POST http://localhost:3000/api/tag-types -H 'Content-Type: application/json' -d '{"name":"feature","color":"#f59e0b"}'`
3. Create tags and assign to chunks
4. Open graph view, verify "Group by Tag Type" section appears in filters
5. Click a tag type — verify nodes cluster, background regions appear, cross-group edges dim
6. Click a node — verify its direct connections restore to full opacity
7. Verify multi-tag chunks appear as ghosts in multiple groups

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration fixes for tag type grouping"
```
