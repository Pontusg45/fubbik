# Chunk Retrieval Enhancement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add retrieval-enhancing metadata fields (summary, aliases, not_about, scope, embedding) to chunks, with Ollama-powered enrichment and semantic search.

**Architecture:** New columns on the chunk table, a SQL migration for pgvector + indexes, an Ollama client for embeddings/generation, an async enrichment pipeline triggered after create/update, and a new semantic search API endpoint. CLI gets enrich, semantic search, and filtering flags.

**Tech Stack:** Drizzle ORM, pgvector, Ollama (nomic-embed-text), Effect, Elysia, Commander.js

---

### Task 1: SQL Migration — pgvector Extension + New Columns + Indexes

**Files:**
- Create: `packages/db/src/migrations/0003_add_retrieval_fields.sql`
- Modify: `packages/db/src/schema/chunk.ts:6-24`

**Step 1: Create the SQL migration file**

Create `packages/db/src/migrations/0003_add_retrieval_fields.sql`:

```sql
-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Add retrieval-enhancing columns
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS summary text;
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS aliases jsonb NOT NULL DEFAULT '[]';
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS not_about jsonb NOT NULL DEFAULT '[]';
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS scope jsonb NOT NULL DEFAULT '{}';
ALTER TABLE chunk ADD COLUMN IF NOT EXISTS embedding vector(768);

-- GIN indexes for JSONB array/object filtering
CREATE INDEX IF NOT EXISTS chunk_aliases_gin_idx ON chunk USING gin (aliases);
CREATE INDEX IF NOT EXISTS chunk_not_about_gin_idx ON chunk USING gin (not_about);
CREATE INDEX IF NOT EXISTS chunk_scope_gin_idx ON chunk USING gin (scope);

-- HNSW index for approximate nearest neighbor search (cosine distance)
CREATE INDEX IF NOT EXISTS chunk_embedding_hnsw_idx ON chunk USING hnsw (embedding vector_cosine_ops);
```

**Step 2: Update the Drizzle schema**

Modify `packages/db/src/schema/chunk.ts`. Add imports for `customType` from `drizzle-orm/pg-core`. Define a custom `vector` column type since Drizzle doesn't have native pgvector support. Add the 5 new columns to the chunk table definition.

The new chunk table columns (add after `updatedAt`, before the closing `}` of the column definitions):

```typescript
import { pgTable, text, timestamp, jsonb, index, uniqueIndex, customType } from "drizzle-orm/pg-core";

// Custom pgvector type for Drizzle
const vector = customType<{ data: number[]; driverParam: string }>({
    dataType() {
        return "vector(768)";
    },
    toDriver(value: number[]) {
        return `[${value.join(",")}]`;
    },
    fromDriver(value: string) {
        return value
            .slice(1, -1)
            .split(",")
            .map(Number);
    }
});
```

New columns to add to the chunk pgTable definition:

```typescript
summary: text("summary"),
aliases: jsonb("aliases").$type<string[]>().notNull().default([]),
notAbout: jsonb("not_about").$type<string[]>().notNull().default([]),
scope: jsonb("scope").$type<Record<string, string>>().notNull().default({}),
embedding: vector("embedding"),
```

**Step 3: Run migration against local database**

```bash
cd packages/db
psql $DATABASE_URL -f src/migrations/0003_add_retrieval_fields.sql
```

Expected: all statements succeed (CREATE EXTENSION, ALTER TABLE x5, CREATE INDEX x4).

**Step 4: Verify schema matches with drizzle-kit**

```bash
cd packages/db
bun drizzle-kit push --force
```

Expected: "Changes applied" or "No changes detected" (if migration already matches schema).

**Step 5: Commit**

```bash
git add packages/db/src/migrations/0003_add_retrieval_fields.sql packages/db/src/schema/chunk.ts
git commit -m "feat: add retrieval fields to chunk schema (summary, aliases, not_about, scope, embedding)"
```

---

### Task 2: OLLAMA_URL Environment Variable

**Files:**
- Modify: `packages/env/src/server.ts:6-28`

**Step 1: Add OLLAMA_URL to server env**

In `packages/env/src/server.ts`, add to the `server` object:

```typescript
OLLAMA_URL: type("string | undefined"),
```

And in the `runtimeEnv` object:

```typescript
OLLAMA_URL: process.env.OLLAMA_URL,
```

**Step 2: Verify type check passes**

```bash
cd /Users/pontus/GitHub/fubbik
bun run check-types --filter=@fubbik/env
```

Expected: PASS

**Step 3: Commit**

```bash
git add packages/env/src/server.ts
git commit -m "feat: add OLLAMA_URL env var"
```

---

### Task 3: Ollama Client Library

**Files:**
- Create: `packages/api/src/ollama/client.ts`

**Step 1: Create the Ollama client**

Create `packages/api/src/ollama/client.ts`:

```typescript
import { env } from "@fubbik/env/server";
import { Effect } from "effect";

import { AiError } from "../errors";

const OLLAMA_URL = env.OLLAMA_URL ?? "http://localhost:11434";
const EMBED_MODEL = "nomic-embed-text";

interface OllamaGenerateResponse {
    response: string;
}

interface OllamaEmbeddingResponse {
    embedding: number[];
}

export function isOllamaAvailable(): Effect.Effect<boolean, never> {
    return Effect.tryPromise({
        try: async () => {
            const res = await fetch(`${OLLAMA_URL}/api/tags`, { signal: AbortSignal.timeout(2000) });
            return res.ok;
        },
        catch: () => false
    }).pipe(Effect.catchAll(() => Effect.succeed(false)));
}

export function generateJson<T>(prompt: string, model?: string): Effect.Effect<T, AiError> {
    return Effect.tryPromise({
        try: async () => {
            const res = await fetch(`${OLLAMA_URL}/api/generate`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    model: model ?? "llama3.2",
                    prompt,
                    format: "json",
                    stream: false
                })
            });
            if (!res.ok) throw new Error(`Ollama generate failed: ${res.status}`);
            const data = (await res.json()) as OllamaGenerateResponse;
            return JSON.parse(data.response) as T;
        },
        catch: cause => new AiError({ cause })
    });
}

export function generateEmbedding(text: string): Effect.Effect<number[], AiError> {
    return Effect.tryPromise({
        try: async () => {
            const res = await fetch(`${OLLAMA_URL}/api/embeddings`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    model: EMBED_MODEL,
                    prompt: text
                })
            });
            if (!res.ok) throw new Error(`Ollama embedding failed: ${res.status}`);
            const data = (await res.json()) as OllamaEmbeddingResponse;
            return data.embedding;
        },
        catch: cause => new AiError({ cause })
    });
}

export function generateQueryEmbedding(query: string): Effect.Effect<number[], AiError> {
    return generateEmbedding(`search_query: ${query}`);
}

export function generateDocumentEmbedding(title: string, summary: string | null, content: string): Effect.Effect<number[], AiError> {
    const text = `search_document: ${title}\n${summary ?? ""}\n${content}`.trim();
    return generateEmbedding(text);
}
```

**Step 2: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 3: Commit**

```bash
git add packages/api/src/ollama/client.ts
git commit -m "feat: add Ollama client for embeddings and JSON generation"
```

---

### Task 4: Enrichment Service

**Files:**
- Create: `packages/api/src/enrich/service.ts`
- Modify: `packages/db/src/repository/chunk.ts` (add `updateChunkEnrichment` function)
- Modify: `packages/db/src/repository/index.ts` (export new function)

**Step 1: Add updateChunkEnrichment to chunk repository**

Add to the end of `packages/db/src/repository/chunk.ts`:

```typescript
export interface EnrichChunkParams {
    summary?: string | null;
    aliases?: string[];
    notAbout?: string[];
    scope?: Record<string, string>;
    embedding?: number[];
}

export function updateChunkEnrichment(chunkId: string, params: EnrichChunkParams) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunk)
                .set({
                    ...(params.summary !== undefined && { summary: params.summary }),
                    ...(params.aliases !== undefined && { aliases: params.aliases }),
                    ...(params.notAbout !== undefined && { notAbout: params.notAbout }),
                    ...(params.scope !== undefined && { scope: params.scope }),
                    ...(params.embedding !== undefined && { embedding: params.embedding })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Create the enrichment service**

Create `packages/api/src/enrich/service.ts`:

```typescript
import { getChunkById, updateChunkEnrichment } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";
import { generateDocumentEmbedding, generateJson, isOllamaAvailable } from "../ollama/client";

interface EnrichmentResult {
    summary: string;
    aliases: string[];
    notAbout: string[];
}

export function enrichChunk(chunkId: string) {
    return isOllamaAvailable().pipe(
        Effect.flatMap(available => {
            if (!available) return Effect.succeed(null);

            return getChunkById(chunkId).pipe(
                Effect.flatMap(c => (c ? Effect.succeed(c) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
                Effect.flatMap(c =>
                    Effect.all({
                        metadata: generateJson<EnrichmentResult>(
                            `Analyze this knowledge chunk and return JSON with these fields:
- "summary": a 1-2 sentence TL;DR of the content
- "aliases": an array of 3-8 alternative names, abbreviations, or search terms someone might use to find this
- "notAbout": an array of 2-5 terms this chunk could be confused with but is NOT about

Title: ${c.title}
Type: ${c.type}
Tags: ${(c.tags as string[]).join(", ")}

Content:
${c.content}`
                        ),
                        embedding: generateDocumentEmbedding(c.title, c.summary, c.content)
                    })
                ),
                Effect.flatMap(({ metadata, embedding }) =>
                    updateChunkEnrichment(chunkId, {
                        summary: metadata.summary,
                        aliases: metadata.aliases,
                        notAbout: metadata.notAbout,
                        embedding
                    })
                )
            );
        })
    );
}

export function enrichChunkIfEmpty(chunkId: string) {
    return getChunkById(chunkId).pipe(
        Effect.flatMap(c => {
            if (!c) return Effect.succeed(null);
            // Only enrich if summary is missing (proxy for "not yet enriched")
            if (c.summary) return Effect.succeed(c);
            return enrichChunk(chunkId);
        })
    );
}
```

**Step 3: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 4: Commit**

```bash
git add packages/db/src/repository/chunk.ts packages/api/src/enrich/service.ts
git commit -m "feat: add chunk enrichment service with Ollama integration"
```

---

### Task 5: Fire-and-Forget Enrichment on Create/Update

**Files:**
- Modify: `packages/api/src/chunks/service.ts`

**Step 1: Add enrichment triggers**

At the top of `packages/api/src/chunks/service.ts`, add the import:

```typescript
import { enrichChunk, enrichChunkIfEmpty } from "../enrich/service";
```

Modify the `createChunk` function to trigger enrichment after creation:

```typescript
export function createChunk(userId: string, body: { title: string; content?: string; type?: string; tags?: string[] }) {
    const id = crypto.randomUUID();
    return createChunkRepo({
        id,
        title: body.title,
        content: body.content ?? "",
        type: body.type ?? "note",
        tags: body.tags ?? [],
        userId
    }).pipe(
        Effect.tap(created => {
            // Fire-and-forget enrichment
            Effect.runPromise(enrichChunkIfEmpty(id)).catch(() => {});
            return Effect.succeed(created);
        })
    );
}
```

Modify the `updateChunk` function — add enrichment after the update (last line of the pipe chain, before the closing parenthesis):

```typescript
Effect.flatMap(() => updateChunkRepo(chunkId, body)),
Effect.tap(updated => {
    // Re-enrich if title or content changed
    if (body.title !== undefined || body.content !== undefined) {
        Effect.runPromise(enrichChunk(chunkId)).catch(() => {});
    }
    return Effect.succeed(updated);
})
```

**Step 2: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 3: Commit**

```bash
git add packages/api/src/chunks/service.ts
git commit -m "feat: trigger async enrichment on chunk create/update"
```

---

### Task 6: Enrich API Endpoint

**Files:**
- Create: `packages/api/src/enrich/routes.ts`
- Modify: `packages/api/src/index.ts` (register routes)

**Step 1: Create enrich routes**

Create `packages/api/src/enrich/routes.ts`:

```typescript
import { listChunks } from "@fubbik/db/repository";
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { enrichChunk } from "./service";

export const enrichRoutes = new Elysia()
    .post(
        "/chunks/:id/enrich",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(() => enrichChunk(ctx.params.id)))),
        { params: t.Object({ id: t.String() }) }
    )
    .post("/chunks/enrich-all", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => listChunks({ userId: session.user.id, limit: 1000, offset: 0 })),
                Effect.flatMap(result =>
                    Effect.forEach(
                        result.chunks,
                        c => enrichChunk(c.id).pipe(Effect.catchAll(() => Effect.succeed(null))),
                        { concurrency: 1 }
                    )
                ),
                Effect.map(results => ({ enriched: results.filter(Boolean).length }))
            )
        )
    );
```

**Step 2: Register in API index**

In `packages/api/src/index.ts`, add:

```typescript
import { enrichRoutes } from "./enrich/routes";
```

And add `.use(enrichRoutes)` after `.use(aiRoutes)` in the Elysia chain.

**Step 3: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 4: Commit**

```bash
git add packages/api/src/enrich/routes.ts packages/api/src/index.ts
git commit -m "feat: add /chunks/:id/enrich and /chunks/enrich-all endpoints"
```

---

### Task 7: Enhanced List Query — exclude, scope, alias Filters

**Files:**
- Modify: `packages/db/src/repository/chunk.ts:8-51` (ListChunksParams and listChunks)
- Modify: `packages/api/src/chunks/service.ts:17-23` (pass new params)
- Modify: `packages/api/src/chunks/routes.ts:8-18` (add query params)

**Step 1: Update ListChunksParams and listChunks in repository**

In `packages/db/src/repository/chunk.ts`, update `ListChunksParams`:

```typescript
export interface ListChunksParams {
    userId?: string;
    type?: string;
    search?: string;
    exclude?: string[];
    scope?: Record<string, string>;
    alias?: string;
    limit: number;
    offset: number;
}
```

In the `listChunks` function, add these conditions after the existing `params.search` block:

```typescript
if (params.exclude?.length) {
    for (const term of params.exclude) {
        conditions.push(sql`NOT (${chunk.notAbout} @> ${JSON.stringify([term])}::jsonb)`);
    }
}
if (params.scope && Object.keys(params.scope).length > 0) {
    conditions.push(sql`${chunk.scope} @> ${JSON.stringify(params.scope)}::jsonb`);
}
if (params.alias) {
    conditions.push(sql`${chunk.aliases} @> ${JSON.stringify([params.alias])}::jsonb`);
}
```

**Step 2: Update the service layer**

In `packages/api/src/chunks/service.ts`, update the `listChunks` function signature and body:

```typescript
export function listChunks(
    userId: string | undefined,
    query: { type?: string; search?: string; limit?: string; offset?: string; exclude?: string; scope?: string; alias?: string }
) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);
    const exclude = query.exclude ? query.exclude.split(",").map(s => s.trim()) : undefined;
    const scope = query.scope
        ? Object.fromEntries(query.scope.split(",").map(s => s.trim().split(":")).filter(p => p.length === 2) as [string, string][])
        : undefined;
    return listChunksRepo({ userId, type: query.type, search: query.search, exclude, scope, alias: query.alias, limit, offset }).pipe(
        Effect.map(result => ({ ...result, limit, offset }))
    );
}
```

**Step 3: Update the route query schema**

In `packages/api/src/chunks/routes.ts`, update the query object for GET /chunks:

```typescript
{
    query: t.Object({
        type: t.Optional(t.String()),
        search: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        offset: t.Optional(t.String()),
        exclude: t.Optional(t.String()),
        scope: t.Optional(t.String()),
        alias: t.Optional(t.String())
    })
}
```

**Step 4: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 5: Commit**

```bash
git add packages/db/src/repository/chunk.ts packages/api/src/chunks/service.ts packages/api/src/chunks/routes.ts
git commit -m "feat: add exclude, scope, alias query filters to chunk list endpoint"
```

---

### Task 8: Semantic Search Endpoint

**Files:**
- Create: `packages/db/src/repository/semantic.ts`
- Modify: `packages/db/src/repository/index.ts` (export)
- Modify: `packages/api/src/chunks/routes.ts` (add endpoint)
- Modify: `packages/api/src/chunks/service.ts` (add service function)

**Step 1: Create semantic search repository**

Create `packages/db/src/repository/semantic.ts`:

```typescript
import { and, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";

export interface SemanticSearchParams {
    embedding: number[];
    userId?: string;
    exclude?: string[];
    scope?: Record<string, string>;
    limit: number;
}

export function semanticSearch(params: SemanticSearchParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [sql`${chunk.embedding} IS NOT NULL`];
            if (params.userId) conditions.push(eq(chunk.userId, params.userId));
            if (params.exclude?.length) {
                for (const term of params.exclude) {
                    conditions.push(sql`NOT (${chunk.notAbout} @> ${JSON.stringify([term])}::jsonb)`);
                }
            }
            if (params.scope && Object.keys(params.scope).length > 0) {
                conditions.push(sql`${chunk.scope} @> ${JSON.stringify(params.scope)}::jsonb`);
            }

            const embeddingStr = `[${params.embedding.join(",")}]`;
            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    summary: chunk.summary,
                    type: chunk.type,
                    tags: chunk.tags,
                    aliases: chunk.aliases,
                    scope: chunk.scope,
                    similarity: sql<number>`1 - (${chunk.embedding} <=> ${embeddingStr}::vector)`
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(sql`${chunk.embedding} <=> ${embeddingStr}::vector`)
                .limit(params.limit);

            return results;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:

```typescript
export * from "./semantic";
```

**Step 3: Add service function**

Add to `packages/api/src/chunks/service.ts`:

```typescript
import { semanticSearch as semanticSearchRepo } from "@fubbik/db/repository";
```

And the function:

```typescript
export function semanticSearch(
    userId: string | undefined,
    query: { q: string; limit?: string; exclude?: string; scope?: string }
) {
    const limit = Math.min(Number(query.limit ?? 5), 20);
    const exclude = query.exclude ? query.exclude.split(",").map(s => s.trim()) : undefined;
    const scope = query.scope
        ? Object.fromEntries(query.scope.split(",").map(s => s.trim().split(":")).filter(p => p.length === 2) as [string, string][])
        : undefined;

    return generateQueryEmbedding(query.q).pipe(
        Effect.flatMap(embedding => semanticSearchRepo({ embedding, userId, exclude, scope, limit }))
    );
}
```

Also add the import at the top:

```typescript
import { generateQueryEmbedding } from "../ollama/client";
```

**Step 4: Add route**

In `packages/api/src/chunks/routes.ts`, add BEFORE the `/chunks/:id` route (order matters — specific routes before parameterized):

```typescript
.get(
    "/chunks/search/semantic",
    ctx =>
        Effect.runPromise(
            optionalSession(ctx).pipe(Effect.flatMap(session => chunkService.semanticSearch(session?.user.id, ctx.query)))
        ),
    {
        query: t.Object({
            q: t.String(),
            limit: t.Optional(t.String()),
            exclude: t.Optional(t.String()),
            scope: t.Optional(t.String())
        })
    }
)
```

**Step 5: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 6: Commit**

```bash
git add packages/db/src/repository/semantic.ts packages/db/src/repository/index.ts packages/api/src/chunks/service.ts packages/api/src/chunks/routes.ts
git commit -m "feat: add semantic search endpoint using pgvector + Ollama embeddings"
```

---

### Task 9: Update Chunk PATCH to Accept New Fields

**Files:**
- Modify: `packages/db/src/repository/chunk.ts` (UpdateChunkParams)
- Modify: `packages/api/src/chunks/routes.ts` (PATCH body schema)
- Modify: `packages/api/src/chunks/service.ts` (pass new fields)

**Step 1: Update UpdateChunkParams**

In `packages/db/src/repository/chunk.ts`, update `UpdateChunkParams`:

```typescript
export interface UpdateChunkParams {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
    summary?: string | null;
    aliases?: string[];
    notAbout?: string[];
    scope?: Record<string, string>;
}
```

And update the `updateChunk` function's `.set()` to include:

```typescript
...(params.summary !== undefined && { summary: params.summary }),
...(params.aliases !== undefined && { aliases: params.aliases }),
...(params.notAbout !== undefined && { notAbout: params.notAbout }),
...(params.scope !== undefined && { scope: params.scope }),
```

**Step 2: Update PATCH route body schema**

In `packages/api/src/chunks/routes.ts`, update the PATCH `/chunks/:id` body:

```typescript
body: t.Object({
    title: t.Optional(t.String({ maxLength: 200 })),
    content: t.Optional(t.String({ maxLength: 50000 })),
    type: t.Optional(t.String({ maxLength: 20 })),
    tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 })),
    summary: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
    aliases: t.Optional(t.Array(t.String({ maxLength: 100 }), { maxItems: 20 })),
    notAbout: t.Optional(t.Array(t.String({ maxLength: 100 }), { maxItems: 20 })),
    scope: t.Optional(t.Record(t.String(), t.String()))
})
```

**Step 3: Verify type check**

```bash
bun run check-types --filter=@fubbik/api
```

Expected: PASS

**Step 4: Commit**

```bash
git add packages/db/src/repository/chunk.ts packages/api/src/chunks/routes.ts
git commit -m "feat: accept summary, aliases, notAbout, scope in chunk PATCH"
```

---

### Task 10: CLI — enrich Command

**Files:**
- Create: `apps/cli/src/commands/enrich.ts`
- Modify: `apps/cli/src/index.ts` (register command)

**Step 1: Create enrich command**

Create `apps/cli/src/commands/enrich.ts`:

```typescript
import { Command } from "commander";

import { output, outputError } from "../lib/output";
import { readStore } from "../lib/store";

export const enrichCommand = new Command("enrich")
    .description("Trigger AI enrichment (summary, aliases, embedding) for chunks")
    .argument("[id]", "chunk ID to enrich (omit for all)")
    .option("--all", "enrich all chunks")
    .action(async (id, opts, cmd) => {
        const store = readStore();
        if (!store.serverUrl) {
            outputError("No server URL configured. Run 'fubbik init' first.");
            process.exit(1);
        }

        if (id) {
            const res = await fetch(`${store.serverUrl}/api/chunks/${id}/enrich`, { method: "POST" });
            if (!res.ok) {
                outputError(`Failed to enrich chunk ${id}: ${res.status}`);
                process.exit(1);
            }
            output(cmd, await res.json(), `Enriched chunk ${id}`);
        } else if (opts.all) {
            const res = await fetch(`${store.serverUrl}/api/chunks/enrich-all`, { method: "POST" });
            if (!res.ok) {
                outputError(`Failed to enrich chunks: ${res.status}`);
                process.exit(1);
            }
            const data = await res.json();
            output(cmd, data, `Enriched ${data.enriched} chunks`);
        } else {
            outputError("Provide a chunk ID or use --all");
            process.exit(1);
        }
    });
```

**Step 2: Register in CLI index**

In `apps/cli/src/index.ts`, add:

```typescript
import { enrichCommand } from "./commands/enrich";
```

And:

```typescript
program.addCommand(enrichCommand);
```

**Step 3: Verify build**

```bash
cd apps/cli && bun run build 2>&1
```

Expected: Build succeeds.

**Step 4: Commit**

```bash
git add apps/cli/src/commands/enrich.ts apps/cli/src/index.ts
git commit -m "feat: add CLI enrich command for chunk AI enrichment"
```

---

### Task 11: CLI — Semantic Search Flag + List Filters

**Files:**
- Modify: `apps/cli/src/commands/search.ts`
- Modify: `apps/cli/src/commands/list.ts`

**Step 1: Add --semantic flag to search command**

In `apps/cli/src/commands/search.ts`, add the `.option("--semantic", "use semantic (AI embedding) search")` option.

When `--semantic` is set, call `${store.serverUrl}/api/chunks/search/semantic?q=${encodeURIComponent(query)}&limit=${opts.limit}` instead of the regular search endpoint.

**Step 2: Add --scope and --exclude flags to list command**

In `apps/cli/src/commands/list.ts`, add:
- `.option("--scope <pairs>", "filter by scope (key:value,key:value)")`
- `.option("--exclude <terms>", "exclude chunks about these terms (comma-separated)")`

Pass these as query params: `&scope=${opts.scope}&exclude=${opts.exclude}`

**Step 3: Verify build**

```bash
cd apps/cli && bun run build 2>&1
```

Expected: Build succeeds.

**Step 4: Commit**

```bash
git add apps/cli/src/commands/search.ts apps/cli/src/commands/list.ts
git commit -m "feat: add --semantic search flag and --scope/--exclude list filters to CLI"
```

---

### Task 12: Update Seed Script with Enrichment

**Files:**
- Modify: `packages/db/src/seed.ts`

**Step 1: Add enrichment call to seed**

At the end of the seed script, after inserting chunks and connections, add an enrichment pass. After the connections insertion:

```typescript
// Attempt to enrich all seeded chunks via Ollama
const OLLAMA_URL = process.env.OLLAMA_URL ?? "http://localhost:11434";
try {
    const res = await fetch(`${OLLAMA_URL}/api/tags`, { signal: AbortSignal.timeout(2000) });
    if (res.ok) {
        console.log("\nEnriching chunks via Ollama...");
        for (const c of chunks) {
            try {
                // Generate metadata
                const genRes = await fetch(`${OLLAMA_URL}/api/generate`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        model: "llama3.2",
                        prompt: `Analyze this knowledge chunk and return JSON with these fields:
- "summary": a 1-2 sentence TL;DR
- "aliases": array of 3-8 alternative names or search terms
- "notAbout": array of 2-5 terms this could be confused with but is NOT about

Title: ${c.title}
Type: ${c.type}
Tags: ${c.tags.join(", ")}
Content: ${c.content}`,
                        format: "json",
                        stream: false
                    })
                });
                if (!genRes.ok) continue;
                const genData = await genRes.json() as { response: string };
                const metadata = JSON.parse(genData.response) as { summary: string; aliases: string[]; notAbout: string[] };

                // Generate embedding
                const embRes = await fetch(`${OLLAMA_URL}/api/embeddings`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        model: "nomic-embed-text",
                        prompt: `search_document: ${c.title}\n${metadata.summary}\n${c.content}`
                    })
                });
                if (!embRes.ok) continue;
                const embData = await embRes.json() as { embedding: number[] };

                // Update chunk
                const embStr = `[${embData.embedding.join(",")}]`;
                await db.execute(
                    `UPDATE chunk SET summary = $1, aliases = $2, not_about = $3, embedding = $4 WHERE id = $5`,
                    [metadata.summary, JSON.stringify(metadata.aliases), JSON.stringify(metadata.notAbout), embStr, c.id]
                );
                console.log(`  ✓ Enriched: ${c.title}`);
            } catch (e) {
                console.log(`  ✗ Failed: ${c.title}`);
            }
        }
    } else {
        console.log("\nOllama not available — skipping enrichment");
    }
} catch {
    console.log("\nOllama not available — skipping enrichment");
}
```

**Step 2: Test seed locally**

```bash
cd packages/db && bun run src/seed.ts
```

Expected: Chunks seeded. If Ollama is running, enrichment happens. If not, it's skipped gracefully.

**Step 3: Commit**

```bash
git add packages/db/src/seed.ts
git commit -m "feat: add Ollama enrichment pass to seed script"
```

---

### Task 13: Run Migration on Production

**Step 1: Run migration against prod database**

```bash
cd packages/db
DATABASE_URL="<public-proxy-url>" psql $DATABASE_URL -f src/migrations/0003_add_retrieval_fields.sql
```

Expected: All statements succeed.

**Step 2: Push schema**

```bash
DATABASE_URL="<public-proxy-url>" bun drizzle-kit push --force
```

Expected: "Changes applied"

**Step 3: Deploy server**

```bash
railway service server && railway up --detach
```

**Step 4: Deploy web**

```bash
railway service web && railway up --detach
```

---

### Task 14: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update CLAUDE.md**

Add under "Additional Features" section:

```markdown
- Embeddings: Ollama (nomic-embed-text) for local vector embeddings
```

Add a new section:

```markdown
## Ollama (Optional)

Required for chunk enrichment (summary, aliases, not_about generation) and semantic search.

- `ollama pull nomic-embed-text` — embedding model
- `ollama pull llama3.2` — generation model for metadata
- `OLLAMA_URL` env var (default: `http://localhost:11434`)
- Without Ollama, all other features work normally
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add Ollama setup instructions to CLAUDE.md"
```
