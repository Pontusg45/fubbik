# Codebase Groups (Workspaces) Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group related codebases into workspaces so users can view knowledge across sibling projects.

**Architecture:** New `workspace` table with `workspace_codebase` join table. The codebase switcher gains a "workspace" view that selects all codebases in a group. Chunk queries accept `workspaceId` which expands to multiple `codebaseId` values.

**Tech Stack:** Drizzle ORM, Effect, Elysia, React, Eden treaty

---

## File Structure

### New files:
- `packages/db/src/schema/workspace.ts` — workspace + workspace_codebase tables
- `packages/db/src/repository/workspace.ts` — CRUD
- `packages/api/src/workspaces/service.ts` — Business logic
- `packages/api/src/workspaces/routes.ts` — API endpoints

### Files to modify:
- `packages/db/src/schema/index.ts` — Export workspace schema
- `packages/db/src/repository/index.ts` — Export workspace repo
- `packages/db/src/repository/chunk.ts` — Accept `workspaceId` in listChunks
- `packages/api/src/chunks/routes.ts` — Add `workspaceId` query param
- `packages/api/src/chunks/service.ts` — Resolve workspace to codebase IDs
- `packages/api/src/index.ts` — Mount workspace routes
- `apps/web/src/features/codebases/codebase-switcher.tsx` — Add workspace grouping

---

## Task 1: Workspace Schema

**Files:**
- Create: `packages/db/src/schema/workspace.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create workspace schema**

Follow the `codebase.ts` pattern:

```ts
// packages/db/src/schema/workspace.ts
import { pgTable, text, timestamp, index, primaryKey } from "drizzle-orm/pg-core";
import { relations } from "drizzle-orm";
import { user } from "./auth";
import { codebase } from "./codebase";

export const workspace = pgTable(
    "workspace",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        userId: text("user_id").notNull().references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().$onUpdate(() => new Date()).notNull(),
    },
    (table) => [
        index("workspace_userId_idx").on(table.userId),
    ]
);

export const workspaceCodebase = pgTable(
    "workspace_codebase",
    {
        workspaceId: text("workspace_id").notNull().references(() => workspace.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").notNull().references(() => codebase.id, { onDelete: "cascade" }),
    },
    (table) => [
        primaryKey({ columns: [table.workspaceId, table.codebaseId] }),
    ]
);

export const workspaceRelations = relations(workspace, ({ one, many }) => ({
    user: one(user, { fields: [workspace.userId], references: [user.id] }),
    codebases: many(workspaceCodebase),
}));

export const workspaceCodebaseRelations = relations(workspaceCodebase, ({ one }) => ({
    workspace: one(workspace, { fields: [workspaceCodebase.workspaceId], references: [workspace.id] }),
    codebase: one(codebase, { fields: [workspaceCodebase.codebaseId], references: [codebase.id] }),
}));
```

- [ ] **Step 2: Export and push schema**

Add `export * from "./workspace"` to schema index. Run `pnpm db:push`.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add workspace and workspace_codebase database schema"
```

---

## Task 2: Workspace Repository + Service + Routes

**Files:**
- Create: `packages/db/src/repository/workspace.ts`
- Create: `packages/api/src/workspaces/service.ts`
- Create: `packages/api/src/workspaces/routes.ts`
- Modify: `packages/db/src/repository/index.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create workspace repository**

Follow the `use-case.ts` repo pattern. Functions: `createWorkspace`, `getWorkspaceById`, `listWorkspaces`, `updateWorkspace`, `deleteWorkspace`, `getCodebasesForWorkspace`, `addCodebaseToWorkspace`, `removeCodebaseFromWorkspace`.

- [ ] **Step 2: Create workspace service**

Validation: workspace names unique per user, codebase must belong to user before adding to workspace.

- [ ] **Step 3: Create workspace routes**

```
GET    /workspaces              — list workspaces
POST   /workspaces              — create workspace
GET    /workspaces/:id          — detail with codebases
PATCH  /workspaces/:id          — update
DELETE /workspaces/:id          — delete
POST   /workspaces/:id/codebases — add codebase to workspace
DELETE /workspaces/:id/codebases/:codebaseId — remove
```

- [ ] **Step 4: Mount and export**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: add workspace API (CRUD + codebase membership)"
```

---

## Task 3: Workspace-Aware Chunk Queries

**Files:**
- Modify: `packages/db/src/repository/chunk.ts`
- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Add workspaceId to listChunks**

In `packages/db/src/repository/chunk.ts`, add `workspaceId?: string` to `ListChunksParams`. When set, expand to all codebase IDs in the workspace:

```ts
if (params.workspaceId) {
    const inWorkspace = db
        .select({ codebaseId: workspaceCodebase.codebaseId })
        .from(workspaceCodebase)
        .where(eq(workspaceCodebase.workspaceId, params.workspaceId));
    const inCodebases = db
        .select({ chunkId: chunkCodebase.chunkId })
        .from(chunkCodebase)
        .where(sql`${chunkCodebase.codebaseId} IN (${inWorkspace})`);
    const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
    conditions.push(
        or(sql`${chunk.id} IN (${inCodebases})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!
    );
}
```

- [ ] **Step 2: Add workspaceId to API**

Add `workspaceId: t.Optional(t.String())` to `GET /chunks` query params. Pass through service to repo.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: workspace-aware chunk queries"
```

---

## Task 4: Codebase Switcher Workspace View

**Files:**
- Modify: `apps/web/src/features/codebases/codebase-switcher.tsx`

- [ ] **Step 1: Fetch workspaces alongside codebases**

Add a query for workspaces. Show them as groups in the switcher dropdown:

```
── Workspaces ──
  Platform (3 codebases)
  Mobile (2 codebases)
── Codebases ──
  web-app
  api-server
  mobile-ios
```

- [ ] **Step 2: When a workspace is selected, set workspaceId instead of codebaseId**

Update `useActiveCodebase` to support both `?codebase=<id>` and `?workspace=<id>` search params.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): workspace grouping in codebase switcher"
```
