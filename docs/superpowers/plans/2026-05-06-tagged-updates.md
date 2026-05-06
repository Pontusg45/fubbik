# Tagged Chunk Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-edit update tags to chunk versions so changes can be labeled (e.g., "feature-x") and queried across codebases with before/after diffs.

**Architecture:** Extend the `chunk_version` schema with `updateTag` and expanded snapshot fields. Modify `createVersion` / `updateChunk` / `createChunk` to accept and store tags. Add query endpoints for tagged versions with diff computation. Add CLI `updates` command and MCP `list_updates` tool.

**Tech Stack:** Drizzle (schema), Effect (service), Elysia (routes), Commander.js (CLI), Zod (MCP schemas)

---

### Task 1: Extend `chunk_version` schema

**Files:**
- Modify: `packages/db/src/schema/chunk-version.ts`

- [ ] **Step 1: Add new columns to `chunk_version` table**

In `packages/db/src/schema/chunk-version.ts`, replace the entire file:

```typescript
import { pgTable, text, timestamp, integer, jsonb, index } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";

export const chunkVersion = pgTable(
    "chunk_version",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        version: integer("version").notNull(),
        title: text("title").notNull(),
        content: text("content").notNull(),
        type: text("type").notNull(),
        tags: jsonb("tags").$type<string[]>().notNull(),
        rationale: text("rationale"),
        alternatives: jsonb("alternatives").$type<string[]>(),
        consequences: text("consequences"),
        scope: jsonb("scope").$type<Record<string, string>>(),
        updateTag: text("update_tag"),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("chunk_version_chunkId_idx").on(table.chunkId),
        index("chunk_version_update_tag_idx").on(table.updateTag)
    ]
);
```

- [ ] **Step 2: Push schema to database**

Run: `pnpm db:push`
Expected: Schema updated with new columns.

- [ ] **Step 3: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/chunk-version.ts
git commit -m "feat: extend chunk_version with updateTag and expanded snapshot fields"
```

---

### Task 2: Update version repository with tag support and query functions

**Files:**
- Modify: `packages/db/src/repository/chunk-version.ts`

- [ ] **Step 1: Expand `CreateVersionParams` and add query functions**

Replace `packages/db/src/repository/chunk-version.ts`:

```typescript
import { and, desc, eq, isNotNull, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkVersion } from "../schema/chunk-version";

export interface CreateVersionParams {
    id: string;
    chunkId: string;
    version: number;
    title: string;
    content: string;
    type: string;
    tags: string[];
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
    scope?: Record<string, string> | null;
    updateTag?: string | null;
}

export function createVersion(params: CreateVersionParams) {
    return dbEffect(async () => {
        const [created] = await db.insert(chunkVersion).values(params).returning();
        return created;
    });
}

export function getVersionsByChunkId(chunkId: string) {
    return dbEffect(() =>
        db.select().from(chunkVersion).where(eq(chunkVersion.chunkId, chunkId)).orderBy(desc(chunkVersion.version))
    );
}

export function getNextVersionNumber(chunkId: string) {
    return dbEffect(async () => {
        const result = await db
            .select({ maxVersion: sql<number>`COALESCE(MAX(${chunkVersion.version}), 0)` })
            .from(chunkVersion)
            .where(eq(chunkVersion.chunkId, chunkId));
        return (result[0]?.maxVersion ?? 0) + 1;
    });
}

export function getVersionsByTag(tag: string, userId: string, codebaseId?: string) {
    return dbEffect(async () => {
        const conditions = [
            eq(chunkVersion.updateTag, tag),
            eq(chunk.userId, userId)
        ];
        if (codebaseId) {
            const { chunkCodebase } = await import("../schema/chunk");
            conditions.push(
                sql`EXISTS (SELECT 1 FROM chunk_codebase WHERE chunk_codebase.chunk_id = ${chunkVersion.chunkId} AND chunk_codebase.codebase_id = ${codebaseId})`
            );
        }

        const versions = await db
            .select({
                versionId: chunkVersion.id,
                chunkId: chunkVersion.chunkId,
                version: chunkVersion.version,
                updateTag: chunkVersion.updateTag,
                title: chunkVersion.title,
                content: chunkVersion.content,
                type: chunkVersion.type,
                rationale: chunkVersion.rationale,
                alternatives: chunkVersion.alternatives,
                consequences: chunkVersion.consequences,
                scope: chunkVersion.scope,
                createdAt: chunkVersion.createdAt,
                chunkTitle: chunk.title,
                chunkContent: chunk.content,
                chunkType: chunk.type,
                chunkRationale: chunk.rationale,
                chunkAlternatives: chunk.alternatives,
                chunkConsequences: chunk.consequences,
                chunkScope: chunk.scope,
            })
            .from(chunkVersion)
            .innerJoin(chunk, eq(chunk.id, chunkVersion.chunkId))
            .where(and(...conditions))
            .orderBy(desc(chunkVersion.createdAt));

        return versions;
    });
}

export function getDistinctUpdateTags(userId: string, codebaseId?: string) {
    return dbEffect(async () => {
        const conditions = [
            isNotNull(chunkVersion.updateTag),
            eq(chunk.userId, userId)
        ];
        if (codebaseId) {
            conditions.push(
                sql`EXISTS (SELECT 1 FROM chunk_codebase WHERE chunk_codebase.chunk_id = ${chunkVersion.chunkId} AND chunk_codebase.codebase_id = ${codebaseId})`
            );
        }

        const result = await db
            .select({
                tag: chunkVersion.updateTag,
                count: sql<number>`count(*)`.as("count"),
            })
            .from(chunkVersion)
            .innerJoin(chunk, eq(chunk.id, chunkVersion.chunkId))
            .where(and(...conditions))
            .groupBy(chunkVersion.updateTag)
            .orderBy(chunkVersion.updateTag);

        return result as { tag: string; count: number }[];
    });
}
```

- [ ] **Step 2: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/chunk-version.ts
git commit -m "feat: version repository with updateTag support and query functions"
```

---

### Task 3: Update chunk service to pass updateTag and expanded snapshots

**Files:**
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Add `updateTag` to `updateChunk` body type and version creation**

In `packages/api/src/chunks/service.ts`, find the `updateChunk` function (around line 300). Add `updateTag?: string` to the body type:

```typescript
export function updateChunk(
    chunkId: string,
    userId: string,
    body: {
        title?: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        summary?: string | null;
        aliases?: string[];
        notAbout?: string[];
        scope?: Record<string, string>;
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
        origin?: string;
        reviewStatus?: string;
        isEntryPoint?: boolean;
        updateTag?: string;
    }
)
```

Update the `createVersion` call inside the function to include expanded fields and the tag. Find:

```typescript
            createVersion({
                id: crypto.randomUUID(),
                chunkId,
                version,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: []
            })
```

Replace with:

```typescript
            createVersion({
                id: crypto.randomUUID(),
                chunkId,
                version,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: [],
                rationale: existing.rationale,
                alternatives: existing.alternatives,
                consequences: existing.consequences,
                scope: existing.scope,
                updateTag: body.updateTag
            })
```

Also, strip `updateTag` from the repo update body. Find:

```typescript
            const { tags: _tags, codebaseIds: _codebaseIds, ...repoBody } = body;
```

Replace with:

```typescript
            const { tags: _tags, codebaseIds: _codebaseIds, updateTag: _updateTag, ...repoBody } = body;
```

- [ ] **Step 2: Add `updateTag` to `createChunk` and create version-0 on tagged creation**

In the `createChunk` function, add `updateTag?: string` to the body type:

```typescript
export function createChunk(
    userId: string,
    body: {
        title: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
        origin?: string;
        documentId?: string;
        documentOrder?: number;
        updateTag?: string;
    }
)
```

After the `events.emit(EVENTS.CHUNK_CREATED, ...)` tap, add a new tap to create a version-0 when updateTag is provided:

```typescript
        Effect.tap(() => {
            if (body.updateTag) {
                return createVersion({
                    id: crypto.randomUUID(),
                    chunkId: id,
                    version: 0,
                    title: "",
                    content: "",
                    type: "",
                    tags: [],
                    updateTag: body.updateTag
                });
            }
            return Effect.void;
        })
```

Add this tap before the final closing of the pipe chain, after the `CHUNK_CREATED` event emit tap.

- [ ] **Step 3: Add `listUpdatesByTag` and `listUpdateTags` service functions**

Add these imports at the top of the service file (with existing imports from `@fubbik/db/repository`):

```typescript
import {
    // ... existing imports ...
    getVersionsByTag,
    getDistinctUpdateTags,
} from "@fubbik/db/repository";
```

Add these functions at the end of the service file:

```typescript
export function listUpdatesByTag(userId: string, tag: string, codebaseId?: string) {
    return getVersionsByTag(tag, userId, codebaseId).pipe(
        Effect.map(versions => versions.map(v => ({
            versionId: v.versionId,
            chunkId: v.chunkId,
            chunkTitle: v.chunkTitle,
            updateTag: v.updateTag,
            version: v.version,
            createdAt: v.createdAt,
            before: v.version === 0
                ? { title: null, content: null, type: null, rationale: null, alternatives: null, consequences: null, scope: null }
                : { title: v.title, content: v.content, type: v.type, rationale: v.rationale, alternatives: v.alternatives, consequences: v.consequences, scope: v.scope },
            after: {
                title: v.chunkTitle,
                content: v.chunkContent,
                type: v.chunkType,
                rationale: v.chunkRationale,
                alternatives: v.chunkAlternatives,
                consequences: v.chunkConsequences,
                scope: v.chunkScope,
            }
        })))
    );
}

export function listUpdateTags(userId: string, codebaseId?: string) {
    return getDistinctUpdateTags(userId, codebaseId);
}
```

- [ ] **Step 4: Run type-check and tests**

Run: `pnpm run check-types && pnpm test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/service.ts
git commit -m "feat: updateTag support in chunk create/update and query services"
```

---

### Task 4: Add API routes for updateTag and query endpoints

**Files:**
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Add `updateTag` to POST and PATCH body schemas**

In `packages/api/src/chunks/routes.ts`, find the POST `/chunks` body schema (around line 288). Add after `documentOrder`:

```typescript
                updateTag: t.Optional(t.String({ maxLength: 100 }))
```

Find the PATCH `/chunks:id` body schema (around line 311). Add after `isEntryPoint`:

```typescript
                updateTag: t.Optional(t.String({ maxLength: 100 }))
```

- [ ] **Step 2: Add GET `/chunks/updates` endpoint**

Add before the `.post("/chunks", ...)` route (so it doesn't conflict with the `:id` param route):

```typescript
    .get(
        "/chunks/updates",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        chunkService.listUpdatesByTag(session.user.id, ctx.query.tag, ctx.query.codebaseId)
                    ),
                    Effect.map(updates => ({ updates }))
                )
            ),
        {
            query: t.Object({
                tag: t.String({ minLength: 1, maxLength: 100 }),
                codebaseId: t.Optional(t.String())
            })
        }
    )
```

- [ ] **Step 3: Add GET `/chunks/updates/tags` endpoint**

Add right after the `/chunks/updates` route:

```typescript
    .get(
        "/chunks/updates/tags",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        chunkService.listUpdateTags(session.user.id, ctx.query.codebaseId)
                    ),
                    Effect.map(tags => ({ tags }))
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
```

- [ ] **Step 4: Run type-check and tests**

Run: `pnpm run check-types && pnpm test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/routes.ts
git commit -m "feat: API routes for tagged updates query and tag listing"
```

---

### Task 5: CLI `--tag` option on add/update/quick and `updates` command

**Files:**
- Modify: `apps/cli/src/commands/update.ts`
- Modify: `apps/cli/src/commands/add.ts`
- Modify: `apps/cli/src/commands/quick.ts`
- Create: `apps/cli/src/commands/updates.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Add `--tag` to update command**

In `apps/cli/src/commands/update.ts`, add the option after `--content-file`:

```typescript
    .option("--tag <tag>", "label this update with a tag (e.g. feature-x)")
```

Update the opts type in the action handler:

```typescript
            opts: { title?: string; content?: string; type?: string; tags?: string; contentFile?: string; tag?: string },
```

Add after the content handling, before the "No updates" check:

```typescript
            if (opts.tag !== undefined) updates.updateTag = opts.tag;
```

- [ ] **Step 2: Add `--tag` to quick command**

In `apps/cli/src/commands/quick.ts`, add the option after `--codebase`:

```typescript
    .option("--tag <tag>", "label this creation with an update tag")
```

Update the opts type:

```typescript
    }, cmd: Command) => {
```

Add `tag?: string` to the opts type in the action handler.

In the server mode body construction, add after the codebaseIds:

```typescript
            if (opts.tag) {
                body.updateTag = opts.tag;
            }
```

In the local mode, the local store doesn't support update tags — skip it there.

- [ ] **Step 3: Add `--tag` to add command**

Read the add command file first to understand its structure, then add `--tag <tag>` option and pass `updateTag` to the API body in server mode, same pattern as quick.

- [ ] **Step 4: Create the `updates` command**

Create `apps/cli/src/commands/updates.ts`:

```typescript
import { Command } from "commander";

import { formatId, formatSuccess, formatTag } from "../lib/colors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

export const updatesCommand = new Command("updates")
    .description("List chunk updates by tag")
    .option("--tag <tag>", "filter updates by tag")
    .option("--tags", "list all update tags with counts")
    .option("--codebase-id <id>", "filter to a specific codebase")
    .action(async (opts: { tag?: string; tags?: boolean; codebaseId?: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("Update tags require server mode. Set FUBBIK_SERVER_URL.");
            return;
        }

        if (opts.tags) {
            const params = new URLSearchParams();
            if (opts.codebaseId) params.set("codebaseId", opts.codebaseId);

            try {
                const res = await fetch(`${serverUrl}/api/chunks/updates/tags?${params}`);
                if (!res.ok) {
                    outputError(`Server error (${res.status}): ${await res.text()}`);
                    return;
                }
                const { tags } = (await res.json()) as { tags: { tag: string; count: number }[] };
                if (tags.length === 0) {
                    output(cmd, tags, "No update tags found.");
                    return;
                }
                const lines = tags.map(t => `  ${formatTag(t.tag)} — ${t.count} update${t.count !== 1 ? "s" : ""}`);
                output(cmd, tags, ["Update tags:", ...lines].join("\n"));
            } catch (err) {
                outputError(`Failed to connect: ${err instanceof Error ? err.message : err}`);
            }
            return;
        }

        if (!opts.tag) {
            outputError("Provide --tag <name> to list updates, or --tags to list all tags.");
            return;
        }

        const params = new URLSearchParams({ tag: opts.tag });
        if (opts.codebaseId) params.set("codebaseId", opts.codebaseId);

        try {
            const res = await fetch(`${serverUrl}/api/chunks/updates?${params}`);
            if (!res.ok) {
                outputError(`Server error (${res.status}): ${await res.text()}`);
                return;
            }
            const { updates } = (await res.json()) as {
                updates: Array<{
                    versionId: string;
                    chunkId: string;
                    chunkTitle: string;
                    version: number;
                    createdAt: string;
                    before: { title: string | null; content: string | null };
                    after: { title: string; content: string };
                }>;
            };

            if (updates.length === 0) {
                output(cmd, updates, `No updates found for tag "${opts.tag}".`);
                return;
            }

            const lines: string[] = [
                formatSuccess(`${updates.length} update${updates.length !== 1 ? "s" : ""} tagged "${opts.tag}":`),
                ""
            ];

            for (const u of updates) {
                const isCreation = u.version === 0;
                const action = isCreation ? "created" : "updated";
                const date = new Date(u.createdAt).toLocaleDateString();
                lines.push(`  ${formatId(u.chunkId)} ${u.chunkTitle} — ${action} ${date}`);

                if (!isCreation && u.before.title && u.before.title !== u.after.title) {
                    lines.push(`    title: "${u.before.title}" → "${u.after.title}"`);
                }
            }

            output(cmd, updates, lines.join("\n"));
        } catch (err) {
            outputError(`Failed to connect: ${err instanceof Error ? err.message : err}`);
        }
    });
```

- [ ] **Step 5: Register the `updates` command**

In `apps/cli/src/index.ts`, add:

```typescript
import { updatesCommand } from "./commands/updates";
```

And register it:

```typescript
program.addCommand(updatesCommand);
```

- [ ] **Step 6: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add apps/cli/src/commands/update.ts apps/cli/src/commands/add.ts apps/cli/src/commands/quick.ts apps/cli/src/commands/updates.ts apps/cli/src/index.ts
git commit -m "feat: CLI --tag option and updates command for tagged chunk changes"
```

---

### Task 6: MCP tool updates

**Files:**
- Modify: `packages/mcp/src/tools.ts`

- [ ] **Step 1: Add `updateTag` to `create_chunk` tool**

In `packages/mcp/src/tools.ts`, find the `create_chunk` tool schema (around line 90). Add to the schema object:

```typescript
            updateTag: z.string().optional().describe("Label this creation with an update tag (e.g. feature-x)")
```

In the handler, add to the body construction:

```typescript
            if (updateTag) body.updateTag = updateTag;
```

Update the destructured params to include `updateTag`.

- [ ] **Step 2: Add `updateTag` to `update_chunk` tool**

Find the `update_chunk` tool schema (around line 233). Add to the schema:

```typescript
            updateTag: z.string().optional().describe("Label this update with a tag (e.g. feature-x)")
```

In the handler, add:

```typescript
            if (updateTag) body.updateTag = updateTag;
```

Update the destructured params to include `updateTag`.

- [ ] **Step 3: Add `list_updates` tool**

Add after the `update_chunk` tool registration:

```typescript
    server.tool(
        "list_updates",
        "List chunk updates labeled with a specific tag. Returns before/after diffs for each change.",
        {
            tag: z.string().describe("Update tag to filter by (e.g. feature-x)"),
            codebaseId: z.string().optional().describe("Codebase ID to scope results")
        },
        async ({ tag, codebaseId }) => {
            const params = new URLSearchParams({ tag });
            if (codebaseId) params.set("codebaseId", codebaseId);

            const data = (await apiFetch(`/chunks/updates?${params}`)) as {
                updates: Array<{
                    chunkId: string;
                    chunkTitle: string;
                    version: number;
                    createdAt: string;
                    before: Record<string, unknown>;
                    after: Record<string, unknown>;
                }>;
            };

            if (data.updates.length === 0) {
                return { content: [{ type: "text" as const, text: `No updates found for tag "${tag}".` }] };
            }

            const summary = data.updates.map(u => {
                const action = u.version === 0 ? "CREATED" : "UPDATED";
                return `${action}: ${u.chunkTitle} (${u.chunkId}) — ${new Date(u.createdAt).toLocaleDateString()}`;
            }).join("\n");

            return {
                content: [{
                    type: "text" as const,
                    text: `${data.updates.length} update(s) tagged "${tag}":\n\n${summary}`
                }]
            };
        }
    );
```

- [ ] **Step 4: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add packages/mcp/src/tools.ts
git commit -m "feat: MCP tools with updateTag support and list_updates tool"
```

---

### Task 7: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 2: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 3: Manual API verification**

Start the dev server: `pnpm dev`

Create a chunk with update tag:
```bash
curl -X POST http://localhost:3000/api/chunks \
  -H "Content-Type: application/json" \
  -d '{"title":"Auth flow","content":"JWT based auth","type":"note","updateTag":"feature-auth"}'
```

Update the chunk with a tag:
```bash
curl -X PATCH http://localhost:3000/api/chunks/<id> \
  -H "Content-Type: application/json" \
  -d '{"content":"JWT with refresh tokens","updateTag":"feature-auth"}'
```

Query tagged updates:
```bash
curl http://localhost:3000/api/chunks/updates?tag=feature-auth
```

List all tags:
```bash
curl http://localhost:3000/api/chunks/updates/tags
```

Verify: creation shows version 0 with null before, update shows before/after diff, tags endpoint shows "feature-auth" with count 2.

- [ ] **Step 4: Commit any fixes**

If verification reveals issues, fix and commit individually.
