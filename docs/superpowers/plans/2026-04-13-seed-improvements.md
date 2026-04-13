# Seed Script Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bugs in the seed script (global deletes, bad column inserts, stale content), add seed data for 15 empty tables, and add missing variety in chunk types, plan statuses, and task statuses.

**Architecture:** Single file modification (`packages/db/src/seed.ts`). Four tasks: fix bugs, update stale content, add missing seed data, verify.

**Tech Stack:** Drizzle ORM, PostgreSQL, Bun

---

## File Structure

### Modified

| Path | Responsibility |
|---|---|
| `packages/db/src/seed.ts` | All changes — bug fixes, new imports, new seed sections |

---

### Task 1: Fix Bugs

**Files:**
- Modify: `packages/db/src/seed.ts`

**Context:** Three bugs need fixing: (1) global deletes without user scoping wipe other users' data, (2) document chunk connections insert a `userId` column that doesn't exist on `chunkConnection`, (3) hardcoded `/Users/pontus/projects/fubbik` path.

- [ ] **Step 1: Read the seed file to identify exact line numbers for each bug**

Read the full file. Locate:
- The delete sweep section (around lines 40-62) — find `db.delete(chunkConnection)` and `db.delete(chunkTag)` which lack `where` clauses
- The document chunk connection insert (around line 1957) — find `{ ...conn, userId: DEV_USER_ID }`
- The codebase seeding (around line 1489) — find `localPaths: ["/Users/pontus/projects/fubbik"]`

- [ ] **Step 2: Fix the global `chunkConnection` delete**

The `chunkConnection` table has no `userId` column, so we can't scope the delete by user. However, since all connections in a dev seed are between seeded chunks (which ARE user-scoped), the cascade from `db.delete(chunk).where(eq(chunk.userId, DEV_USER_ID))` will handle connection cleanup via foreign key cascade.

**Fix:** Remove the explicit `db.delete(chunkConnection)` line from the delete sweep entirely. The FK cascade from chunk deletion handles it.

Find the line:
```typescript
await db.delete(chunkConnection);
```

Delete it.

- [ ] **Step 3: Fix the `chunkTag` global delete**

Same issue — `chunkTag` has no `userId`. But `chunkTag` has an FK to `chunk.id` and `tag.id`. Deleting chunks (user-scoped) and tags (user-scoped) will cascade-delete `chunkTag` rows.

**Fix:** Remove the explicit `db.delete(chunkTag)` line. FK cascades from chunk + tag deletion handle it.

Find the line:
```typescript
await db.delete(chunkTag);
```

Delete it.

Also check: `db.delete(chunkAppliesTo)`, `db.delete(chunkFileRef)`, `db.delete(chunkCodebase)`, `db.delete(requirementChunk)`, `db.delete(chunkTag)` — these all reference `chunk.id` and should cascade. Remove any that lack user scoping and rely on FK cascades. Keep any that DO have `where(eq(..., userId))` guards.

**Important:** Read each table's schema to confirm ON DELETE CASCADE exists on the FK before removing the explicit delete. If a table uses ON DELETE SET NULL or no cascade, keep the explicit delete.

- [ ] **Step 4: Fix the document chunk connection `userId` bug**

Find the section that inserts document chunk connections. It looks like:

```typescript
await db.insert(chunkConnection).values({ ...conn, userId: DEV_USER_ID }).catch(...)
```

Remove `userId: DEV_USER_ID` from the values object:

```typescript
await db.insert(chunkConnection).values(conn).catch(...)
```

Or if `conn` is a spread object, just remove the `userId` key.

- [ ] **Step 5: Fix the hardcoded local path**

Find:
```typescript
localPaths: ["/Users/pontus/projects/fubbik"]
```

Replace with:
```typescript
localPaths: [process.cwd()]
```

This uses the working directory at seed time, which is correct for local development.

- [ ] **Step 6: Run the seed to verify bug fixes**

```bash
pnpm seed 2>&1 | tail -20
```

Expected: no errors. If any FK constraint violations appear from the removed deletes, add them back with a comment explaining why.

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/seed.ts
git commit -m "fix(db): fix seed bugs — scoped deletes, chunkConnection userId, hardcoded path

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Update Stale Chunk Content

**Files:**
- Modify: `packages/db/src/seed.ts`

**Context:** Several chunks describe features that have been removed from the codebase. The CLAUDE.md is the source of truth — read it to confirm what's current.

- [ ] **Step 1: Read CLAUDE.md to confirm current tech stack**

Key facts:
- AI: Ollama only (vercel-ai SDK removed, no OpenAI)
- Embeddings: Ollama nomic-embed-text
- Validation: Elysia `t` schema (NOT Arktype)
- Package manager: pnpm (NOT bun install)
- Runtime: bun
- UI: shadcn-ui on base-ui (render prop, NOT asChild)

- [ ] **Step 2: Fix the architecture overview chunk**

Find the chunk with id `ids.arch` (around line 141). Its content mentions "AI: OpenAI (gpt-4o-mini), Ollama". 

Update to:
```
AI: Ollama (local LLM for enrichment + embeddings via nomic-embed-text)
```

Remove any mention of OpenAI, gpt-4o-mini, or vercel-ai.

- [ ] **Step 3: Fix the AI integration chunk**

Find the chunk with id `ids.apiAI` (around line 451). It references `@ai-sdk/openai` and `gpt-4o-mini`.

Rewrite the content to describe the current Ollama-only setup:
- Enrichment via Ollama (llama3.2): generates summary, aliases, notAbout
- Embeddings via Ollama (nomic-embed-text): 768-dim vectors for semantic search
- No external AI APIs — fully local

- [ ] **Step 4: Fix the env validation chunk**

Find the chunk with id `ids.env` (around line 1048). It says "uses `@t3-oss/env-core` with Arktype schemas".

Update to reference Elysia `t` schema validation (Arktype was removed).

- [ ] **Step 5: Fix the Docker chunk**

Find the chunk with id `ids.docker` (around line 1085). It references `bun install` and `bun dev`.

Update to `pnpm install` and `pnpm dev`.

- [ ] **Step 6: Fix the CLI store chunk**

Find the chunk with id `ids.cliStore` (around line 968). Check if it references `.fubbik/store.json` or an outdated path. Update to match the current CLI config location (`fubbik.config.json`).

- [ ] **Step 7: Fix the schema chunk**

Find the chunk with id `ids.schemaChunks` (around line 186). It mentions a `guide` chunk type. The valid types are: note, document, reference, schema, checklist.

Replace `guide` with `checklist` (or remove the mention).

- [ ] **Step 8: Scan for any other stale references**

```bash
grep -n "openai\|gpt-4\|arktype\|vercel-ai\|bun install\|bun dev" packages/db/src/seed.ts
```

Fix any remaining hits.

- [ ] **Step 9: Commit**

```bash
git add packages/db/src/seed.ts
git commit -m "fix(db): update stale chunk content in seed (OpenAI→Ollama, Arktype→Elysia t)

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Add Missing Seed Data + Variety

**Files:**
- Modify: `packages/db/src/seed.ts`

**Context:** 15 tables have no seed data. Also missing: note/checklist chunk types, draft/ready/archived plan statuses, blocked/skipped task statuses, plan task chunks, plan task dependencies.

- [ ] **Step 1: Add new imports at the top of the seed file**

Add these imports alongside the existing ones:

```typescript
import { chunkVersion } from "./schema/chunk-version";
import { chunkStaleness } from "./schema/staleness";
import { chunkProposal } from "./schema/chunk-proposal";
import { contextSnapshot } from "./schema/context-snapshot";
import { favorite } from "./schema/favorite";
import { activity } from "./schema/activity";
import { notification } from "./schema/notification";
import { chunkComment } from "./schema/comment";
import { savedGraph } from "./schema/saved-graph";
import { savedQuery } from "./schema/saved-query";
import { planTaskChunk } from "./schema/plan";
import { planTaskDependency } from "./schema/plan";
```

**Verify each import path** by reading the schema index file (`packages/db/src/schema/index.ts`) to confirm the exact export names. The table names above are guesses — adapt to the actual exported names. For example, `activity` might be `activityLog`, `chunkComment` might be `comment`, etc.

- [ ] **Step 2: Add deletes for the new tables in the cleanup section**

Add before the existing delete sweep (at the very top, before any plan/chunk deletes). These must come first because some FK-reference other tables:

```typescript
// New tables cleanup
await db.delete(contextSnapshot).where(eq(contextSnapshot.userId, DEV_USER_ID));
await db.delete(chunkProposal); // FK cascades from chunk delete, but clear explicitly for clean slate
await db.delete(chunkStaleness); // FK cascades from chunk delete
await db.delete(chunkVersion); // FK cascades from chunk delete
await db.delete(favorite).where(eq(favorite.userId, DEV_USER_ID));
await db.delete(notification).where(eq(notification.userId, DEV_USER_ID));
await db.delete(activity); // may not have userId — check schema; if not, delete all
await db.delete(chunkComment); // FK cascades from chunk delete
await db.delete(savedGraph).where(eq(savedGraph.userId, DEV_USER_ID));
await db.delete(savedQuery).where(eq(savedQuery.userId, DEV_USER_ID));
```

**Read each schema** to confirm which tables have `userId` columns. Use `where(eq(..., DEV_USER_ID))` where available; omit the where clause only for tables that FK-cascade from user-scoped parent tables.

- [ ] **Step 3: Add note + checklist type chunks**

After the existing chunk inserts, add 2-3 chunks of types not yet represented:

```typescript
// Note-type chunks
const noteChunk1Id = crypto.randomUUID();
await db.insert(chunk).values({
    id: noteChunk1Id,
    title: "Authentication convention",
    content: "All API routes require a valid session token via Better Auth. The session is resolved in the Elysia middleware and injected into the request context via `requireSession(ctx)`. Routes that need public access (health check, login) skip this middleware.",
    type: "note",
    userId: DEV_USER_ID,
    origin: "human",
    reviewStatus: "approved",
});

const noteChunk2Id = crypto.randomUUID();
await db.insert(chunk).values({
    id: noteChunk2Id,
    title: "Error handling convention",
    content: "All service functions return Effect<T, TaggedError>. The global Elysia error handler extracts the _tag from FiberFailure and maps to HTTP status codes: ValidationError→400, AuthError→401, NotFoundError→404, DatabaseError→500.",
    type: "note",
    userId: DEV_USER_ID,
    origin: "human",
    reviewStatus: "approved",
});

const checklistChunkId = crypto.randomUUID();
await db.insert(chunk).values({
    id: checklistChunkId,
    title: "New feature checklist",
    content: "- [ ] Add schema in packages/db/src/schema/\n- [ ] Add repository in packages/db/src/repository/\n- [ ] Add service in packages/api/src/\n- [ ] Add routes in packages/api/src/\n- [ ] Add MCP tool in packages/mcp/src/\n- [ ] Add CLI command in apps/cli/src/commands/\n- [ ] Add web page in apps/web/src/routes/\n- [ ] Update CLAUDE.md\n- [ ] Run pnpm ci",
    type: "checklist",
    userId: DEV_USER_ID,
    origin: "human",
    reviewStatus: "approved",
});
```

Tag the note chunks with a "convention" tag so the structured formatter (Feature 6 from context improvements) can group them correctly.

- [ ] **Step 4: Add chunk_version seed data**

After chunk creation, add version history for 2-3 chunks:

```typescript
// Version history for a couple of chunks
await db.insert(chunkVersion).values([
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch, // architecture overview chunk
        version: 1,
        title: "Architecture Overview (original)",
        content: "Initial architecture notes before the plans rewrite.",
        type: "document",
        tags: [],
        createdAt: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000), // 7 days ago
    },
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch,
        version: 2,
        title: "Architecture Overview",
        content: "Updated after session removal and plans-as-central-entity rewrite.",
        type: "document",
        tags: [],
        createdAt: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000), // 2 days ago
    },
]);
```

Adapt `ids.arch` to whatever the actual chunk ID variable is for the architecture overview chunk.

- [ ] **Step 5: Add chunk_staleness seed data**

```typescript
// Staleness flags — make the dashboard "Attention Needed" widget show content
await db.insert(chunkStaleness).values([
    {
        id: crypto.randomUUID(),
        chunkId: /* pick a chunk that hasn't been updated recently — use one of the existing IDs */,
        reason: "age",
        detail: "Not updated in 120 days",
        detectedAt: new Date(),
    },
    {
        id: crypto.randomUUID(),
        chunkId: /* pick two related chunks */,
        reason: "diverged_duplicate",
        detail: "Content overlaps significantly with another chunk",
        relatedChunkId: /* the other chunk */,
        detectedAt: new Date(),
    },
]);
```

Use actual chunk IDs from the seeded chunks. Pick chunks that make sense (e.g., the env chunk and the docker chunk could be "stale").

- [ ] **Step 6: Add chunk_proposal seed data**

```typescript
// Proposals — make the /review page have content
await db.insert(chunkProposal).values([
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch, // propose changes to the architecture chunk
        changes: {
            title: "Architecture Overview (updated)",
            content: "Updated architecture overview reflecting the plans-as-central-entity rewrite and session removal.",
        },
        reason: "Architecture chunk is stale after the plans rewrite removed the sessions subsystem",
        status: "pending",
        proposedBy: "ai",
        createdAt: new Date(Date.now() - 2 * 60 * 60 * 1000), // 2 hours ago
    },
    {
        id: crypto.randomUUID(),
        chunkId: noteChunk1Id, // propose an edit to the auth convention
        changes: {
            content: "All API routes require a valid session token via Better Auth. Routes use requireSession(ctx) from the shared middleware. Public routes (health, login) are explicitly excluded.",
            tags: ["convention", "auth"],
        },
        reason: "Simplified wording and added tags",
        status: "pending",
        proposedBy: "ai",
        createdAt: new Date(Date.now() - 30 * 60 * 1000), // 30 minutes ago
    },
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch,
        changes: { title: "System Architecture" },
        reason: "Rename for clarity",
        status: "approved",
        proposedBy: "ai",
        reviewedBy: DEV_USER_ID,
        reviewedAt: new Date(Date.now() - 24 * 60 * 60 * 1000),
        reviewNote: "Good rename",
        createdAt: new Date(Date.now() - 48 * 60 * 60 * 1000),
    },
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch,
        changes: { type: "reference" },
        reason: "Should be a reference not a document",
        status: "rejected",
        proposedBy: "ai",
        reviewedBy: DEV_USER_ID,
        reviewedAt: new Date(Date.now() - 24 * 60 * 60 * 1000),
        reviewNote: "It's correctly a document",
        createdAt: new Date(Date.now() - 72 * 60 * 60 * 1000),
    },
]);
```

This gives 2 pending + 1 approved + 1 rejected — all four statuses represented.

- [ ] **Step 7: Add plan variety (draft, ready, archived) + task variety (blocked, skipped) + task chunks + task dependencies**

After the existing plan section, add:

```typescript
// Draft plan — just a title, no tasks yet
const [planDraft] = await db.insert(plan).values({
    title: "Improve graph visualization performance",
    description: "The force-directed graph slows down with 200+ nodes. Investigate WebGL rendering or spatial partitioning.",
    status: "draft",
    userId: DEV_USER_ID,
    codebaseId: fubbikCodebaseId,
}).returning();

// Ready plan — analyzed + tasks drafted, not yet started
const [planReady] = await db.insert(plan).values({
    title: "Add chunk templates for common patterns",
    description: "Create a template system so users can quickly create chunks for conventions, API endpoints, runbooks, etc.",
    status: "ready",
    userId: DEV_USER_ID,
    codebaseId: fubbikCodebaseId,
}).returning();

if (planReady) {
    const [readyTask1] = await db.insert(planTask).values({
        planId: planReady.id,
        title: "Design template schema",
        status: "pending",
        order: 0,
        acceptanceCriteria: ["Templates stored in DB", "Built-in templates are read-only", "Users can duplicate and customize"],
    }).returning();

    const [readyTask2] = await db.insert(planTask).values({
        planId: planReady.id,
        title: "Build template picker UI",
        status: "blocked",
        order: 1,
        description: "Blocked by template schema task",
    }).returning();

    // Task dependency: task2 depends on task1
    if (readyTask1 && readyTask2) {
        await db.insert(planTaskDependency).values({
            taskId: readyTask2.id,
            dependsOnTaskId: readyTask1.id,
        });
    }

    const [readyTask3] = await db.insert(planTask).values({
        planId: planReady.id,
        title: "Add CLI template support",
        status: "skipped",
        order: 2,
        description: "Decided to defer CLI support to a follow-up",
    }).returning();
}

// Archived plan
await db.insert(plan).values({
    title: "Evaluate GraphQL for API layer",
    description: "Explored but decided to stick with REST + Eden for now.",
    status: "archived",
    userId: DEV_USER_ID,
    codebaseId: fubbikCodebaseId,
});

// Add task-chunk links to the in_progress plan's tasks
// (use the existing planInProgress and its tasks from the prior section)
```

For task-chunk links, find the in_progress plan's task IDs and link them to existing chunks:

```typescript
// Link tasks to chunks (planInProgress tasks from earlier)
// Read the existing task IDs — they were inserted with .returning() in the prior section
// If task variables are still in scope:
if (/* first task ID from planInProgress */) {
    await db.insert(planTaskChunk).values({
        taskId: /* first task ID */,
        chunkId: ids.arch,
        relation: "context",
    });
}
```

**Adapt:** The exact variable names for the in_progress plan's tasks depend on what the current seed code uses. Read the plan seeding section to find them.

- [ ] **Step 8: Add activity_log, favorite, notification, comment, saved_query seed data**

```typescript
// Activity log — makes /activity page show content
const activityTable = /* import name — check schema */;
await db.insert(activityTable).values([
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        action: "created",
        entityType: "chunk",
        entityId: ids.arch,
        createdAt: new Date(Date.now() - 60 * 60 * 1000),
    },
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        action: "updated",
        entityType: "chunk",
        entityId: ids.arch,
        createdAt: new Date(Date.now() - 30 * 60 * 1000),
    },
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        action: "created",
        entityType: "plan",
        entityId: planInProgress?.id ?? "unknown",
        createdAt: new Date(Date.now() - 45 * 60 * 1000),
    },
]);

// Favorites
await db.insert(favorite).values([
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        chunkId: ids.arch,
        createdAt: new Date(),
    },
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        chunkId: noteChunk1Id,
        createdAt: new Date(),
    },
]);

// Notifications
const notificationTable = /* import name — check schema */;
await db.insert(notificationTable).values([
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        type: "proposal_created",
        message: "AI proposed changes to Architecture Overview",
        entityType: "chunk",
        entityId: ids.arch,
        read: false,
        createdAt: new Date(Date.now() - 2 * 60 * 60 * 1000),
    },
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        type: "staleness_detected",
        message: "2 chunks flagged as stale",
        read: true,
        createdAt: new Date(Date.now() - 24 * 60 * 60 * 1000),
    },
]);

// Comments on a chunk
await db.insert(chunkComment).values([
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch,
        userId: DEV_USER_ID,
        content: "This needs updating after the plans rewrite — sessions are gone now.",
        createdAt: new Date(Date.now() - 4 * 60 * 60 * 1000),
    },
    {
        id: crypto.randomUUID(),
        chunkId: ids.arch,
        userId: DEV_USER_ID,
        content: "Updated. Also linked to the plans-as-central-entity plan for tracking.",
        createdAt: new Date(Date.now() - 2 * 60 * 60 * 1000),
    },
]);

// Saved query
await db.insert(savedQuery).values([
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        name: "API docs needing review",
        query: { clauses: [{ field: "type", operator: "is", value: "reference" }, { field: "review", operator: "is", value: "draft" }] },
        createdAt: new Date(),
    },
    {
        id: crypto.randomUUID(),
        userId: DEV_USER_ID,
        name: "Stale architecture docs",
        query: { clauses: [{ field: "type", operator: "is", value: "document" }, { field: "updated", operator: "is", value: "90d" }] },
        createdAt: new Date(),
    },
]);
```

**IMPORTANT:** Read every schema file before inserting to confirm exact column names. The code above uses guessed column names (`action`, `entityType`, `message`, `read`, etc.). The actual schemas may differ. Read:
- `packages/db/src/schema/activity.ts` (or wherever `activityLog` is defined)
- `packages/db/src/schema/favorite.ts`
- `packages/db/src/schema/notification.ts`
- `packages/db/src/schema/comment.ts`
- `packages/db/src/schema/saved-query.ts`

Adapt all column names to match.

- [ ] **Step 9: Fix section numbering in comments**

Read through the file's section comments. They should be numbered sequentially: 1, 2, 3, ... N. Fix any jumps, duplicates, or dead references like "Second plan seeding moved to section 6".

- [ ] **Step 10: Run the seed**

```bash
pnpm seed 2>&1 | tail -30
```

Expected: no errors.

Verify coverage:

```bash
psql "${DATABASE_URL:-postgres://localhost/fubbik}" -c "
SELECT 'plan' AS t, count(*) FROM plan
UNION ALL SELECT 'plan_task', count(*) FROM plan_task
UNION ALL SELECT 'plan_task_dependency', count(*) FROM plan_task_dependency
UNION ALL SELECT 'plan_task_chunk', count(*) FROM plan_task_chunk
UNION ALL SELECT 'plan_analyze_item', count(*) FROM plan_analyze_item
UNION ALL SELECT 'chunk_proposal', count(*) FROM chunk_proposal
UNION ALL SELECT 'chunk_staleness', count(*) FROM chunk_staleness
UNION ALL SELECT 'chunk_version', count(*) FROM chunk_version
UNION ALL SELECT 'activity_log', count(*) FROM activity_log
UNION ALL SELECT 'favorite', count(*) FROM favorite
UNION ALL SELECT 'notification', count(*) FROM notification
UNION ALL SELECT 'chunk_comment', count(*) FROM chunk_comment
UNION ALL SELECT 'saved_query', count(*) FROM saved_query
ORDER BY 1;
"
```

Expected: all counts > 0.

- [ ] **Step 11: Commit**

```bash
git add packages/db/src/seed.ts
git commit -m "feat(db): add seed data for 15 empty tables, add chunk type + plan status variety

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Verify + Push

- [ ] **Step 1: Run the full seed against clean local DB**

```bash
pnpm db:push && pnpm seed 2>&1 | tail -20
```

Expected: clean run, no errors.

- [ ] **Step 2: Start the server and spot-check**

```bash
timeout 15 bun run --cwd apps/server src/index.ts 2>&1 | head -5
```

Then check key endpoints:

```bash
curl -s http://localhost:3000/api/plans | python3 -m json.tool | head -20
curl -s http://localhost:3000/api/proposals?status=pending | python3 -m json.tool | head -20
curl -s http://localhost:3000/api/proposals/count | python3 -m json.tool
```

Expected: plans list shows 6 plans (3 original + 3 new), proposals shows 2 pending, count shows `{"pending":2}`.

- [ ] **Step 3: Push**

```bash
git push origin main
```
