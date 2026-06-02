# Codebase → Space Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the `codebase` entity to `space`, introduce a `kind` discriminator, and move git-specific fields (`remote_url`, `local_paths`) into a 1:1 side-table `space_code_metadata`. Preserves all existing data, all code-knowledge features remain functional after kind defaults to `code`.

**Architecture:** Schema-first migration via a hand-written SQL migration file (matches the existing `packages/db/src/migrations/*.sql` pattern). New `space`, `space_kind`, `space_code_metadata` tables; FK columns `codebase_id` rename to `space_id` across 17 schema files and one chunk join table. App-layer rename follows in repository → service → routes → CLI → frontend → VS Code → seed → tests order, with a backwards-compat `/api/codebases` alias kept until the VS Code extension is rebuilt.

**Tech Stack:** TypeScript, Drizzle ORM 0.45 (Postgres), Elysia + Eden treaty, Effect, Bun (CLI), TanStack Start (web), Commander.js, vitest.

**Spec:** `docs/superpowers/specs/2026-06-02-domain-agnostic-database-design.md` (Phase A only).

---

## Phase 0: Pre-flight

### Task 0.1: Create a worktree (optional but recommended)

**Files:**
- None (worktree creation)

- [ ] **Step 1: Create isolated worktree**

This rename is wide; isolating it makes it easier to abandon if needed.

```bash
git worktree add ../fubbik-codebase-to-space -b feat/codebase-to-space
cd ../fubbik-codebase-to-space
pnpm install
```

Skip this step if you prefer to work in the current branch.

- [ ] **Step 2: Confirm baseline is green**

```bash
pnpm ci
```

Expected: passes. If failures are pre-existing, capture them so you can distinguish from regressions you introduce.

---

## Phase 1: New schema files (additive — no removal yet)

### Task 1.1: Add `space_kind` schema

**Files:**
- Create: `packages/db/src/schema/space-kind.ts`

- [ ] **Step 1: Write the schema file**

```ts
import { boolean, integer, pgTable, text, timestamp } from "drizzle-orm/pg-core";

export const spaceKind = pgTable("space_kind", {
    id: text("id").primaryKey(), // 'code' | 'wiki' | 'notes' | 'research' | ...
    label: text("label").notNull(),
    description: text("description"),
    icon: text("icon"),
    displayOrder: integer("display_order").notNull().default(100),
    builtIn: boolean("built_in").notNull().default(false),
    createdAt: timestamp("created_at").defaultNow().notNull(),
    updatedAt: timestamp("updated_at")
        .defaultNow()
        .$onUpdate(() => new Date())
        .notNull()
});
```

- [ ] **Step 2: Commit**

```bash
git add packages/db/src/schema/space-kind.ts
git commit -m "feat(db): add space_kind schema"
```

### Task 1.2: Add `space` schema

**Files:**
- Create: `packages/db/src/schema/space.ts`

- [ ] **Step 1: Write the schema file**

```ts
import { relations } from "drizzle-orm";
import { index, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";
import { spaceKind } from "./space-kind";

export const space = pgTable(
    "space",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        kind: text("kind").notNull().references(() => spaceKind.id, { onDelete: "restrict" }),
        description: text("description"),
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
        uniqueIndex("space_user_name_idx").on(table.userId, table.name),
        index("space_userId_idx").on(table.userId),
        index("space_kind_idx").on(table.kind)
    ]
);

export const chunkSpace = pgTable(
    "chunk_space",
    {
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        spaceId: text("space_id")
            .notNull()
            .references(() => space.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.chunkId, table.spaceId] }),
        index("chunk_space_chunkId_idx").on(table.chunkId),
        index("chunk_space_spaceId_idx").on(table.spaceId)
    ]
);

export const spaceRelations = relations(space, ({ one, many }) => ({
    user: one(user, { fields: [space.userId], references: [user.id] }),
    spaceKind: one(spaceKind, { fields: [space.kind], references: [spaceKind.id] }),
    chunkSpaces: many(chunkSpace)
}));

export const chunkSpaceRelations = relations(chunkSpace, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkSpace.chunkId], references: [chunk.id] }),
    space: one(space, { fields: [chunkSpace.spaceId], references: [space.id] })
}));
```

- [ ] **Step 2: Commit**

```bash
git add packages/db/src/schema/space.ts
git commit -m "feat(db): add space + chunk_space schema"
```

### Task 1.3: Add `space_code_metadata` schema

**Files:**
- Create: `packages/db/src/schema/space-code-metadata.ts`

- [ ] **Step 1: Write the schema file**

```ts
import { relations, sql } from "drizzle-orm";
import { jsonb, pgTable, text, uniqueIndex } from "drizzle-orm/pg-core";

import { space } from "./space";

export const spaceCodeMetadata = pgTable(
    "space_code_metadata",
    {
        spaceId: text("space_id")
            .primaryKey()
            .references(() => space.id, { onDelete: "cascade" }),
        userId: text("user_id").notNull(),
        remoteUrl: text("remote_url"),
        localPaths: jsonb("local_paths").$type<string[]>().notNull().default([])
    },
    table => [
        uniqueIndex("space_code_user_remote_idx")
            .on(table.userId, table.remoteUrl)
            .where(sql`"remote_url" IS NOT NULL`)
    ]
);

export const spaceCodeMetadataRelations = relations(spaceCodeMetadata, ({ one }) => ({
    space: one(space, { fields: [spaceCodeMetadata.spaceId], references: [space.id] })
}));
```

Note: `userId` is denormalised here so the partial unique index works. The repository layer is responsible for keeping it in sync with `space.userId`.

- [ ] **Step 2: Commit**

```bash
git add packages/db/src/schema/space-code-metadata.ts
git commit -m "feat(db): add space_code_metadata schema"
```

### Task 1.4: Re-export new schemas from index

**Files:**
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Add exports**

Insert these lines (anywhere in the file, sorted near existing entries):

```ts
export * from "./space";
export * from "./space-kind";
export * from "./space-code-metadata";
```

- [ ] **Step 2: Verify type-check**

```bash
pnpm --filter @fubbik/db check-types
```

Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/schema/index.ts
git commit -m "feat(db): re-export space schemas"
```

---

## Phase 2: Data migration

### Task 2.1: Write the SQL migration

**Files:**
- Create: `packages/db/src/migrations/0004_codebase_to_space.sql`

- [ ] **Step 1: Write the migration**

```sql
-- Phase A of domain-agnostic-database: codebase → space rename.
-- Idempotent (uses IF NOT EXISTS / IF EXISTS where possible).

-- 1. space_kind lookup table + seeds
CREATE TABLE IF NOT EXISTS space_kind (
    id text PRIMARY KEY,
    label text NOT NULL,
    description text,
    icon text,
    display_order integer NOT NULL DEFAULT 100,
    built_in boolean NOT NULL DEFAULT false,
    created_at timestamp NOT NULL DEFAULT now(),
    updated_at timestamp NOT NULL DEFAULT now()
);

INSERT INTO space_kind (id, label, description, icon, display_order, built_in) VALUES
    ('code',     'Code',     'A code repository or codebase',                 'Code',       10, true),
    ('wiki',     'Wiki',     'A general-purpose knowledge wiki',              'BookOpen',   20, true),
    ('notes',    'Notes',    'Personal notes, journals, or scratch space',    'Notebook',   30, true),
    ('research', 'Research', 'Research artefacts, references, and findings', 'FlaskConical', 40, true)
ON CONFLICT (id) DO NOTHING;

-- 2. space table
CREATE TABLE IF NOT EXISTS space (
    id text PRIMARY KEY,
    name text NOT NULL,
    kind text NOT NULL REFERENCES space_kind(id) ON DELETE RESTRICT,
    description text,
    user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    created_at timestamp NOT NULL DEFAULT now(),
    updated_at timestamp NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS space_user_name_idx ON space(user_id, name);
CREATE INDEX IF NOT EXISTS space_userId_idx ON space(user_id);
CREATE INDEX IF NOT EXISTS space_kind_idx ON space(kind);

-- 3. space_code_metadata side-table
CREATE TABLE IF NOT EXISTS space_code_metadata (
    space_id text PRIMARY KEY REFERENCES space(id) ON DELETE CASCADE,
    user_id text NOT NULL,
    remote_url text,
    local_paths jsonb NOT NULL DEFAULT '[]'::jsonb
);
CREATE UNIQUE INDEX IF NOT EXISTS space_code_user_remote_idx
    ON space_code_metadata(user_id, remote_url) WHERE remote_url IS NOT NULL;

-- 4. Copy codebase → space + space_code_metadata
INSERT INTO space (id, name, kind, description, user_id, created_at, updated_at)
SELECT id, name, 'code', NULL, user_id, created_at, updated_at
FROM codebase
ON CONFLICT (id) DO NOTHING;

INSERT INTO space_code_metadata (space_id, user_id, remote_url, local_paths)
SELECT id, user_id, remote_url, COALESCE(local_paths, '[]'::jsonb)
FROM codebase
ON CONFLICT (space_id) DO NOTHING;

-- 5. chunk_space (renamed from chunk_codebase) — recreate as new join table
CREATE TABLE IF NOT EXISTS chunk_space (
    chunk_id text NOT NULL REFERENCES chunk(id) ON DELETE CASCADE,
    space_id text NOT NULL REFERENCES space(id) ON DELETE CASCADE,
    PRIMARY KEY (chunk_id, space_id)
);
CREATE INDEX IF NOT EXISTS chunk_space_chunkId_idx ON chunk_space(chunk_id);
CREATE INDEX IF NOT EXISTS chunk_space_spaceId_idx ON chunk_space(space_id);

INSERT INTO chunk_space (chunk_id, space_id)
SELECT chunk_id, codebase_id FROM chunk_codebase
ON CONFLICT DO NOTHING;

-- 6. workspace_space (renamed from workspace_codebase)
CREATE TABLE IF NOT EXISTS workspace_space (
    workspace_id text NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
    space_id text NOT NULL REFERENCES space(id) ON DELETE CASCADE,
    PRIMARY KEY (workspace_id, space_id)
);
CREATE INDEX IF NOT EXISTS workspace_space_workspaceId_idx ON workspace_space(workspace_id);
CREATE INDEX IF NOT EXISTS workspace_space_spaceId_idx ON workspace_space(space_id);

INSERT INTO workspace_space (workspace_id, space_id)
SELECT workspace_id, codebase_id FROM workspace_codebase
ON CONFLICT DO NOTHING;

-- 7. Rename codebase_id → space_id on every table that has it
-- These ALTERs are guarded so they're safe to re-run.
DO $$
DECLARE
    tbl text;
    tables text[] := ARRAY[
        'document', 'plan', 'requirement', 'use_case', 'collection',
        'feature', 'chunk_type', 'connection_relation', 'saved_graph',
        'saved_query', 'activity', 'settings', 'staleness_scan',
        'vocabulary', 'behavior_matrix'
    ];
BEGIN
    FOREACH tbl IN ARRAY tables LOOP
        IF EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = tbl AND column_name = 'codebase_id'
        ) AND NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = tbl AND column_name = 'space_id'
        ) THEN
            EXECUTE format('ALTER TABLE %I RENAME COLUMN codebase_id TO space_id', tbl);
            -- Drop and recreate FK to point at space
            EXECUTE format(
                'ALTER TABLE %I DROP CONSTRAINT IF EXISTS %I',
                tbl,
                tbl || '_codebase_id_codebase_id_fk'
            );
            EXECUTE format(
                'ALTER TABLE %I ADD CONSTRAINT %I FOREIGN KEY (space_id) REFERENCES space(id) ON DELETE %s',
                tbl,
                tbl || '_space_id_space_fk',
                CASE WHEN tbl IN ('staleness_scan') THEN 'CASCADE' ELSE 'SET NULL' END
            );
        END IF;
    END LOOP;
END $$;

-- 8. Drop old join tables and codebase
DROP TABLE IF EXISTS chunk_codebase;
DROP TABLE IF EXISTS workspace_codebase;
DROP TABLE IF EXISTS codebase;
```

Notes:
- `staleness_scan.last_commit_sha` stays — it's only populated for code spaces.
- The FK delete behaviour mirrors the existing schemas (mostly `SET NULL`; `staleness_scan` was `CASCADE`). If a table uses a different `onDelete` in its schema, adjust the `CASE` clause above accordingly when you find it.
- `behavior_matrix` is included even though its schema file may use a different naming convention — verify before running.

- [ ] **Step 2: Cross-check FK delete behaviour for each renamed table**

Before running the migration, open each schema file with a `codebaseId` FK and note its `onDelete` directive. If any uses something other than `set null` or `cascade`, update the `CASE` clause in step 7 of the SQL.

```bash
grep -n "codebase_id\|codebaseId" packages/db/src/schema/*.ts
```

- [ ] **Step 3: Commit (do not run yet)**

```bash
git add packages/db/src/migrations/0004_codebase_to_space.sql
git commit -m "feat(db): migration 0004 — codebase to space rename"
```

### Task 2.2: Add migration runner helper to package.json

**Files:**
- Modify: `packages/db/package.json`

- [ ] **Step 1: Add a script for running this migration**

In `scripts`, add:

```json
"db:migrate:0004": "psql $DATABASE_URL -f src/migrations/0004_codebase_to_space.sql"
```

(matches the pattern of `db:migrate:trgm`).

- [ ] **Step 2: Commit**

```bash
git add packages/db/package.json
git commit -m "chore(db): add 0004 migration script"
```

### Task 2.3: Run the migration against a fresh dev DB

**Files:**
- None (DB operation)

- [ ] **Step 1: Reset the dev DB**

```bash
pnpm db:down && pnpm --filter @fubbik/db db:start
# wait a few seconds for Postgres
pnpm --filter @fubbik/db db:push
pnpm seed
```

- [ ] **Step 2: Apply the migration**

```bash
pnpm --filter @fubbik/db db:migrate:0004
```

Expected: no errors. The SQL `DO $$` block silently no-ops for already-renamed tables.

- [ ] **Step 3: Sanity-check the data**

```bash
psql $DATABASE_URL -c "SELECT count(*) FROM space; SELECT count(*) FROM space_code_metadata; SELECT count(*) FROM chunk_space; SELECT count(*) FROM workspace_space;"
```

Expected: counts match what `codebase`, `chunk_codebase`, and `workspace_codebase` had before (re-run `pnpm seed` and the migration if numbers look off).

```bash
psql $DATABASE_URL -c "\d staleness_scan" | grep space_id
```

Expected: `staleness_scan` shows a `space_id` column.

---

## Phase 3: Update existing schema files to reference space

The migration already renamed columns in the database. Now the Drizzle schema files must match.

### Task 3.1: Update `document.ts` schema

**Files:**
- Modify: `packages/db/src/schema/document.ts`

- [ ] **Step 1: Replace codebase references**

Change `import { codebase } from "./codebase";` → `import { space } from "./space";`.

Rename the column:
```ts
spaceId: text("space_id").references(() => space.id, { onDelete: "set null" }),
```
(replacing `codebaseId: text("codebase_id").references(() => codebase.id, ...)`)

Rename the index `document_codebaseId_idx` → `document_spaceId_idx`.

Update the relations block: `codebase: one(codebase, ...)` → `space: one(space, { fields: [document.spaceId], references: [space.id] })`.

Update the unique index column from `table.codebaseId` to `table.spaceId`.

- [ ] **Step 2: Type-check**

```bash
pnpm --filter @fubbik/db check-types
```

Expected: errors about other files importing `document.codebaseId` — that's expected; we'll fix them as we go.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/schema/document.ts
git commit -m "refactor(db): document references space"
```

### Task 3.2: Update remaining schema files

For each file below, repeat the same pattern: change `codebase` import → `space`, rename `codebaseId` → `spaceId`, rename indexes, update relations. **Show your work in a single commit per file.**

**Files to update (one task step each):**

- [ ] `packages/db/src/schema/workspace.ts` (rename `workspaceCodebase` → `workspaceSpace`; column `codebaseId` → `spaceId`; index names; relations)
- [ ] `packages/db/src/schema/staleness.ts` (`staleness_scan.codebaseId` → `spaceId`; keep `last_commit_sha` as-is)
- [ ] `packages/db/src/schema/plan.ts`
- [ ] `packages/db/src/schema/requirement.ts`
- [ ] `packages/db/src/schema/use-case.ts`
- [ ] `packages/db/src/schema/collection.ts`
- [ ] `packages/db/src/schema/feature.ts`
- [ ] `packages/db/src/schema/chunk-type.ts`
- [ ] `packages/db/src/schema/connection-relation.ts`
- [ ] `packages/db/src/schema/saved-graph.ts`
- [ ] `packages/db/src/schema/saved-query.ts`
- [ ] `packages/db/src/schema/activity.ts`
- [ ] `packages/db/src/schema/settings.ts`
- [ ] `packages/db/src/schema/vocabulary.ts`
- [ ] `packages/db/src/schema/behavior-matrix.ts`
- [ ] `packages/db/src/schema/__tests__/behavior-matrix.test.ts` (test fixture references)

- [ ] **Verify type-check after each file:**

```bash
pnpm --filter @fubbik/db check-types
```

Some errors will remain in repository/seed/api files — that's expected at this stage. Only stop if you introduce *new* errors unrelated to the rename.

### Task 3.3: Delete the `codebase.ts` schema file

**Files:**
- Delete: `packages/db/src/schema/codebase.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Remove the export from index**

Delete the line `export * from "./codebase";` in `packages/db/src/schema/index.ts`.

- [ ] **Step 2: Delete the file**

```bash
git rm packages/db/src/schema/codebase.ts
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/schema/index.ts
git commit -m "refactor(db): drop codebase schema (replaced by space)"
```

---

## Phase 4: Repository layer

### Task 4.1: Create `space.ts` repository

**Files:**
- Create: `packages/db/src/repository/space.ts`

- [ ] **Step 1: Write the file**

```ts
import { and, eq, inArray, sql } from "drizzle-orm";
import { Effect } from "effect";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { document } from "../schema/document";
import { plan } from "../schema/plan";
import { requirement } from "../schema/requirement";
import { space, chunkSpace } from "../schema/space";
import { spaceCodeMetadata } from "../schema/space-code-metadata";

export interface CreateSpaceParams {
    id: string;
    name: string;
    kind: string;
    description?: string;
    userId: string;
    code?: { remoteUrl?: string; localPaths?: string[] };
}

export function createSpace(params: CreateSpaceParams) {
    return dbEffect(async () => {
        const [created] = await db
            .insert(space)
            .values({
                id: params.id,
                name: params.name,
                kind: params.kind,
                description: params.description,
                userId: params.userId
            })
            .returning();
        if (params.kind === "code" && params.code) {
            await db.insert(spaceCodeMetadata).values({
                spaceId: created!.id,
                userId: params.userId,
                remoteUrl: params.code.remoteUrl,
                localPaths: params.code.localPaths ?? []
            });
        }
        return created!;
    });
}

export function getSpaceById(spaceId: string, userId?: string) {
    return dbEffect(async () => {
        const conditions = [eq(space.id, spaceId)];
        if (userId) conditions.push(eq(space.userId, userId));
        const [found] = await db.select().from(space).where(and(...conditions));
        return found ?? null;
    });
}

export function getSpaceWithCodeMetadata(spaceId: string, userId?: string) {
    return dbEffect(async () => {
        const conditions = [eq(space.id, spaceId)];
        if (userId) conditions.push(eq(space.userId, userId));
        const [row] = await db
            .select({
                space,
                code: spaceCodeMetadata
            })
            .from(space)
            .leftJoin(spaceCodeMetadata, eq(spaceCodeMetadata.spaceId, space.id))
            .where(and(...conditions));
        return row ?? null;
    });
}

export function listSpaces(userId: string) {
    return dbEffect(() => db.select().from(space).where(eq(space.userId, userId)));
}

export function getCodeSpaceByRemoteUrl(remoteUrl: string, userId: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select({ space })
            .from(space)
            .innerJoin(spaceCodeMetadata, eq(spaceCodeMetadata.spaceId, space.id))
            .where(and(eq(spaceCodeMetadata.remoteUrl, remoteUrl), eq(space.userId, userId)));
        return row?.space ?? null;
    });
}

export function getCodeSpaceByLocalPath(localPath: string, userId: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select({ space })
            .from(space)
            .innerJoin(spaceCodeMetadata, eq(spaceCodeMetadata.spaceId, space.id))
            .where(
                and(
                    sql`${spaceCodeMetadata.localPaths} @> ${JSON.stringify([localPath])}::jsonb`,
                    eq(space.userId, userId)
                )
            );
        return row?.space ?? null;
    });
}

export interface UpdateSpaceParams {
    name?: string;
    description?: string | null;
    code?: { remoteUrl?: string | null; localPaths?: string[] };
}

export function updateSpace(spaceId: string, userId: string, params: UpdateSpaceParams) {
    return dbEffect(async () => {
        const setClause: Record<string, unknown> = {};
        if (params.name !== undefined) setClause.name = params.name;
        if (params.description !== undefined) setClause.description = params.description;

        const [updated] = Object.keys(setClause).length
            ? await db
                  .update(space)
                  .set(setClause)
                  .where(and(eq(space.id, spaceId), eq(space.userId, userId)))
                  .returning()
            : await db.select().from(space).where(and(eq(space.id, spaceId), eq(space.userId, userId)));

        if (params.code) {
            await db
                .insert(spaceCodeMetadata)
                .values({
                    spaceId,
                    userId,
                    remoteUrl: params.code.remoteUrl ?? null,
                    localPaths: params.code.localPaths ?? []
                })
                .onConflictDoUpdate({
                    target: spaceCodeMetadata.spaceId,
                    set: {
                        remoteUrl: params.code.remoteUrl ?? null,
                        localPaths: params.code.localPaths ?? []
                    }
                });
        }
        return updated ?? null;
    });
}

export function resetSpaceData(spaceId: string, userId: string) {
    return dbEffect(async () => {
        const exclusiveChunkIds = await db
            .select({ id: chunkSpace.chunkId })
            .from(chunkSpace)
            .where(eq(chunkSpace.spaceId, spaceId))
            .then(rows => rows.map(r => r.id));

        let chunksDeleted = 0;
        if (exclusiveChunkIds.length > 0) {
            const sharedChunkIds = await db
                .select({ id: chunkSpace.chunkId })
                .from(chunkSpace)
                .where(and(inArray(chunkSpace.chunkId, exclusiveChunkIds), sql`${chunkSpace.spaceId} != ${spaceId}`))
                .then(rows => new Set(rows.map(r => r.id)));
            const toDelete = exclusiveChunkIds.filter(id => !sharedChunkIds.has(id));
            if (toDelete.length > 0) {
                await db.delete(chunk).where(and(inArray(chunk.id, toDelete), eq(chunk.userId, userId)));
                chunksDeleted = toDelete.length;
            }
        }
        await db.delete(chunkSpace).where(eq(chunkSpace.spaceId, spaceId));

        const docsDeleted = await db
            .delete(document)
            .where(and(eq(document.spaceId, spaceId), eq(document.userId, userId)))
            .returning()
            .then(rows => rows.length);
        const plansDeleted = await db
            .delete(plan)
            .where(and(eq(plan.spaceId, spaceId), eq(plan.userId, userId)))
            .returning()
            .then(rows => rows.length);
        const requirementsDeleted = await db
            .delete(requirement)
            .where(and(eq(requirement.spaceId, spaceId), eq(requirement.userId, userId)))
            .returning()
            .then(rows => rows.length);
        return { chunksDeleted, docsDeleted, plansDeleted, requirementsDeleted };
    });
}

export function deleteSpace(spaceId: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(space)
            .where(and(eq(space.id, spaceId), eq(space.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

export function countChunksInSpace(spaceId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ count: sql<number>`count(*)` })
            .from(chunkSpace)
            .where(eq(chunkSpace.spaceId, spaceId));
        return Number(result?.count ?? 0);
    });
}

export function setChunkSpaces(chunkId: string, spaceIds: string[]) {
    return dbEffect(async () => {
        await db.delete(chunkSpace).where(eq(chunkSpace.chunkId, chunkId));
        if (spaceIds.length === 0) return [];
        return db
            .insert(chunkSpace)
            .values(spaceIds.map(spaceId => ({ chunkId, spaceId })))
            .onConflictDoNothing()
            .returning();
    });
}

export function getSpacesForChunk(chunkId: string) {
    return dbEffect(() =>
        db
            .select({ id: space.id, name: space.name, kind: space.kind })
            .from(chunkSpace)
            .innerJoin(space, eq(chunkSpace.spaceId, space.id))
            .where(eq(chunkSpace.chunkId, chunkId))
    );
}

export function getSpacesForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([] as { chunkId: string; spaceId: string; spaceName: string }[]);
    return dbEffect(() =>
        db
            .select({ chunkId: chunkSpace.chunkId, spaceId: space.id, spaceName: space.name })
            .from(chunkSpace)
            .innerJoin(space, eq(chunkSpace.spaceId, space.id))
            .where(inArray(chunkSpace.chunkId, chunkIds))
    );
}
```

- [ ] **Step 2: Type-check**

```bash
pnpm --filter @fubbik/db check-types
```

Expected: passes for the new file (errors elsewhere still expected).

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/space.ts
git commit -m "feat(db): add space repository"
```

### Task 4.2: Re-export from repository index

**Files:**
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Add the export**

Add `export * from "./space";` to `packages/db/src/repository/index.ts`.

- [ ] **Step 2: Commit**

```bash
git add packages/db/src/repository/index.ts
git commit -m "feat(db): re-export space repository"
```

### Task 4.3: Delete `codebase.ts` repository

**Files:**
- Delete: `packages/db/src/repository/codebase.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Remove the export**

Delete the line `export * from "./codebase";` from `packages/db/src/repository/index.ts`.

- [ ] **Step 2: Delete the file**

```bash
git rm packages/db/src/repository/codebase.ts
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/index.ts
git commit -m "refactor(db): drop codebase repository"
```

### Task 4.4: Update workspace repository

**Files:**
- Modify: `packages/db/src/repository/workspace.ts`

- [ ] **Step 1: Rewrite to use `space` + `workspaceSpace`**

Replace `codebase` imports with `space`, `workspaceCodebase` with `workspaceSpace`. Rename function names:
- `getCodebasesForWorkspace` → `getSpacesForWorkspace`
- `addCodebaseToWorkspace` → `addSpaceToWorkspace`
- `removeCodebaseFromWorkspace` → `removeSpaceFromWorkspace`

Select fields: `space.id`, `space.name`, `space.kind` (drop `remote_url` / `local_paths` from the default join; callers that need them can do a follow-up join to `space_code_metadata`).

- [ ] **Step 2: Type-check**

```bash
pnpm --filter @fubbik/db check-types
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/workspace.ts
git commit -m "refactor(db): workspace repo uses space"
```

### Task 4.5: Update all repositories that reference `codebaseId`

For each file below, mechanically rename:
- import `codebase` → `space`
- field `codebaseId` → `spaceId`
- index/column references

**Files (one commit per file):**

- [ ] `packages/db/src/repository/chunk.ts`
- [ ] `packages/db/src/repository/document.ts`
- [ ] `packages/db/src/repository/plan.ts`
- [ ] `packages/db/src/repository/requirement.ts`
- [ ] `packages/db/src/repository/staleness.ts`
- [ ] `packages/db/src/repository/knowledge-health.ts`
- [ ] `packages/db/src/repository/graph.ts`
- [ ] `packages/db/src/repository/feature.ts`
- [ ] `packages/db/src/repository/coverage.ts`
- [ ] `packages/db/src/repository/timeline.ts`
- [ ] `packages/db/src/repository/density.ts`
- [ ] `packages/db/src/repository/saved-graph.ts`
- [ ] `packages/db/src/repository/saved-query.ts`
- [ ] `packages/db/src/repository/settings.ts`
- [ ] `packages/db/src/repository/activity.ts`
- [ ] `packages/db/src/repository/use-case.ts`
- [ ] `packages/db/src/repository/vocabulary.ts`
- [ ] `packages/db/src/repository/vocabulary-catalog.ts`
- [ ] `packages/db/src/repository/chunk-version.ts`
- [ ] `packages/db/src/repository/chunk-groups.ts`
- [ ] `packages/db/src/repository/collection.ts`
- [ ] `packages/db/src/repository/file-ref.ts` (only if it has codebase refs)
- [ ] `packages/db/src/repository/behavior-matrix.ts`
- [ ] `packages/db/src/repository/tag-new.ts`

After each file, run:

```bash
pnpm --filter @fubbik/db check-types
```

You're done when the @fubbik/db type-check is fully green.

- [ ] **Step: Final db verification**

```bash
pnpm --filter @fubbik/db check-types
pnpm --filter @fubbik/db test
```

Expected: both pass. Update any failing tests' fixture data (`codebaseId` → `spaceId`).

---

## Phase 5: API layer

### Task 5.1: Create new `spaces` API module

**Files:**
- Create: `packages/api/src/spaces/service.ts`
- Create: `packages/api/src/spaces/routes.ts`
- Create: `packages/api/src/spaces/normalize-url.ts` (copy from `codebases/normalize-url.ts`)
- Create: `packages/api/src/spaces/service.test.ts` (copy + adapt from `codebases/service.test.ts`)
- Create: `packages/api/src/spaces/normalize-url.test.ts` (copy from `codebases/normalize-url.test.ts`)

- [ ] **Step 1: Copy normalize-url verbatim**

```bash
cp packages/api/src/codebases/normalize-url.ts packages/api/src/spaces/normalize-url.ts
cp packages/api/src/codebases/normalize-url.test.ts packages/api/src/spaces/normalize-url.test.ts
```

- [ ] **Step 2: Write `service.ts`**

```ts
import {
    countChunksInSpace,
    createSpace as createSpaceRepo,
    deleteSpace as deleteSpaceRepo,
    getCodeSpaceByLocalPath,
    getCodeSpaceByRemoteUrl,
    getSpaceById,
    getSpaceWithCodeMetadata,
    listSpaces as listSpacesRepo,
    resetSpaceData as resetSpaceDataRepo,
    updateSpace as updateSpaceRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";
import { normalizeGitUrl } from "./normalize-url";

export function listSpaces(userId: string) {
    return listSpacesRepo(userId);
}

export function getSpace(spaceId: string, userId: string) {
    return getSpaceWithCodeMetadata(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" }))))
    );
}

export interface CreateSpaceBody {
    name: string;
    kind?: string;
    description?: string;
    remoteUrl?: string;
    localPaths?: string[];
}

export function createSpace(userId: string, body: CreateSpaceBody) {
    const id = crypto.randomUUID();
    const kind = body.kind ?? "code";
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : undefined;
    return Effect.suspend(() => {
        if (kind !== "code" || !remoteUrl) return Effect.void;
        return getCodeSpaceByRemoteUrl(remoteUrl, userId).pipe(
            Effect.flatMap(existing =>
                existing
                    ? Effect.fail(new ValidationError({ message: "A space with this remote URL already exists" }))
                    : Effect.void
            )
        );
    }).pipe(
        Effect.flatMap(() =>
            createSpaceRepo({
                id,
                name: body.name,
                kind,
                description: body.description,
                userId,
                code: kind === "code" ? { remoteUrl, localPaths: body.localPaths } : undefined
            })
        )
    );
}

export interface UpdateSpaceBody {
    name?: string;
    description?: string | null;
    remoteUrl?: string | null;
    localPaths?: string[];
}

export function updateSpace(spaceId: string, userId: string, body: UpdateSpaceBody) {
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : body.remoteUrl;
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(found =>
            updateSpaceRepo(spaceId, userId, {
                name: body.name,
                description: body.description,
                code:
                    found.kind === "code"
                        ? { remoteUrl: remoteUrl ?? null, localPaths: body.localPaths ?? [] }
                        : undefined
            })
        )
    );
}

export function resetSpace(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => resetSpaceDataRepo(spaceId, userId))
    );
}

export function deleteSpace(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => resetSpaceDataRepo(spaceId, userId)),
        Effect.flatMap(() => deleteSpaceRepo(spaceId, userId))
    );
}

export function detectSpace(userId: string, query: { remoteUrl?: string; localPath?: string }) {
    const normalizedUrl = query.remoteUrl ? normalizeGitUrl(query.remoteUrl) : undefined;
    if (normalizedUrl) return getCodeSpaceByRemoteUrl(normalizedUrl, userId);
    if (query.localPath) return getCodeSpaceByLocalPath(query.localPath, userId);
    return Effect.succeed(null);
}

export function getSpaceChunkCount(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => countChunksInSpace(spaceId))
    );
}
```

- [ ] **Step 3: Write `routes.ts`**

```ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as spaceService from "./service";

export const spaceRoutes = new Elysia()
    .get(
        "/spaces/detect",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => spaceService.detectSpace(session.user.id, ctx.query)))
            ),
        {
            query: t.Object({
                remoteUrl: t.Optional(t.String()),
                localPath: t.Optional(t.String())
            })
        }
    )
    .get("/spaces", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.listSpaces(session.user.id))))
    )
    .post(
        "/spaces",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => spaceService.createSpace(session.user.id, ctx.body)),
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
                kind: t.Optional(t.String({ maxLength: 50 })),
                description: t.Optional(t.String({ maxLength: 1000 })),
                remoteUrl: t.Optional(t.String({ maxLength: 500 })),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .get("/spaces/:id", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.getSpace(ctx.params.id, session.user.id))))
    )
    .patch(
        "/spaces/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => spaceService.updateSpace(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.Union([t.String({ maxLength: 1000 }), t.Null()])),
                remoteUrl: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                localPaths: t.Optional(t.Array(t.String({ maxLength: 500 }), { maxItems: 10 }))
            })
        }
    )
    .post("/spaces/:id/reset", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => spaceService.resetSpace(ctx.params.id, session.user.id)))
        )
    )
    .delete("/spaces/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => spaceService.deleteSpace(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

- [ ] **Step 4: Adapt the service test**

Open `packages/api/src/codebases/service.test.ts`, copy its structure into `packages/api/src/spaces/service.test.ts`, replacing every `codebase` symbol with `space`. The test should hit the new repo functions.

- [ ] **Step 5: Run tests**

```bash
pnpm --filter @fubbik/api test -- spaces
```

Expected: passes. If fixture data references `codebaseId`, fix those references.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/spaces/
git commit -m "feat(api): add /api/spaces routes + service"
```

### Task 5.2: Mount space routes + alias `/api/codebases`

**Files:**
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Add the import + mount**

In `packages/api/src/index.ts`, alongside the existing route imports:

```ts
import { spaceRoutes } from "./spaces/routes";
```

In the route-mounting block, mount `spaceRoutes` next to where `codebaseRoutes` is mounted.

- [ ] **Step 2: Keep `codebaseRoutes` as alias for one release**

For the VS Code extension's benefit, leave `codebaseRoutes` mounted. Add a `// TODO: remove after VS Code extension v0.X+1 ships` comment above it.

In a follow-up change, the `codebases/routes.ts` file can be rewritten to forward to `spaceService` (delete + recreate as an alias). For now, keep both routes alive so we don't block.

- [ ] **Step 3: Run API tests**

```bash
pnpm --filter @fubbik/api test
```

Expected: both `/spaces` and `/codebases` paths work. Some tests under `codebases/` will fail because the repository no longer exists — those need updating in the next step.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/index.ts
git commit -m "feat(api): mount /api/spaces (keep /api/codebases alias)"
```

### Task 5.3: Rewrite legacy `codebases/` module as alias

**Files:**
- Modify: `packages/api/src/codebases/service.ts`
- Modify: `packages/api/src/codebases/routes.ts`
- Delete: `packages/api/src/codebases/service.test.ts` (already covered by spaces test)
- Delete: `packages/api/src/codebases/normalize-url.ts` (re-export from spaces)
- Delete: `packages/api/src/codebases/normalize-url.test.ts`

- [ ] **Step 1: Replace `service.ts` with re-exports**

```ts
// Legacy alias kept for the VS Code extension. Remove after the extension upgrade.
export * from "../spaces/service";
export {
    listSpaces as listCodebases,
    getSpace as getCodebase,
    createSpace as createCodebase,
    updateSpace as updateCodebase,
    deleteSpace as deleteCodebase,
    resetSpace as resetCodebase,
    detectSpace as detectCodebase
} from "../spaces/service";
```

- [ ] **Step 2: Replace `routes.ts` with re-exports**

```ts
// Legacy alias kept for the VS Code extension. Same handlers as /api/spaces.
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as spaceService from "../spaces/service";

export const codebaseRoutes = new Elysia()
    .get(
        "/codebases/detect",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => spaceService.detectSpace(session.user.id, ctx.query)))
            ),
        { query: t.Object({ remoteUrl: t.Optional(t.String()), localPath: t.Optional(t.String()) }) }
    )
    .get("/codebases", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.listSpaces(session.user.id))))
    )
    .post(
        "/codebases",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => spaceService.createSpace(session.user.id, { ...ctx.body, kind: "code" })),
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
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.getSpace(ctx.params.id, session.user.id))))
    )
    .patch(
        "/codebases/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => spaceService.updateSpace(ctx.params.id, session.user.id, ctx.body))
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
    .post("/codebases/:id/reset", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => spaceService.resetSpace(ctx.params.id, session.user.id))))
    )
    .delete("/codebases/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => spaceService.deleteSpace(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

- [ ] **Step 3: Delete obsolete files**

```bash
git rm packages/api/src/codebases/service.test.ts
git rm packages/api/src/codebases/normalize-url.ts
git rm packages/api/src/codebases/normalize-url.test.ts
```

- [ ] **Step 4: Run tests**

```bash
pnpm --filter @fubbik/api test
```

Expected: passes. Both `/spaces` and `/codebases` work via the same service.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/codebases/
git commit -m "refactor(api): codebases routes become alias for spaces"
```

### Task 5.4: Update services that reference codebase fields

For each file below, mechanically rename `codebaseId` → `spaceId`, `getCodebaseById` → `getSpaceById`, etc. Update imports to point at `@fubbik/db/repository` for the new symbols.

**Files (one commit each):**

- [ ] `packages/api/src/workspaces/service.ts` and `routes.ts` (rename functions + endpoints; `/workspaces/:id/codebases` becomes `/workspaces/:id/spaces`)
- [ ] `packages/api/src/documents/service.ts` and `routes.ts`
- [ ] `packages/api/src/chunks/service.ts`, `chunk-import.ts`, `chunk-mutations.ts`, `chunk-search.ts`, `bulk-service.ts`, `federated-search.ts`, `group-service.ts`, `routes.ts`, `group-routes.ts`
- [ ] `packages/api/src/plans/service.ts`, `routes.ts`, `tasks.ts`
- [ ] `packages/api/src/requirements/service.ts`, `routes.ts`, `batch-service.ts`, `suggest-context-service.ts`
- [ ] `packages/api/src/staleness/routes.ts` (and any related service)
- [ ] `packages/api/src/context/routes.ts`, `claude-md.ts`, `resolvers.ts`, `snapshot-routes.ts`, `snapshot-service.ts`
- [ ] `packages/api/src/context-export/service.ts`, `routes.ts`
- [ ] `packages/api/src/context-for-file/service.ts`, `routes.ts`, `service.test.ts`
- [ ] `packages/api/src/graph/service.ts`, `routes.ts`
- [ ] `packages/api/src/coverage/service.ts`, `routes.ts`
- [ ] `packages/api/src/density/service.ts`, `routes.ts`
- [ ] `packages/api/src/diagram/service.ts`, `routes.ts`
- [ ] `packages/api/src/features/service.ts`, `routes.ts`, `service.test.ts`, `routes.test.ts`
- [ ] `packages/api/src/matrices/service.ts`, `routes.ts`, `service.test.ts`
- [ ] `packages/api/src/knowledge-health/service.ts`, `routes.ts`
- [ ] `packages/api/src/vocabulary/service.ts`, `routes.ts`
- [ ] `packages/api/src/vocabularies/service.ts`, `routes.ts`
- [ ] `packages/api/src/saved-graphs/service.ts`, `routes.ts`
- [ ] `packages/api/src/search/service.ts`, `routes.ts`, `types.ts`, `query-types.ts`
- [ ] `packages/api/src/settings/service.ts`, `routes.ts`
- [ ] `packages/api/src/collections/service.ts`, `routes.ts`
- [ ] `packages/api/src/use-cases/service.ts`, `routes.ts`
- [ ] `packages/api/src/activity/service.ts`, `routes.ts`
- [ ] `packages/api/src/timeline/service.ts`, `routes.ts`
- [ ] `packages/api/src/ai/structure-requirement.ts`, `routes.ts`
- [ ] `packages/api/src/generate-instructions/service.ts`, `routes.ts`
- [ ] `packages/api/src/tasks/routes.ts`

**Public query parameters:** in route files, rename query params from `codebaseId` → `spaceId`, `workspaceId` (unchanged). For each route that accepts `codebaseId` from the client, keep accepting `codebaseId` as a deprecated alias that maps to `spaceId` internally — or do a hard cutover and update all callers. **Recommended: hard cutover. CLI and frontend are updated in subsequent phases.** This means: rename the query param and rename the field in the request schema.

After each file:

```bash
pnpm --filter @fubbik/api check-types
pnpm --filter @fubbik/api test
```

- [ ] **Step: Full API verification**

```bash
pnpm --filter @fubbik/api check-types
pnpm --filter @fubbik/api test
pnpm --filter @fubbik/api build
```

Expected: all pass.

---

## Phase 6: CLI

### Task 6.1: Create new `space.ts` command file

**Files:**
- Create: `apps/cli/src/commands/space.ts`

- [ ] **Step 1: Write the file**

Copy `apps/cli/src/commands/codebase.ts` to `apps/cli/src/commands/space.ts`. Then transform:
- `Command("codebase")` → `Command("space")`
- `/api/codebases` → `/api/spaces`
- `addCodebase` → `addSpace`, etc.
- The `add` subcommand body should include `kind: "code"` in the POST body to match the new schema default.

- [ ] **Step 2: Verify shape**

```bash
pnpm --filter @fubbik/cli build
```

Expected: builds.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/space.ts
git commit -m "feat(cli): add `fubbik space` command"
```

### Task 6.2: Rewrite legacy `codebase.ts` CLI command as alias

**Files:**
- Modify: `apps/cli/src/commands/codebase.ts`

- [ ] **Step 1: Replace with a deprecation-warning alias**

```ts
import { Command } from "commander";

import { spaceCommand } from "./space";

// Legacy alias — `fubbik codebase ...` forwards to `fubbik space ...`.
export const codebaseCommand = new Command("codebase")
    .description("[deprecated] alias of `fubbik space`")
    .hook("preAction", () => {
        console.warn("[fubbik] `codebase` is deprecated; use `space` instead.");
    });

for (const sub of spaceCommand.commands) {
    codebaseCommand.addCommand(sub);
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/cli/src/commands/codebase.ts
git commit -m "refactor(cli): codebase command becomes deprecated alias of space"
```

### Task 6.3: Mount the new command

**Files:**
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Wire it up**

Add the import and `.addCommand(spaceCommand)` next to the existing `codebaseCommand` registration.

- [ ] **Step 2: Smoke test**

```bash
pnpm --filter @fubbik/cli build
node apps/cli/dist/index.js space --help
node apps/cli/dist/index.js codebase --help
```

Expected: both work; `codebase` prints a deprecation warning.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/index.ts
git commit -m "feat(cli): register space command"
```

### Task 6.4: Update CLI commands that pass `--codebase`

For each file below, rename the option flag `--codebase` → `--space` (keep `--codebase` accepted as a hidden alias for one release), and rename internal variables.

**Files (one commit each):**

- [ ] `apps/cli/src/commands/add.ts`
- [ ] `apps/cli/src/commands/list.ts`
- [ ] `apps/cli/src/commands/search.ts`
- [ ] `apps/cli/src/commands/quick.ts`
- [ ] `apps/cli/src/commands/update.ts`
- [ ] `apps/cli/src/commands/remove.ts`
- [ ] `apps/cli/src/commands/get.ts`
- [ ] `apps/cli/src/commands/cat.ts`
- [ ] `apps/cli/src/commands/context.ts`
- [ ] `apps/cli/src/commands/context-for.ts`
- [ ] `apps/cli/src/commands/context-dir.ts`
- [ ] `apps/cli/src/commands/context-about.ts`
- [ ] `apps/cli/src/commands/context-for-plan.ts`
- [ ] `apps/cli/src/commands/context-for-diff.ts`
- [ ] `apps/cli/src/commands/context-group.ts`
- [ ] `apps/cli/src/commands/context-snapshot.ts`
- [ ] `apps/cli/src/commands/cleanup.ts`
- [ ] `apps/cli/src/commands/import.ts`
- [ ] `apps/cli/src/commands/import-requirements.ts`
- [ ] `apps/cli/src/commands/export-site.ts`
- [ ] `apps/cli/src/commands/kb-diff.ts`
- [ ] `apps/cli/src/commands/sync-claude-md.ts`
- [ ] `apps/cli/src/commands/sync.ts`
- [ ] `apps/cli/src/commands/setup.ts`
- [ ] `apps/cli/src/commands/lint.ts`
- [ ] `apps/cli/src/commands/recap.ts`
- [ ] `apps/cli/src/commands/why.ts`
- [ ] `apps/cli/src/commands/task.ts`
- [ ] `apps/cli/src/commands/plan.ts`
- [ ] `apps/cli/src/commands/requirements.ts`
- [ ] `apps/cli/src/commands/req.ts`
- [ ] `apps/cli/src/commands/matrix.ts`
- [ ] `apps/cli/src/commands/gaps.ts`
- [ ] `apps/cli/src/commands/suggest.ts`
- [ ] `apps/cli/src/commands/status.ts`
- [ ] `apps/cli/src/commands/updates.ts`
- [ ] `apps/cli/src/commands/tag-normalize.ts`
- [ ] `apps/cli/src/commands/generate.ts`
- [ ] `apps/cli/src/commands/docs.ts`
- [ ] `apps/cli/src/commands/doctor.ts`
- [ ] `apps/cli/src/commands/bulk-add.ts`
- [ ] `apps/cli/src/commands/mcp-tools.ts`
- [ ] `apps/cli/src/lib/detect-codebase.ts` (rename file to `detect-space.ts`, update exports, update imports across files above)
- [ ] `apps/cli/src/lib/config.ts`
- [ ] `apps/cli/src/lib/completions.ts`
- [ ] `apps/cli/src/lib/setup/*.ts`

- [ ] **Step: Final CLI verification**

```bash
pnpm --filter @fubbik/cli check-types
pnpm --filter @fubbik/cli test
pnpm --filter @fubbik/cli build
```

Expected: all pass.

---

## Phase 7: Frontend

### Task 7.1: Move frontend feature folder

**Files:**
- Create: `apps/web/src/features/spaces/active-space-provider.tsx`
- Create: `apps/web/src/features/spaces/space-switcher.tsx`
- Create: `apps/web/src/features/spaces/use-active-space.ts`
- Delete: `apps/web/src/features/codebases/`

- [ ] **Step 1: Copy + rename files**

```bash
mkdir -p apps/web/src/features/spaces
git mv apps/web/src/features/codebases/active-codebase-provider.tsx apps/web/src/features/spaces/active-space-provider.tsx
git mv apps/web/src/features/codebases/codebase-switcher.tsx apps/web/src/features/spaces/space-switcher.tsx
git mv apps/web/src/features/codebases/use-active-codebase.ts apps/web/src/features/spaces/use-active-space.ts
```

- [ ] **Step 2: Rename symbols inside each file**

In each moved file, replace:
- `ActiveCodebaseProvider` → `ActiveSpaceProvider`
- `useActiveCodebase` → `useActiveSpace`
- `codebase` → `space` (variable / state names)
- Eden client paths `/api/codebases` → `/api/spaces`

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/spaces/ apps/web/src/features/codebases/
git commit -m "refactor(web): move codebases feature folder to spaces"
```

### Task 7.2: Update all frontend imports + Eden calls

The frontend has many imports of `useActiveCodebase`, `ActiveCodebaseProvider`, and direct Eden calls like `eden.api.codebases.get`. Mechanically rename each.

- [ ] **Step 1: Find all callsites**

```bash
grep -rn "useActiveCodebase\|ActiveCodebaseProvider\|features/codebases\|codebases\." apps/web/src/ | wc -l
```

- [ ] **Step 2: Rename callsites file by file**

Touchpoints (likely): the nav switcher mount, command palette, chunks pages, graph pages, plans pages, requirements pages, documents pages, dashboard widgets, settings page. Use the grep output as your worklist.

For Eden: `eden.api.codebases.get` → `eden.api.spaces.get`, `?codebaseId=` query params → `?spaceId=`, request body keys `codebaseId` → `spaceId`, etc.

- [ ] **Step 3: Regenerate route tree if needed**

```bash
pnpm --filter @fubbik/web dev # let TanStack Start regenerate routeTree.gen.ts
# or
pnpm --filter @fubbik/web build
```

- [ ] **Step 4: Type-check + smoke test**

```bash
pnpm --filter @fubbik/web check-types
pnpm --filter @fubbik/web build
pnpm dev
```

Visit `http://localhost:3001`, switch the space, create a chunk, run a search. Confirm no console errors.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/
git commit -m "refactor(web): codebase → space across all features"
```

### Task 7.3: Rename web routes

**Files:**
- Rename: `apps/web/src/routes/codebases*` → `apps/web/src/routes/spaces*` (if any)

- [ ] **Step 1: Move route files**

```bash
ls apps/web/src/routes/codebases* 2>/dev/null
# rename whatever you find with git mv
```

- [ ] **Step 2: Update route content + regenerate**

Inside each renamed route file, update `useActiveCodebase` references, page titles, and any internal links from `/codebases` → `/spaces`.

```bash
pnpm --filter @fubbik/web build  # regenerates routeTree.gen.ts
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/ apps/web/src/routeTree.gen.ts
git commit -m "refactor(web): codebases routes become spaces"
```

---

## Phase 8: VS Code extension

### Task 8.1: Update extension to use `/api/spaces`

**Files:**
- Modify: `apps/vscode/src/api.ts`
- Modify: `apps/vscode/src/detect-codebase.ts` (rename file to `detect-space.ts`)
- Modify: `apps/vscode/src/extension.ts`
- Modify: `apps/vscode/src/create-chunk.ts`
- Modify: `apps/vscode/src/sidebar-provider.ts`
- Modify: `apps/vscode/src/status-bar.ts`
- Modify: `apps/vscode/package.json` (rename config keys?)

- [ ] **Step 1: Replace endpoints**

In `api.ts`, change `/api/codebases` → `/api/spaces`. Type names like `Codebase` → `Space`. Function names like `detectCodebase` → `detectSpace`.

- [ ] **Step 2: Rename `detect-codebase.ts`**

```bash
git mv apps/vscode/src/detect-codebase.ts apps/vscode/src/detect-space.ts
```

Update imports in the other vscode files.

- [ ] **Step 3: Update settings keys (optional, only if you want to rename them)**

If you want `fubbik.activeCodebaseId` → `fubbik.activeSpaceId` in VS Code user settings, update `package.json`'s `contributes.configuration` section and read the new key in code (with a fallback to the old key for one release).

- [ ] **Step 4: Build the extension**

```bash
cd apps/vscode && node esbuild.mjs
```

Expected: builds.

- [ ] **Step 5: Commit**

```bash
git add apps/vscode/
git commit -m "refactor(vscode): use /api/spaces"
```

---

## Phase 9: MCP server

### Task 9.1: Update MCP tool definitions

**Files:**
- Modify: `packages/mcp/src/tools.ts`
- Modify: `packages/mcp/src/context-tools.ts`
- Modify: `packages/mcp/src/plan-tools.ts`
- Modify: `packages/mcp/src/requirement-tools.ts`
- Modify: `packages/mcp/src/suggestion-tools.ts`
- Modify: `packages/mcp/src/matrix-tools.ts`

- [ ] **Step 1: Mechanical rename**

For each file, replace `codebaseId` → `spaceId` in tool parameter schemas; replace `/api/codebases` → `/api/spaces` in `apiFetch` calls. Tool descriptions that say "codebase" can stay as-is or be reworded to "space" — keeping the user-facing description in sync with the new vocabulary is recommended.

- [ ] **Step 2: Type-check + commit**

```bash
pnpm --filter @fubbik/mcp check-types
```

```bash
git add packages/mcp/
git commit -m "refactor(mcp): codebase → space"
```

---

## Phase 10: Seed data

### Task 10.1: Update seed modules

**Files:**
- Modify: `packages/db/src/seed.ts`
- Modify: `packages/db/src/seed/modules/codebases.ts` (rename file to `spaces.ts`)
- Modify: `packages/db/src/seed/modules/workspaces.ts`
- Modify: `packages/db/src/seed/modules/chunks.ts`
- Modify: `packages/db/src/seed/modules/plans.ts`
- Modify: `packages/db/src/seed/modules/requirements.ts`
- Modify: `packages/db/src/seed/modules/use-cases.ts`
- Modify: `packages/db/src/seed/modules/documents.ts`
- Modify: `packages/db/src/seed/modules/matrices.ts`
- Modify: `packages/db/src/seed/modules/self-documenting.ts`
- Modify: `packages/db/src/seed/modules/collections.ts`
- Modify: `packages/db/src/seed/modules/vocabulary.ts`
- Modify: `packages/db/src/seed/modules/tags.ts`
- Modify: `packages/db/src/seed/context.ts`
- Modify: `packages/db/src/seed/fixtures.ts`
- Modify: `packages/db/src/seed/index.ts`
- Modify: `packages/db/src/seed/factories.ts`
- Modify: `packages/db/src/seed/verify.ts`

- [ ] **Step 1: Rename the module file**

```bash
git mv packages/db/src/seed/modules/codebases.ts packages/db/src/seed/modules/spaces.ts
```

- [ ] **Step 2: Rewrite seed inserts**

In every module, replace `db.insert(codebase)` with `db.insert(space)` (passing `kind: "code"`) plus a follow-up `db.insert(spaceCodeMetadata)` for the git fields. Replace `chunk_codebase` insertions with `chunk_space`, `workspace_codebase` with `workspace_space`.

- [ ] **Step 3: Smoke test the seed**

```bash
pnpm db:down && pnpm --filter @fubbik/db db:start
sleep 3
pnpm --filter @fubbik/db db:push
pnpm seed
```

Expected: no errors, sample data loads. Visit the dashboard and verify the seed-loaded chunks/spaces appear.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/seed/ packages/db/src/seed.ts
git commit -m "refactor(db): seed modules use space"
```

### Task 10.2: Update DB tests

**Files:**
- Rename: `packages/db/src/__tests__/codebase.test.ts` → `space.test.ts`
- Modify: `packages/db/src/__tests__/requirement.test.ts`
- Modify: `packages/db/src/__tests__/vocabulary.test.ts`
- Modify: `packages/db/src/__tests__/feature.test.ts`

- [ ] **Step 1: Rename + rewrite the codebase test**

```bash
git mv packages/db/src/__tests__/codebase.test.ts packages/db/src/__tests__/space.test.ts
```

Inside, replace all `codebase` references with `space`. Tests should now exercise `createSpace`, `getSpaceById`, etc.

- [ ] **Step 2: Fix the other test fixtures**

In each test, rename `codebaseId` → `spaceId`, swap `createCodebase` for `createSpace`.

- [ ] **Step 3: Run tests**

```bash
pnpm --filter @fubbik/db test
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/__tests__/
git commit -m "test(db): rename codebase test → space, update fixtures"
```

---

## Phase 11: Final verification

### Task 11.1: Full CI

**Files:**
- None (verification only)

- [ ] **Step 1: Run the full CI**

```bash
pnpm ci
```

Expected: all of type-check, lint, test, build, format-check, and sherif pass.

- [ ] **Step 2: Manual smoke test**

Start the app:

```bash
pnpm dev
```

Walk through:
1. Sign in.
2. Visit `/spaces` (the renamed page) — your spaces should be listed, all marked as `code`.
3. Open the space switcher in the nav — it should show the same spaces.
4. Visit `/chunks` with a space selected — chunks should load.
5. Visit `/graph` — nodes/edges render, no console errors.
6. Visit `/plans` with a space — plans load.
7. Visit `/requirements` — requirements load.
8. From the CLI, run `fubbik space list` — same output as the legacy `fubbik codebase list` (which should print a deprecation warning).
9. Hit `/api/spaces` and `/api/codebases` with curl — both return the same data.

- [ ] **Step 3: Commit any final fix-ups**

If anything broke during the smoke test, fix it and commit.

### Task 11.2: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the "Codebases & Workspaces" section**

Rename the section to "Spaces & Workspaces". Replace references to `codebase` with `space`, mention the `kind` discriminator, and note `space_code_metadata` as the side-table for git fields.

- [ ] **Step 2: Update the API endpoints listing**

Add `/api/spaces` endpoints next to (or replacing) `/api/codebases`. Mention the legacy alias.

- [ ] **Step 3: Update the CLI commands listing**

`fubbik codebase ...` → `fubbik space ...` (note deprecation alias).

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for space rename"
```

### Task 11.3: Cleanup tracking

**Files:**
- None (just create reminders)

- [ ] **Step 1: Open a follow-up issue or note**

Capture the deprecation cleanup items so they aren't forgotten:
- Remove `/api/codebases` route alias once the VS Code extension is on the new endpoints.
- Remove `fubbik codebase` deprecation alias.
- Drop the `userId` denormalisation on `space_code_metadata` if a trigger/check is preferred.

---

## Self-review checklist

After implementing, verify:

- [ ] `pnpm ci` is fully green.
- [ ] `pnpm seed` succeeds from a fresh DB.
- [ ] No file in `packages/db/src/schema/` imports `codebase`.
- [ ] No file in `packages/db/src/repository/` imports from `./codebase`.
- [ ] `grep -rn "codebaseId" packages/db apps/web apps/cli packages/api | grep -v "node_modules"` returns either nothing or only deprecated-alias paths.
- [ ] The VS Code extension still works against both `/api/spaces` and `/api/codebases`.
- [ ] CLI `fubbik space list` and `fubbik codebase list` return the same rows.
- [ ] Web app: visit `/spaces`, switch a space, create a chunk.
- [ ] Migration `0004_codebase_to_space.sql` is idempotent (re-running it twice on a fresh DB doesn't error).
