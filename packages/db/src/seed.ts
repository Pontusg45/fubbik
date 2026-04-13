import { resolve } from "path";

import { config } from "dotenv";
import { eq, sql } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

import { user } from "./schema/auth";
import { chunk, chunkConnection } from "./schema/chunk";
import { chunkTag, tag, tagType } from "./schema/tag";
import { codebase, chunkCodebase } from "./schema/codebase";
import { chunkAppliesTo } from "./schema/applies-to";
import { chunkFileRef } from "./schema/file-ref";
import { requirement, requirementChunk } from "./schema/requirement";
import { useCase } from "./schema/use-case";
import { plan, planRequirement, planAnalyzeItem, planTask } from "./schema/plan";
import { vocabularyEntry } from "./schema/vocabulary";
import { collection } from "./schema/collection";
import { workspace, workspaceCodebase } from "./schema/workspace";

config({ path: resolve(import.meta.dirname, "../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

const DEV_USER_ID = "dev-user";

// Ensure dev user exists
const [existing] = await db.select().from(user).where(eq(user.id, DEV_USER_ID));
if (!existing) {
    await db.insert(user).values({
        id: DEV_USER_ID,
        name: "Dev User",
        email: "dev@localhost",
        emailVerified: false
    });
}

// Clear existing data for dev user (order matters for FK constraints)
// New entity cleanup (must come before chunk/codebase deletion)
await db.delete(workspaceCodebase).catch(() => {});
await db.delete(workspace).where(eq(workspace.userId, DEV_USER_ID)).catch(() => {});
await db.delete(collection).where(eq(collection.userId, DEV_USER_ID)).catch(() => {});
await db.delete(vocabularyEntry).where(eq(vocabularyEntry.userId, DEV_USER_ID)).catch(() => {});
// Delete plan sub-tables first (FK constraints), then plans
await db.delete(planTask).catch(() => {});
await db.delete(planAnalyzeItem).catch(() => {});
await db.delete(planRequirement).catch(() => {});
await db.delete(plan).where(eq(plan.userId, DEV_USER_ID)).catch(() => {});
await db.delete(requirementChunk).catch(() => {});
await db.delete(requirement).where(eq(requirement.userId, DEV_USER_ID)).catch(() => {});
await db.delete(useCase).where(eq(useCase.userId, DEV_USER_ID)).catch(() => {});
await db.delete(chunkFileRef).catch(() => {});
await db.delete(chunkAppliesTo).catch(() => {});
await db.delete(chunkCodebase).catch(() => {});
await db.delete(codebase).where(eq(codebase.userId, DEV_USER_ID)).catch(() => {});
// Original cleanup
await db.delete(tag).where(eq(tag.userId, DEV_USER_ID));
await db.delete(tagType).where(eq(tagType.userId, DEV_USER_ID));
await db.delete(chunk).where(eq(chunk.userId, DEV_USER_ID));

// Deterministic IDs for seed data
const ids = {
    arch: "seed-arch",
    schemaChunks: "seed-schema-chunks",
    schemaConn: "seed-schema-conn",
    schemaVer: "seed-schema-ver",
    schemaAuth: "seed-schema-auth",
    effect: "seed-effect",
    apiChunks: "seed-api-chunks",
    apiConnGraph: "seed-api-conn-graph",
    apiAI: "seed-api-ai",
    enrich: "seed-enrich",
    semantic: "seed-semantic",
    repo: "seed-repo",
    service: "seed-service",
    eden: "seed-eden",
    routes: "seed-routes",
    graph: "seed-graph",
    graphLayout: "seed-graph-layout",
    auth: "seed-auth",
    cli: "seed-cli",
    cliStore: "seed-cli-store",
    cliScanner: "seed-cli-scanner",
    env: "seed-env",
    docker: "seed-docker",
    turbo: "seed-turbo",
    // Docs codebase chunks
    docsArch: "seed-docs-arch",
    docsContent: "seed-docs-content",
    docsDeploy: "seed-docs-deploy"
};

const chunks = [
    {
        id: ids.arch,
        title: "Fubbik Architecture Overview",
        type: "document",
        content: `Fubbik is a local-first knowledge framework for humans and machines. It stores knowledge as **chunks** (atomic units) connected by **typed relationships** in a graph.

## Core Data Flow

\`\`\`
CLI / Web Frontend
  \u2193 Eden Treaty (type-safe RPC)
Elysia API (routes)
  \u2193 Effect (typed errors)
Service Layer (business logic)
  \u2193
Repository Layer (Drizzle ORM)
  \u2193
PostgreSQL + pgvector
\`\`\`

## Monorepo Layout

\`\`\`
fubbik/
\u251c\u2500\u2500 apps/
\u2502   \u251c\u2500\u2500 web/      # TanStack Start frontend
\u2502   \u251c\u2500\u2500 server/   # Elysia backend
\u2502   \u2514\u2500\u2500 cli/      # Commander.js CLI
\u251c\u2500\u2500 packages/
\u2502   \u251c\u2500\u2500 api/      # Shared routes, services, error handling
\u2502   \u251c\u2500\u2500 auth/     # Better Auth + Drizzle adapter
\u2502   \u251c\u2500\u2500 config/   # Shared TypeScript config
\u2502   \u251c\u2500\u2500 db/       # Schema, repositories, migrations
\u2502   \u2514\u2500\u2500 env/      # Elysia t schema environment validation
\`\`\`

## Key Technologies

- **Runtime:** Bun
- **Frontend:** TanStack Start, Tailwind, shadcn-ui, React Query
- **Backend:** Elysia, Effect (error handling), Drizzle ORM
- **Database:** PostgreSQL with pgvector extension
- **Auth:** Better Auth (email/password)
- **AI:** Ollama (local LLM — llama3.2 for enrichment, nomic-embed-text for embeddings)
- **Build:** Turborepo
- **Testing:** Vitest`
    },
    {
        id: ids.schemaChunks,
        title: "Database Schema: Chunks",
        type: "schema",

        content: `The \`chunk\` table is the core data model. Each chunk is an atomic unit of knowledge owned by a user.

\`\`\`sql
CREATE TABLE chunk (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  content     TEXT NOT NULL DEFAULT '',
  type        TEXT NOT NULL DEFAULT 'note',
  tags        JSONB NOT NULL DEFAULT '[]',     -- string[]
  user_id     TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  created_at  TIMESTAMP DEFAULT NOW(),
  updated_at  TIMESTAMP DEFAULT NOW(),

  -- Enrichment fields (populated by Ollama)
  summary     TEXT,                             -- 1-2 sentence TL;DR
  aliases     JSONB NOT NULL DEFAULT '[]',      -- alternative names
  not_about   JSONB NOT NULL DEFAULT '[]',      -- exclusion terms
  scope       JSONB NOT NULL DEFAULT '{}',      -- key:value metadata
  embedding   VECTOR(768)                       -- nomic-embed-text
);

CREATE INDEX chunk_userId_idx ON chunk(user_id);
CREATE INDEX chunk_type_idx ON chunk(type);
\`\`\`

## Chunk Types

| Type | Purpose |
|------|--------|
| \`note\` | General knowledge, ideas |
| \`document\` | Structured long-form content |
| \`guide\` | How-to instructions |
| \`reference\` | API docs, lookup tables |
| \`schema\` | Data models, type definitions |
| \`checklist\` | Step-by-step procedures |

## JSONB Operators Used

\`\`\`sql
tags @> '["tag"]'::jsonb           -- array contains
scope @> '{"key":"val"}'::jsonb    -- object contains
jsonb_array_elements_text(tags)     -- unnest for aggregation
\`\`\``
    },
    {
        id: ids.schemaConn,
        title: "Database Schema: Connections",
        type: "schema",
        content: `Connections are directed, typed edges between two chunks forming a knowledge graph.

\`\`\`sql
CREATE TABLE chunk_connection (
  id         TEXT PRIMARY KEY,
  source_id  TEXT NOT NULL REFERENCES chunk(id) ON DELETE CASCADE,
  target_id  TEXT NOT NULL REFERENCES chunk(id) ON DELETE CASCADE,
  relation   TEXT NOT NULL DEFAULT 'related',
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX connection_sourceId_idx ON chunk_connection(source_id);
CREATE INDEX connection_targetId_idx ON chunk_connection(target_id);
CREATE UNIQUE INDEX connection_unique_idx ON chunk_connection(source_id, target_id, relation);
\`\`\`

## Relation Types

| Relation | Meaning |
|----------|--------|
| \`related_to\` | General association |
| \`part_of\` | Hierarchical containment |
| \`depends_on\` | Functional dependency |
| \`extends\` | Builds upon / specializes |
| \`references\` | Mentions / cites |
| \`supports\` | Provides evidence |
| \`contradicts\` | Conflicts with |
| \`alternative_to\` | Different approach |

The unique index prevents duplicate connections with the same relation between the same pair.`
    },
    {
        id: ids.schemaVer,
        title: "Database Schema: Versions",
        type: "schema",
        content: `Chunk versions track edit history. A version is created every time a chunk is updated.

\`\`\`sql
CREATE TABLE chunk_version (
  id        TEXT PRIMARY KEY,
  chunk_id  TEXT NOT NULL REFERENCES chunk(id) ON DELETE CASCADE,
  version   INTEGER NOT NULL,
  title     TEXT NOT NULL,
  content   TEXT NOT NULL,
  type      TEXT NOT NULL,
  tags      JSONB NOT NULL,
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX chunk_version_chunkId_idx ON chunk_version(chunk_id);
\`\`\`

Versions are created in the chunk service before applying updates, preserving the previous state. The web UI shows version history via \`GET /api/chunks/:id/history\`.`
    },
    {
        id: ids.schemaAuth,
        title: "Database Schema: Auth Tables",
        type: "schema",
        content: `Authentication uses Better Auth with four tables managed by the Drizzle adapter.

\`\`\`sql
-- Users
CREATE TABLE "user" (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  email           TEXT NOT NULL UNIQUE,
  email_verified  BOOLEAN DEFAULT FALSE,
  image           TEXT,
  created_at      TIMESTAMP DEFAULT NOW(),
  updated_at      TIMESTAMP DEFAULT NOW()
);

-- Sessions (httpOnly cookies)
CREATE TABLE session (
  id          TEXT PRIMARY KEY,
  token       TEXT NOT NULL UNIQUE,
  expires_at  TIMESTAMP NOT NULL,
  ip_address  TEXT,
  user_agent  TEXT,
  user_id     TEXT REFERENCES "user"(id) ON DELETE CASCADE
);

-- OAuth accounts
CREATE TABLE account (
  id           TEXT PRIMARY KEY,
  account_id   TEXT NOT NULL,
  provider_id  TEXT NOT NULL,
  user_id      TEXT REFERENCES "user"(id) ON DELETE CASCADE,
  access_token, refresh_token, id_token  TEXT,
  password     TEXT  -- for email/password auth
);

-- Email verification tokens
CREATE TABLE verification (
  id          TEXT PRIMARY KEY,
  identifier  TEXT NOT NULL,
  value       TEXT NOT NULL,
  expires_at  TIMESTAMP NOT NULL
);
\`\`\`

In dev mode, a \`DEV_SESSION\` with \`userId="dev-user"\` is injected when no auth cookie is present.`
    },
    {
        id: ids.effect,
        title: "Effect Error Handling Pattern",
        type: "reference",
        content: `All backend code uses the Effect library for typed error handling. Errors flow from repositories through services to a global handler.

## Error Types

Each error is a tagged class:

\`\`\`typescript
class DatabaseError { readonly _tag = "DatabaseError"; }
class NotFoundError { readonly _tag = "NotFoundError"; resource: string; }
class AuthError     { readonly _tag = "AuthError"; }
class ValidationError { readonly _tag = "ValidationError"; }
class AiError       { readonly _tag = "AiError"; }
\`\`\`

## Layer Responsibilities

- **Repository** returns \`Effect<T, DatabaseError>\` \u2014 wraps Drizzle calls in \`Effect.tryPromise\`
- **Service** composes Effects, introduces \`NotFoundError\`, \`AuthError\`, etc.
- **Route** calls \`Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(...)))\`

## Global Error Handler

In \`packages/api/src/index.ts\`, the Elysia \`.onError\` handler extracts the Effect error from \`FiberFailure\` and maps \`_tag\` to HTTP status:

\`\`\`typescript
AuthError       \u2192 401
NotFoundError   \u2192 404
ValidationError \u2192 400
AiError         \u2192 502
DatabaseError   \u2192 500
\`\`\`

## Example Flow

\`\`\`typescript
// Repository
function getChunkById(id, userId) {
  return Effect.tryPromise({
    try: () => db.select()...where(...),
    catch: (e) => new DatabaseError({ cause: e })
  });
}

// Service
function getChunkDetail(id, userId) {
  return getChunkById(id, userId).pipe(
    Effect.flatMap(chunk =>
      chunk ? Effect.succeed(chunk) : Effect.fail(new NotFoundError({ resource: "Chunk" }))
    )
  );
}

// Route
.get("/chunks/:id", ctx =>
  Effect.runPromise(
    requireSession(ctx).pipe(
      Effect.flatMap(session => getChunkDetail(ctx.params.id, session.user.id))
    )
  )
)
\`\`\``
    },
    {
        id: ids.apiChunks,
        title: "API Endpoints: Chunks",
        type: "reference",
        content: `All chunk endpoints are defined in \`packages/api/src/chunks/routes.ts\`.

## List Chunks
\`GET /api/chunks\`

Query params:
- \`search\` \u2014 fulltext + trigram similarity search
- \`type\` \u2014 filter by chunk type
- \`tags\` \u2014 comma-separated, JSONB containment
- \`sort\` \u2014 \`newest\` | \`oldest\` | \`alpha\` | \`updated\`
- \`limit\`, \`offset\` \u2014 pagination
- \`scope\` \u2014 comma-separated key:value pairs
- \`exclude\` \u2014 terms NOT in notAbout array
- \`alias\` \u2014 search in aliases array
- \`after\` \u2014 updated within N days
- \`enrichment\` \u2014 \`missing\` | \`complete\`
- \`minConnections\` \u2014 minimum connection count

## CRUD
- \`POST /api/chunks\` \u2014 Create chunk (triggers async enrichment)
- \`GET /api/chunks/:id\` \u2014 Get chunk with connections
- \`PATCH /api/chunks/:id\` \u2014 Update (creates version, re-enriches)
- \`DELETE /api/chunks/:id\` \u2014 Delete chunk

## Bulk Operations
- \`POST /api/chunks/import\` \u2014 Import up to 500 chunks
- \`DELETE /api/chunks/bulk\` \u2014 Delete up to 100 chunks by IDs
- \`GET /api/chunks/export\` \u2014 Export all chunks as JSON

## Search
- \`GET /api/chunks/search/semantic\` \u2014 Vector similarity search (\`q\`, \`limit\`, \`exclude\`, \`scope\`)

## History
- \`GET /api/chunks/:id/history\` \u2014 Version history

## Validation

All inputs validated with Elysia's \`t.Object()\` (TypeBox):
- \`title\`: max 200 chars
- \`content\`: max 50,000 chars
- \`type\`: max 20 chars
- \`tags\`: max 20 items, each max 50 chars`
    },
    {
        id: ids.apiConnGraph,
        title: "API Endpoints: Connections, Graph & Stats",
        type: "reference",
        content: `## Connections

Defined in \`packages/api/src/connections/routes.ts\`.

- \`POST /api/connections\` \u2014 Create connection
  - Body: \`{ sourceId, targetId, relation }\`
  - Validates both chunks exist and belong to the user
  - Prevents self-connections (sourceId \u2260 targetId)
- \`DELETE /api/connections/:id\` \u2014 Delete connection

## Graph

Defined in \`packages/api/src/graph/routes.ts\`.

- \`GET /api/graph\` \u2014 Get full knowledge graph
  - Returns \`{ chunks: [...], connections: [...] }\`
  - Chunks include: id, title, type, tags, summary, createdAt
  - Used by the graph visualization frontend

## Stats

- \`GET /api/stats\` \u2014 Returns \`{ chunks, connections, tags }\` counts
- \`GET /api/tags\` \u2014 Tag list with frequency counts (uses \`jsonb_array_elements_text\`)

## Health

- \`GET /api/health\` \u2014 Returns \`{ status: "ok", db: true|false }\`
  - Returns 503 if database connection fails

## Auth

- \`ALL /api/auth/*\` \u2014 Proxied to Better Auth handler
  - Sign up, sign in, sign out, session management`
    },
    {
        id: ids.apiAI,
        title: "API Endpoints: AI & Enrichment",
        type: "reference",
        content: `## Ollama AI Integration

Fubbik uses Ollama exclusively for all AI features — no external AI APIs required.

Defined in \`packages/api/src/enrich/routes.ts\`. Requires Ollama running locally.

- \`POST /api/chunks/:id/enrich\` — Enrich a single chunk
- \`POST /api/chunks/enrich-all\` — Enrich all user's chunks sequentially

Enrichment generates:
1. **Metadata** (via \`llama3.2\`): summary, aliases[], notAbout[]
2. **Embedding** (via \`nomic-embed-text\`): 768-dim vector for semantic search

Enrichment also fires automatically (async, fire-and-forget) when a chunk is created or updated with title/content changes.

## Required Models

\`\`\`bash
ollama pull llama3.2           # metadata generation
ollama pull nomic-embed-text   # 768-dim embeddings
\`\`\`

Without Ollama, all other features work normally. There is no OpenAI or external AI SDK dependency.`
    },
    {
        id: ids.enrich,
        title: "Enrichment Pipeline",
        type: "reference",
        content: `The enrichment pipeline generates AI metadata for chunks using Ollama (local LLM).

## Flow

\`\`\`
Chunk created/updated
  \u2193 (async, fire-and-forget)
enrichChunkIfEmpty(chunkId)
  \u2193 (only if no summary exists)
enrichChunk(chunkId)
  \u2193
1. Check Ollama availability (GET /api/tags, 2s timeout)
2. Generate metadata via llama3.2:
   - summary: 1-2 sentence TL;DR
   - aliases: 3-8 alternative names
   - notAbout: 2-5 exclusion terms
3. Generate embedding via nomic-embed-text:
   - Input: "search_document: {title}. {summary}. {content}"
   - Output: 768-dimensional vector
4. updateChunkEnrichment() \u2192 store all fields
\`\`\`

## Ollama Client

Defined in \`packages/api/src/ollama/client.ts\`.

\`\`\`typescript
isOllamaAvailable()       // Ping with 2s timeout
generateJson<T>(prompt)   // POST /api/generate, format: "json"
generateEmbedding(text)   // POST /api/embeddings
\`\`\`

## Embedding Prefixes

Nomic-embed-text uses task-specific prefixes:
- **Documents:** \`"search_document: "\` + title + summary + content
- **Queries:** \`"search_query: "\` + query text

## Required Models

\`\`\`bash
ollama pull nomic-embed-text   # embeddings
ollama pull llama3.2           # metadata generation
\`\`\`

Without Ollama, all other features work normally. Enrichment fails gracefully.`
    },
    {
        id: ids.semantic,
        title: "Semantic Search",
        type: "reference",
        content: `Semantic search uses vector embeddings to find chunks by meaning rather than keywords.

## How It Works

\`\`\`
User query: "how do I deploy?"
  \u2193
generateQueryEmbedding("how do I deploy?")
  \u2193 Ollama nomic-embed-text
  \u2193 prefix: "search_query: "
768-dim vector
  \u2193
PostgreSQL: SELECT ... ORDER BY embedding <=> query_vector
  \u2193
Top K results with similarity score (1 - cosine distance)
\`\`\`

## PostgreSQL Query

\`\`\`sql
SELECT
  id, title, type,
  1 - (embedding <=> $query_vector) AS similarity
FROM chunk
WHERE user_id = $userId
  AND embedding IS NOT NULL
ORDER BY embedding <=> $query_vector
LIMIT $limit;
\`\`\`

The \`<=>\` operator computes cosine distance using the pgvector extension.

## Filters

Semantic search also supports:
- \`scope\` \u2014 JSONB containment filter
- \`exclude\` \u2014 NOT in notAbout array

## API

\`\`\`
GET /api/chunks/search/semantic?q=deployment&limit=10
\`\`\`

## CLI

\`\`\`bash
fubbik search --semantic "how to deploy"
\`\`\`

Chunks must be enriched (have embeddings) to appear in semantic search results.`
    },
    {
        id: ids.repo,
        title: "Repository Layer",
        type: "reference",
        content: `Repositories provide pure data access with no business logic. All functions return \`Effect<T, DatabaseError>\`.

Defined in \`packages/db/src/repository/\`.

## chunk.ts

\`\`\`typescript
listChunks(params)              // WHERE builder + pagination \u2192 { chunks, total }
getChunkById(chunkId, userId?)  // Single chunk lookup
getChunkConnections(chunkId)    // LEFT JOIN for connected chunks
createChunk(params)             // INSERT RETURNING
updateChunk(chunkId, params)    // Partial UPDATE
updateChunkEnrichment(...)      // Only enrichment fields
deleteChunk(chunkId, userId)    // DELETE with ownership check
deleteMany(ids[], userId)       // Bulk DELETE
exportAllChunks(userId?)        // All chunks ordered by createdAt
\`\`\`

## semantic.ts

\`\`\`typescript
semanticSearch({ embedding, userId?, exclude?, scope?, limit })
  // Uses embedding <=> operator for cosine distance
\`\`\`

## connection.ts

\`\`\`typescript
createConnection({ id, sourceId, targetId, relation })
deleteConnection(connectionId)
getConnectionById(connectionId)
\`\`\`

## graph.ts

\`\`\`typescript
getAllChunksMeta(userId?)          // id, title, type, tags, summary, createdAt
getAllConnectionsForUser(userId?)  // All connections for user's chunks
\`\`\`

## stats.ts & tags.ts

\`\`\`typescript
getChunkCount(userId?)
getConnectionCount(userId?)
getTagCount(userId?)               // Uses raw SQL with jsonb_array_elements_text
getTagsWithCounts(userId?)         // SELECT tag, COUNT(*) GROUP BY tag
\`\`\``
    },
    {
        id: ids.service,
        title: "Service Layer",
        type: "reference",
        content: `Services compose repository Effects and add business logic. Defined in \`packages/api/src/*/service.ts\`.

## Chunk Service

\`\`\`typescript
listChunks(userId?, query)     // Delegates to repo with parsed filters
getChunkDetail(chunkId, userId)// Chunk + connections, NotFoundError if missing
createChunk(userId, body)      // Create + fire-and-forget enrichment
updateChunk(chunkId, userId, body) // Create version \u2192 update \u2192 re-enrich
deleteChunk(chunkId, userId)   // Ownership check + delete
deleteMany(ids, userId)        // Bulk delete with ownership
semanticSearch(userId?, query) // Generate embedding \u2192 vector search
exportChunks(userId?)          // All chunks as JSON
importChunks(userId, chunks[]) // Batch create (concurrency: 10)
\`\`\`

Key behavior: \`createChunk\` and \`updateChunk\` trigger \`enrichChunkIfEmpty\` or \`enrichChunk\` asynchronously. The enrichment is fire-and-forget \u2014 it doesn't block the API response.

## Connection Service

\`\`\`typescript
createConnection(userId, { sourceId, targetId, relation })
  // Validates: sourceId \u2260 targetId
  // Checks both chunks exist and belong to user
  // Creates with UUID

deleteConnection(connectionId, userId)
  // Checks connection exists
  // Validates both endpoint chunks belong to user
\`\`\`

## Graph Service

\`\`\`typescript
getUserGraph(userId?)
  // Returns { chunks: getAllChunksMeta(), connections: getAllConnectionsForUser() }
\`\`\``
    },
    {
        id: ids.eden,
        title: "Eden Treaty API Client",
        type: "reference",
        content: `The frontend communicates with the backend using Eden Treaty \u2014 a type-safe RPC-like client generated from Elysia's route definitions.

## Setup

\`\`\`typescript
// apps/web/src/utils/api.ts
import { treaty } from '@elysiajs/eden';
import type { Api } from '@fubbik/api';

export const api = treaty<Api>(serverUrl, {
  fetch: { credentials: 'include' }  // send auth cookies
});
\`\`\`

## Usage

\`\`\`typescript
// Full type safety \u2014 params, body, and response are all typed
const { data, error } = await api.api.chunks.get({ query: { type: 'guide' } });
const { data } = await api.api.chunks({ id: chunkId }).get();
const { data } = await api.api.chunks.post({ title: '...', content: '...' });
const { data } = await api.api.chunks({ id }).patch({ title: 'New Title' });
\`\`\`

## Error Handling

\`\`\`typescript
// unwrapEden throws if error is present
import { unwrapEden } from '@/utils/eden';
const data = unwrapEden(await api.api.chunks.get());
\`\`\`

The \`Api\` type is exported from \`packages/api/src/index.ts\` and includes all routes. Eden generates client methods that mirror the server's URL structure with full TypeScript inference.`
    },
    {
        id: ids.routes,
        title: "Frontend Route Structure",
        type: "reference",
        content: `The web app uses TanStack Start with file-based routing.

## Routes

| Route | File | Purpose |
|-------|------|---------|
| \`/\` | \`index.tsx\` | Landing page |
| \`/login\` | \`login.tsx\` | Sign in / sign up forms |
| \`/dashboard\` | \`dashboard.tsx\` | Stats, export/import, favorites |
| \`/chunks\` | \`chunks.index.tsx\` | List view with filters, sorting, kanban |
| \`/chunks/new\` | \`chunks.new.tsx\` | Create chunk form |
| \`/chunks/:id\` | \`chunks.$chunkId.tsx\` | View chunk detail + connections |
| \`/chunks/:id/edit\` | \`chunks.$chunkId_.edit.tsx\` | Edit chunk |
| \`/graph\` | \`graph.tsx\` | Knowledge graph visualization |
| \`/tags\` | \`tags.tsx\` | Tag management |

## Feature Modules

\`\`\`
features/
\u251c\u2500\u2500 auth/        # Sign-in/up forms, user menu
\u251c\u2500\u2500 chunks/      # Split dialog, link dialog, AI section,
\u2502                # version history, kanban view, templates
\u251c\u2500\u2500 graph/       # Graph view, nodes, edges, layouts,
\u2502                # metrics, timeline, filters, legend
\u251c\u2500\u2500 search/      # Semantic + full-text search
\u251c\u2500\u2500 editor/      # Markdown editor
\u251c\u2500\u2500 nav/         # Mobile navigation
\u2514\u2500\u2500 dashboard/   # Dashboard components
\`\`\`

## State Management

- **Server state:** TanStack Query (React Query) for API data
- **Local state:** TanStack Store for UI preferences
- **Hooks:** \`usePinnedChunks\`, \`useRecentChunks\`, \`useFavorites\`, \`useCollections\`, \`useSavedFilters\``
    },
    {
        id: ids.graph,
        title: "Graph Visualization",
        type: "document",
        content: `The graph view renders the knowledge base as an interactive node-edge diagram using \`@xyflow/react\` (React Flow).

## Architecture

\`\`\`
Graph View (graph-view.tsx)
\u251c\u2500\u2500 Layout Worker (layout.worker.ts)    \u2014 force simulation in Web Worker
\u251c\u2500\u2500 Quadtree (quadtree.ts)              \u2014 Barnes-Hut N-body repulsion
\u251c\u2500\u2500 Layouts (layouts.ts)                 \u2014 hierarchical & radial alternatives
\u251c\u2500\u2500 Graph Node (graph-node.tsx)          \u2014 custom node renderer
\u251c\u2500\u2500 Floating Edge (floating-edge.tsx)    \u2014 curved edge renderer
\u251c\u2500\u2500 Graph Filters (graph-filters.tsx)    \u2014 type/relation filter panel
\u251c\u2500\u2500 Graph Metrics (graph-metrics.tsx)    \u2014 density, hubs, stats overlay
\u251c\u2500\u2500 Graph Timeline (graph-timeline.tsx)  \u2014 date-based animation
\u251c\u2500\u2500 Graph Legend (graph-legend.tsx)       \u2014 color legend
\u2514\u2500\u2500 Graph Detail Panel                   \u2014 side panel for selected chunk
\`\`\`

## Features

- **Semantic zoom:** Compact/normal/detailed views with hysteresis
- **Node sizing:** Nodes scale by connection count
- **Layout algorithms:** Force-directed, hierarchical, radial
- **Path highlighting:** BFS shortest path between two nodes
- **Progressive explore:** Expand graph incrementally from a starting node
- **Edge creation:** Connect nodes directly in the graph
- **Multi-select:** Shift+click for bulk operations
- **Edge bundling:** Collapse parallel edges by type cluster
- **Timeline:** Date slider with auto-play animation
- **Saved views:** Persist filter/layout configurations
- **Keyboard shortcuts:** \`?\` for help overlay
- **Export:** Download graph as PNG

## Data Flow

\`\`\`
GET /api/graph \u2192 { chunks, connections }
  \u2193
filteredGraph useMemo (type/relation/timeline/explore filters)
  \u2193
Web Worker (force simulation with Barnes-Hut quadtree)
  \u2193
layoutPositions state
  \u2193
layoutNodes/layoutEdges useMemo (styling, parallel detection, bundling)
  \u2193
Consolidated styling useEffect (search/focus/selection/path/multi-select)
  \u2193
ReactFlow renders nodes + edges
\`\`\``
    },
    {
        id: ids.graphLayout,
        title: "Graph Layout Algorithms",
        type: "reference",
        content: `Three layout algorithms are available, selectable via dropdown in the graph toolbar.

## Force-Directed (default)

Runs in a Web Worker (\`layout.worker.ts\`) using Barnes-Hut quadtree for O(n log n) repulsion.

**Parameters:**
- Repulsion: 160,000 (strong push between nodes)
- Spring K: 0.002 (attraction along edges)
- Center gravity: 0.003 (gentle pull toward origin)
- Cluster K: 0.001 (type-based clustering)
- Damping: 0.85, Iterations: 200

**Spring lengths vary by relation:**
- \`part_of\`: 300px (tight)
- \`depends_on\` / \`extends\`: 380px
- \`references\` / \`supports\`: 450px
- \`related_to\`: 520px
- \`contradicts\` / \`alternative_to\`: 560px (loose)

## Hierarchical

Tree layout from \`part_of\` and \`depends_on\` edges. BFS from root nodes, orphans placed at bottom. Defined in \`layouts.ts\`.

## Radial

Concentric rings from a center node (selected or most-connected). BFS determines ring placement. Defined in \`layouts.ts\`.

## Dragged Positions

Manually dragged nodes persist their positions across layout recalculations via \`draggedPositions\` state.`
    },
    {
        id: ids.auth,
        title: "Authentication System",
        type: "document",
        content: `Fubbik uses Better Auth for email/password authentication with a Drizzle adapter for PostgreSQL.

## Configuration

Defined in \`packages/auth/src/index.ts\`:

\`\`\`typescript
export const auth = betterAuth({
  database: drizzleAdapter(db, { provider: 'pg', schema }),
  emailAndPassword: { enabled: true },
  trustedOrigins: [env.CORS_ORIGIN],
  advanced: {
    cookiePrefix: 'fubbik',
    defaultCookieAttributes: {
      httpOnly: true,
      secure: true,
      sameSite: 'none'
    }
  }
});
\`\`\`

## Session Flow

1. User signs up/in via \`/api/auth/*\` endpoints
2. Better Auth sets httpOnly cookie with session token
3. Each API request: \`requireSession(ctx)\` extracts session from cookie
4. Returns \`Effect<Session, AuthError>\` \u2014 401 if no valid session

## Dev Mode

In development, if no session cookie is present, a \`DEV_SESSION\` is injected:
- \`userId: "dev-user"\`
- Allows local testing without sign-up

## Session Type

\`\`\`typescript
type Session = {
  session: { id, token, userId, expiresAt, ... };
  user: { id, name, email, emailVerified, image, ... };
};
\`\`\``
    },
    {
        id: ids.cli,
        title: "CLI Commands",
        type: "reference",
        content: `The Fubbik CLI (\`apps/cli\`) provides local-first knowledge management with optional server sync.

## Setup
\`\`\`bash
fubbik init [name]           # Initialize .fubbik/store.json
fubbik init --scan           # Auto-generate chunks from project
fubbik init --scan --push    # Scan + push to server
fubbik health                # Check server connectivity
\`\`\`

## Chunk CRUD
\`\`\`bash
fubbik add -t "Title" -c "Content" --type guide --tags "a,b"
fubbik add -t "Title" --content-file README.md
fubbik get <id>              # Get chunk details
fubbik cat <id>              # Output raw content (pipe-friendly)
fubbik update <id> --title "New" --tags "x,y"
fubbik remove <id>           # Delete chunk
\`\`\`

## Search & Browse
\`\`\`bash
fubbik list                  # List all chunks
fubbik list --type guide     # Filter by type
fubbik search "query"        # Full-text search
fubbik search --semantic "meaning-based query"  # Vector search (requires server)
fubbik tags                  # List tags with counts
fubbik stats                 # Chunk/connection/tag counts
\`\`\`

## Connections
\`\`\`bash
fubbik link <source> <target> -r part_of
fubbik unlink <connection-id>
\`\`\`

## Import / Export
\`\`\`bash
fubbik export > backup.json
fubbik import --file chunks.json
fubbik import --file docs/        # Import markdown directory
fubbik bulk-add --file chunks.jsonl
\`\`\`

## Sync
\`\`\`bash
fubbik sync --url http://localhost:3000
fubbik sync --push           # Local \u2192 server only
fubbik sync --pull           # Server \u2192 local only
\`\`\`

## Output Modes
\`\`\`bash
fubbik list --json           # Machine-readable JSON
fubbik get <id> -q           # Quiet mode (just ID)
\`\`\``
    },
    {
        id: ids.cliStore,
        title: "CLI Local Store",
        type: "reference",
        content: `The CLI uses a JSON file at \`.fubbik/store.json\` for local-first storage.

## Structure

\`\`\`typescript
interface Store {
  name: string;             // Knowledge base name
  chunks: Chunk[];          // All local chunks
  serverUrl?: string;       // Saved server URL
  lastSync?: string;        // ISO timestamp of last sync
}

interface Chunk {
  id: string;               // Format: c-{nanoid}
  serverId?: string;        // Server-side ID after sync
  title: string;
  content: string;
  type: string;
  tags: string[];
  createdAt: string;        // ISO timestamp
  updatedAt: string;
}
\`\`\`

## Operations

All local operations are synchronous JSON read/write:
- \`readStore()\` \u2014 Parse \`.fubbik/store.json\`
- \`writeStore(store)\` \u2014 Write back
- \`addChunk(data)\` \u2014 Generate ID, append, write
- \`searchChunks(query)\` \u2014 Filter by title/content/tags match

## Sync Strategy

Sync compares local and server chunks **by title**:
- Local-only chunks \u2192 pushed to server via \`POST /api/chunks/import\`
- Server-only chunks \u2192 pulled to local store
- Conflicts (same title, different content) \u2192 preserved on both sides

The server URL is saved after first sync for future use.`
    },
    {
        id: ids.cliScanner,
        title: "CLI Project Scanner",
        type: "reference",
        content: `The project scanner (\`apps/cli/src/lib/scanner.ts\`) auto-generates chunks from a codebase.

## What Gets Scanned

1. **Root documentation:** README.md, CLAUDE.md, CONTRIBUTING.md, Agents.md, CHANGELOG.md
   - Type: \`guide\`, Tags: \`["documentation", "project"]\`

2. **docs/ directory:** All \`.md\` files recursively
   - Type: \`guide\`, Tags: \`["documentation", ...pathSegments]\`

3. **Other markdown files:** \`.md\` files throughout the project
   - Skips root docs and docs/ (already handled)

## Auto-Split

Large files (>5000 chars or >~150 lines) are automatically split:
1. An **index chunk** is created listing all sections
2. **Sub-chunks** are created per H1/H2/H3 heading
3. Sub-chunks get a \`parentTitle\` linking back to the index

## Ignored Directories

\`node_modules\`, \`.git\`, \`.turbo\`, \`dist\`, \`build\`, \`.next\`, \`.output\`, \`.cache\`, \`coverage\`, \`.fubbik\`

## Usage

\`\`\`bash
fubbik init --scan --dry-run   # Preview what would be generated
fubbik init --scan             # Generate and store locally
fubbik init --scan --push      # Generate + push to server
\`\`\`

The scanner extracts markdown titles from \`# Heading\` lines and generates path-based tags from directory segments.`
    },
    {
        id: ids.env,
        title: "Environment Variables",
        type: "reference",
        content: `Environment validation uses Elysia \`t\` schema (TypeBox). Defined in \`packages/env/src/\`.

## Server Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| \`DATABASE_URL\` | Yes | \u2014 | PostgreSQL connection string |
| \`BETTER_AUTH_SECRET\` | Yes | \u2014 | Auth secret (32+ chars) |
| \`BETTER_AUTH_URL\` | Yes | \u2014 | Auth base URL |
| \`CORS_ORIGIN\` | Yes | \u2014 | Allowed frontend origin |
| \`PORT\` | Yes | \`3000\` | Server port |
| \`NODE_ENV\` | No | \`development\` | Environment |
| \`OLLAMA_URL\` | No | \`http://localhost:11434\` | Ollama server |
| \`RATE_LIMIT_MAX\` | No | \u2014 | Max requests per window |
| \`RATE_LIMIT_DURATION_MS\` | No | \u2014 | Rate limit window |

## Web Variables

| Variable | Required | Description |
|----------|----------|-------------|
| \`VITE_SERVER_URL\` | Yes | Backend API URL |

## Docker Compose Defaults

\`\`\`env
POSTGRES_DB=fubbik
POSTGRES_USER=postgres
POSTGRES_PASSWORD=password
DATABASE_URL=postgresql://postgres:password@db:5432/fubbik
\`\`\``
    },
    {
        id: ids.docker,
        title: "Docker Deployment",
        type: "reference",
        content: `Fubbik can be deployed via Docker Compose with three services.

## Services

| Service | Image | Port | Purpose |
|---------|-------|------|---------|
| \`web\` | Node 20+ | 3001 | TanStack Start frontend |
| \`server\` | Node 20+ | 3000 | Elysia API server |
| \`db\` | postgres:16-alpine | 5432 | PostgreSQL database |

## docker-compose.yml

\`\`\`yaml
services:
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: fubbik
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: pg_isready -U postgres

  server:
    depends_on: [db]
    environment:
      DATABASE_URL: postgresql://postgres:password@db:5432/fubbik
      BETTER_AUTH_SECRET: <32+ chars>
    healthcheck:
      test: curl -f http://localhost:3000/api/health

  web:
    depends_on: [server]
    environment:
      VITE_SERVER_URL: http://server:3000
\`\`\`

## Development

\`\`\`bash
pnpm install
pnpm dev          # Start all services via Turborepo
pnpm db:push      # Push schema to PostgreSQL
pnpm db:studio    # Open Drizzle Studio (port 5555)
\`\`\`

## CI Pipeline

\`\`\`bash
pnpm ci           # type-check \u2192 lint \u2192 test \u2192 build \u2192 format-check \u2192 sherif
\`\`\``
    },
    {
        id: ids.turbo,
        title: "Turborepo Build System",
        type: "reference",
        content: `Fubbik uses Turborepo for monorepo task orchestration.

## Task Pipeline

\`\`\`json
{
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "test": {
      "dependsOn": ["^build"],
      "cache": false
    },
    "db:push": { "cache": false },
    "db:start": { "persistent": true },
    "db:watch": { "persistent": true }
  }
}
\`\`\`

## Key Scripts

| Script | Purpose |
|--------|---------|
| \`pnpm dev\` | Start all apps in dev mode |
| \`pnpm dev:web\` | Frontend only |
| \`pnpm dev:server\` | Backend only |
| \`pnpm build\` | Production build (respects \`^build\` deps) |
| \`pnpm test\` | Run Vitest across all packages |
| \`pnpm ci\` | Full CI: type-check, lint, test, build, format, sherif |
| \`pnpm db:push\` | Push Drizzle schema |
| \`pnpm db:studio\` | Drizzle Studio UI |

## Package Dependencies

\`\`\`
apps/web      \u2192 @fubbik/api (types), @fubbik/env
apps/server   \u2192 @fubbik/api, @fubbik/auth, @fubbik/db, @fubbik/env
apps/cli      \u2192 @fubbik/api (chunk-size)
packages/api  \u2192 @fubbik/db, @fubbik/auth, @fubbik/env
packages/auth \u2192 @fubbik/db, @fubbik/env
packages/db   \u2192 @fubbik/env
\`\`\``
    },
    // ── Docs codebase chunks ────────────────────────────────────────
    {
        id: ids.docsArch,
        title: "Documentation Site Architecture",
        type: "document",
        content: `The fubbik-docs site is a static documentation site built with Astro and deployed to Vercel. It pulls content from markdown files co-located in the docs/ directory and auto-generates API reference pages from the OpenAPI spec exported by the Elysia server. The site uses a custom Astro integration to resolve cross-references between pages and inject frontmatter metadata from the knowledge base. Search is powered by Pagefind, which indexes the static output at build time for instant client-side full-text search.`
    },
    {
        id: ids.docsContent,
        title: "Content Guidelines",
        type: "reference",
        content: `All documentation pages follow a consistent structure: a title, a one-sentence summary, a "Prerequisites" section listing required knowledge, and the main body. Code examples must be runnable — every snippet is extracted and executed in CI via a custom test harness. Tone should be concise and direct, avoiding marketing language. Diagrams use Mermaid syntax embedded in fenced code blocks. Each page must declare its audience (beginner, intermediate, advanced) in frontmatter so the site can surface appropriate content based on reader context.`
    },
    {
        id: ids.docsDeploy,
        title: "Deployment Process",
        type: "reference",
        content: `The docs site deploys automatically on every push to the main branch via a Vercel project linked to the fubbik-docs repository. Preview deployments are created for every pull request, allowing reviewers to check rendered output before merging. The build step runs Astro's static build, then Pagefind indexing, and finally a link-checker that fails the build if any internal links are broken. Environment variables for the Vercel project include FUBBIK_API_URL (pointing to the production API for live OpenAPI spec fetching) and SITE_URL for canonical URL generation.`
    }
];

for (const c of chunks) {
    await db
        .insert(chunk)
        .values({
            ...c,
            userId: DEV_USER_ID
        })
        .catch(e => console.error(`  \u2717 ${c.title}:`, e));
    console.log(`  \u2713 ${c.title}`);
}

// Add connections (36 typed relationships)
const connections = [
    // Architecture overview connects to major subsystems
    { id: "conn-01", sourceId: ids.arch, targetId: ids.schemaChunks, relation: "part_of" },
    { id: "conn-02", sourceId: ids.arch, targetId: ids.schemaConn, relation: "part_of" },
    { id: "conn-03", sourceId: ids.arch, targetId: ids.schemaAuth, relation: "part_of" },
    { id: "conn-04", sourceId: ids.arch, targetId: ids.effect, relation: "part_of" },
    { id: "conn-05", sourceId: ids.arch, targetId: ids.auth, relation: "part_of" },
    { id: "conn-06", sourceId: ids.arch, targetId: ids.graph, relation: "part_of" },
    { id: "conn-07", sourceId: ids.arch, targetId: ids.cli, relation: "part_of" },

    // Schema dependencies
    { id: "conn-08", sourceId: ids.schemaConn, targetId: ids.schemaChunks, relation: "depends_on" },
    { id: "conn-09", sourceId: ids.schemaVer, targetId: ids.schemaChunks, relation: "depends_on" },
    { id: "conn-10", sourceId: ids.schemaAuth, targetId: ids.auth, relation: "references" },

    // API layer depends on service
    { id: "conn-11", sourceId: ids.apiChunks, targetId: ids.service, relation: "depends_on" },
    { id: "conn-12", sourceId: ids.apiConnGraph, targetId: ids.service, relation: "depends_on" },
    { id: "conn-13", sourceId: ids.apiAI, targetId: ids.service, relation: "depends_on" },

    // Service depends on repository
    { id: "conn-14", sourceId: ids.service, targetId: ids.repo, relation: "depends_on" },
    { id: "conn-15", sourceId: ids.repo, targetId: ids.schemaChunks, relation: "depends_on" },
    { id: "conn-16", sourceId: ids.repo, targetId: ids.schemaConn, relation: "depends_on" },

    // Effect pattern usage
    { id: "conn-17", sourceId: ids.service, targetId: ids.effect, relation: "references" },
    { id: "conn-18", sourceId: ids.repo, targetId: ids.effect, relation: "references" },

    // Enrichment & semantic search
    { id: "conn-19", sourceId: ids.enrich, targetId: ids.semantic, relation: "supports" },
    { id: "conn-20", sourceId: ids.enrich, targetId: ids.schemaChunks, relation: "references" },
    { id: "conn-21", sourceId: ids.semantic, targetId: ids.schemaChunks, relation: "depends_on" },
    { id: "conn-22", sourceId: ids.apiAI, targetId: ids.enrich, relation: "references" },

    // Frontend dependencies
    { id: "conn-23", sourceId: ids.eden, targetId: ids.apiChunks, relation: "depends_on" },
    { id: "conn-24", sourceId: ids.eden, targetId: ids.apiConnGraph, relation: "depends_on" },
    { id: "conn-25", sourceId: ids.routes, targetId: ids.eden, relation: "depends_on" },

    // Graph visualization
    { id: "conn-26", sourceId: ids.graph, targetId: ids.graphLayout, relation: "part_of" },
    { id: "conn-27", sourceId: ids.graph, targetId: ids.apiConnGraph, relation: "depends_on" },
    { id: "conn-28", sourceId: ids.routes, targetId: ids.graph, relation: "references" },
    { id: "conn-29", sourceId: ids.routes, targetId: ids.auth, relation: "references" },

    // CLI structure
    { id: "conn-30", sourceId: ids.cli, targetId: ids.cliStore, relation: "part_of" },
    { id: "conn-31", sourceId: ids.cli, targetId: ids.cliScanner, relation: "part_of" },
    { id: "conn-32", sourceId: ids.cliStore, targetId: ids.apiChunks, relation: "references" },
    { id: "conn-33", sourceId: ids.cliScanner, targetId: ids.schemaChunks, relation: "references" },

    // Infrastructure
    { id: "conn-34", sourceId: ids.docker, targetId: ids.env, relation: "references" },
    { id: "conn-35", sourceId: ids.turbo, targetId: ids.docker, relation: "related_to" },
    { id: "conn-36", sourceId: ids.env, targetId: ids.auth, relation: "references" },

    // Cross-codebase: docs deployment references fubbik's docker setup
    { id: "conn-37", sourceId: ids.docsDeploy, targetId: ids.docker, relation: "references" }
];

for (const conn of connections) {
    await db
        .insert(chunkConnection)
        .values(conn)
        .catch(e => console.error(`  \u2717 ${conn.id}:`, e));
}
console.log(`  \u2713 ${connections.length} connections`);

// Seed tag types
const tagTypeIds = {
    feature: "seed-tt-feature",
    techstack: "seed-tt-techstack",
    infrastructure: "seed-tt-infrastructure",
    pattern: "seed-tt-pattern",
    documentation: "seed-tt-documentation"
};

const seedTagTypes = [
    { id: tagTypeIds.feature, name: "feature", color: "#3b82f6", userId: DEV_USER_ID },
    { id: tagTypeIds.techstack, name: "techstack", color: "#10b981", userId: DEV_USER_ID },
    { id: tagTypeIds.infrastructure, name: "infrastructure", color: "#f59e0b", userId: DEV_USER_ID },
    { id: tagTypeIds.pattern, name: "pattern", color: "#8b5cf6", userId: DEV_USER_ID },
    { id: tagTypeIds.documentation, name: "documentation", color: "#ef4444", userId: DEV_USER_ID }
];

for (const tt of seedTagTypes) {
    await db.insert(tagType).values(tt).catch(e => console.error(`  \u2717 Tag type ${tt.name}:`, e));
}
console.log(`  \u2713 ${seedTagTypes.length} tag types`);

// Seed tags (each assigned to a tag type)
const seedTags: { id: string; name: string; tagTypeId: string; userId: string }[] = [
    // feature tags
    { id: "seed-tag-authentication", name: "authentication", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-search", name: "search", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-enrichment", name: "enrichment", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-graph", name: "graph", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-visualization", name: "visualization", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-cli", name: "cli", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-deployment", name: "deployment", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-scanner", name: "scanner", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-storage", name: "storage", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    { id: "seed-tag-sync", name: "sync", tagTypeId: tagTypeIds.feature, userId: DEV_USER_ID },
    // techstack tags
    { id: "seed-tag-postgresql", name: "postgresql", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-drizzle", name: "drizzle", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-effect", name: "effect", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-elysia", name: "elysia", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-eden", name: "eden", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-tanstack", name: "tanstack", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-react", name: "react", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-reactflow", name: "reactflow", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-ollama", name: "ollama", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-pgvector", name: "pgvector", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-better-auth", name: "better-auth", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-docker", name: "docker", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-turborepo", name: "turborepo", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    { id: "seed-tag-typescript", name: "typescript", tagTypeId: tagTypeIds.techstack, userId: DEV_USER_ID },
    // infrastructure tags
    { id: "seed-tag-database", name: "database", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    { id: "seed-tag-api", name: "api", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    { id: "seed-tag-frontend", name: "frontend", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    { id: "seed-tag-backend", name: "backend", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    { id: "seed-tag-monorepo", name: "monorepo", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    { id: "seed-tag-configuration", name: "configuration", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    { id: "seed-tag-ci", name: "ci", tagTypeId: tagTypeIds.infrastructure, userId: DEV_USER_ID },
    // pattern tags
    { id: "seed-tag-repository", name: "repository", tagTypeId: tagTypeIds.pattern, userId: DEV_USER_ID },
    { id: "seed-tag-service", name: "service", tagTypeId: tagTypeIds.pattern, userId: DEV_USER_ID },
    { id: "seed-tag-error-handling", name: "error-handling", tagTypeId: tagTypeIds.pattern, userId: DEV_USER_ID },
    { id: "seed-tag-force-directed", name: "force-directed", tagTypeId: tagTypeIds.pattern, userId: DEV_USER_ID },
    { id: "seed-tag-rest", name: "rest", tagTypeId: tagTypeIds.pattern, userId: DEV_USER_ID },
    // documentation tags
    { id: "seed-tag-overview", name: "overview", tagTypeId: tagTypeIds.documentation, userId: DEV_USER_ID },
    { id: "seed-tag-schema", name: "schema", tagTypeId: tagTypeIds.documentation, userId: DEV_USER_ID },
    { id: "seed-tag-guide", name: "guide", tagTypeId: tagTypeIds.documentation, userId: DEV_USER_ID },
    { id: "seed-tag-reference", name: "reference", tagTypeId: tagTypeIds.documentation, userId: DEV_USER_ID }
];

for (const t of seedTags) {
    await db.insert(tag).values(t).catch(e => console.error(`  \u2717 Tag ${t.name}:`, e));
}
console.log(`  \u2713 ${seedTags.length} tags`);

// Chunk-to-tag associations
const chunkTagAssociations: { chunkId: string; tagId: string }[] = [
    // Architecture Overview
    { chunkId: ids.arch, tagId: "seed-tag-overview" },
    { chunkId: ids.arch, tagId: "seed-tag-monorepo" },
    // Schema: Chunks
    { chunkId: ids.schemaChunks, tagId: "seed-tag-database" },
    { chunkId: ids.schemaChunks, tagId: "seed-tag-schema" },
    { chunkId: ids.schemaChunks, tagId: "seed-tag-postgresql" },
    // Schema: Connections
    { chunkId: ids.schemaConn, tagId: "seed-tag-database" },
    { chunkId: ids.schemaConn, tagId: "seed-tag-schema" },
    { chunkId: ids.schemaConn, tagId: "seed-tag-postgresql" },
    // Schema: Versions
    { chunkId: ids.schemaVer, tagId: "seed-tag-database" },
    { chunkId: ids.schemaVer, tagId: "seed-tag-schema" },
    // Schema: Auth
    { chunkId: ids.schemaAuth, tagId: "seed-tag-database" },
    { chunkId: ids.schemaAuth, tagId: "seed-tag-schema" },
    { chunkId: ids.schemaAuth, tagId: "seed-tag-authentication" },
    { chunkId: ids.schemaAuth, tagId: "seed-tag-better-auth" },
    // Effect Error Handling
    { chunkId: ids.effect, tagId: "seed-tag-error-handling" },
    { chunkId: ids.effect, tagId: "seed-tag-effect" },
    { chunkId: ids.effect, tagId: "seed-tag-backend" },
    // API: Chunks
    { chunkId: ids.apiChunks, tagId: "seed-tag-api" },
    { chunkId: ids.apiChunks, tagId: "seed-tag-rest" },
    // API: Connections, Graph & Stats
    { chunkId: ids.apiConnGraph, tagId: "seed-tag-api" },
    { chunkId: ids.apiConnGraph, tagId: "seed-tag-graph" },
    // API: AI & Enrichment
    { chunkId: ids.apiAI, tagId: "seed-tag-api" },
    { chunkId: ids.apiAI, tagId: "seed-tag-enrichment" },
    { chunkId: ids.apiAI, tagId: "seed-tag-ollama" },
    // Enrichment Pipeline
    { chunkId: ids.enrich, tagId: "seed-tag-enrichment" },
    { chunkId: ids.enrich, tagId: "seed-tag-ollama" },
    { chunkId: ids.enrich, tagId: "seed-tag-guide" },
    // Semantic Search
    { chunkId: ids.semantic, tagId: "seed-tag-search" },
    { chunkId: ids.semantic, tagId: "seed-tag-pgvector" },
    // Repository Layer
    { chunkId: ids.repo, tagId: "seed-tag-repository" },
    { chunkId: ids.repo, tagId: "seed-tag-drizzle" },
    { chunkId: ids.repo, tagId: "seed-tag-backend" },
    // Service Layer
    { chunkId: ids.service, tagId: "seed-tag-service" },
    { chunkId: ids.service, tagId: "seed-tag-effect" },
    { chunkId: ids.service, tagId: "seed-tag-backend" },
    // Eden Treaty
    { chunkId: ids.eden, tagId: "seed-tag-eden" },
    { chunkId: ids.eden, tagId: "seed-tag-elysia" },
    { chunkId: ids.eden, tagId: "seed-tag-frontend" },
    { chunkId: ids.eden, tagId: "seed-tag-typescript" },
    // Frontend Routes
    { chunkId: ids.routes, tagId: "seed-tag-frontend" },
    { chunkId: ids.routes, tagId: "seed-tag-tanstack" },
    { chunkId: ids.routes, tagId: "seed-tag-react" },
    // Graph Visualization
    { chunkId: ids.graph, tagId: "seed-tag-graph" },
    { chunkId: ids.graph, tagId: "seed-tag-visualization" },
    { chunkId: ids.graph, tagId: "seed-tag-reactflow" },
    { chunkId: ids.graph, tagId: "seed-tag-frontend" },
    // Graph Layout
    { chunkId: ids.graphLayout, tagId: "seed-tag-graph" },
    { chunkId: ids.graphLayout, tagId: "seed-tag-force-directed" },
    { chunkId: ids.graphLayout, tagId: "seed-tag-visualization" },
    // Authentication System
    { chunkId: ids.auth, tagId: "seed-tag-authentication" },
    { chunkId: ids.auth, tagId: "seed-tag-better-auth" },
    // CLI Commands
    { chunkId: ids.cli, tagId: "seed-tag-cli" },
    { chunkId: ids.cli, tagId: "seed-tag-reference" },
    // CLI Local Store
    { chunkId: ids.cliStore, tagId: "seed-tag-cli" },
    { chunkId: ids.cliStore, tagId: "seed-tag-storage" },
    { chunkId: ids.cliStore, tagId: "seed-tag-sync" },
    // CLI Scanner
    { chunkId: ids.cliScanner, tagId: "seed-tag-cli" },
    { chunkId: ids.cliScanner, tagId: "seed-tag-scanner" },
    // Environment Variables
    { chunkId: ids.env, tagId: "seed-tag-configuration" },
    { chunkId: ids.env, tagId: "seed-tag-reference" },
    // Docker Deployment
    { chunkId: ids.docker, tagId: "seed-tag-deployment" },
    { chunkId: ids.docker, tagId: "seed-tag-docker" },
    { chunkId: ids.docker, tagId: "seed-tag-ci" },
    // Turborepo Build
    { chunkId: ids.turbo, tagId: "seed-tag-turborepo" },
    { chunkId: ids.turbo, tagId: "seed-tag-monorepo" },
    // Docs: Architecture
    { chunkId: ids.docsArch, tagId: "seed-tag-overview" },
    { chunkId: ids.docsArch, tagId: "seed-tag-frontend" },
    // Docs: Content Guidelines
    { chunkId: ids.docsContent, tagId: "seed-tag-guide" },
    // Docs: Deployment
    { chunkId: ids.docsDeploy, tagId: "seed-tag-deployment" },
    { chunkId: ids.docsDeploy, tagId: "seed-tag-ci" }
];

for (const ct of chunkTagAssociations) {
    await db.insert(chunkTag).values(ct).catch(e => console.error(`  \u2717 chunk_tag:`, e));
}
console.log(`  \u2713 ${chunkTagAssociations.length} chunk-tag associations`);

// ─── 1. Codebase ───────────────────────────────────────────────────
const CODEBASE_ID = "seed-codebase-fubbik";
await db
    .insert(codebase)
    .values({
        id: CODEBASE_ID,
        name: "fubbik",
        remoteUrl: "https://github.com/user/fubbik",
        localPaths: [process.cwd()],
        userId: DEV_USER_ID
    })
    .catch(e => console.error("  \u2717 codebase:", e));
console.log("  \u2713 1 codebase");

// Associate all seed chunks with the codebase
const docsChunkIdSet = new Set([ids.docsArch, ids.docsContent, ids.docsDeploy]);
const chunkCodebaseAssociations = chunks
    .filter(c => !docsChunkIdSet.has(c.id))
    .map(c => ({
        chunkId: c.id,
        codebaseId: CODEBASE_ID
    }));
for (const cc of chunkCodebaseAssociations) {
    await db.insert(chunkCodebase).values(cc).catch(e => console.error("  \u2717 chunk_codebase:", e));
}
console.log(`  \u2713 ${chunkCodebaseAssociations.length} chunk-codebase associations`);

// ─── 2. Applies-to patterns ────────────────────────────────────────
const appliesToPatterns = [
    { id: "seed-at-arch", chunkId: ids.arch, pattern: "packages/**", note: "All packages" },
    { id: "seed-at-schema", chunkId: ids.schemaChunks, pattern: "packages/db/src/schema/chunk.ts", note: "Chunk schema file" },
    { id: "seed-at-effect", chunkId: ids.effect, pattern: "packages/api/src/**/*.ts", note: "All API source files" },
    { id: "seed-at-api", chunkId: ids.apiChunks, pattern: "packages/api/src/chunks/**", note: "Chunks API routes" },
    { id: "seed-at-routes", chunkId: ids.routes, pattern: "apps/web/src/routes/**", note: "Frontend routes" },
    { id: "seed-at-graph", chunkId: ids.graph, pattern: "apps/web/src/features/graph/**", note: "Graph feature" },
    { id: "seed-at-cli", chunkId: ids.cli, pattern: "apps/cli/src/**", note: "CLI source" },
    { id: "seed-at-docker", chunkId: ids.docker, pattern: "Dockerfile*", note: "Docker configuration" },
    { id: "seed-at-turbo", chunkId: ids.turbo, pattern: "turbo.json", note: "Turborepo config" }
];
for (const at of appliesToPatterns) {
    await db.insert(chunkAppliesTo).values(at).catch(e => console.error("  \u2717 applies_to:", e));
}
console.log(`  \u2713 ${appliesToPatterns.length} applies-to patterns`);

// ─── 3. File references ────────────────────────────────────────────
const fileRefs = [
    { id: "seed-fr-schema", chunkId: ids.schemaChunks, path: "packages/db/src/schema/chunk.ts", relation: "documents" },
    { id: "seed-fr-effect", chunkId: ids.effect, path: "packages/api/src/errors.ts", relation: "documents" },
    { id: "seed-fr-eden", chunkId: ids.eden, path: "apps/web/src/utils/api.ts", relation: "configures" },
    { id: "seed-fr-auth", chunkId: ids.auth, path: "packages/auth/src/index.ts", relation: "documents" },
    { id: "seed-fr-env", chunkId: ids.env, path: "packages/env/src/index.ts", relation: "documents" }
];
for (const fr of fileRefs) {
    await db.insert(chunkFileRef).values(fr).catch(e => console.error("  \u2717 file_ref:", e));
}
console.log(`  \u2713 ${fileRefs.length} file references`);

// ─── 4. Use cases (before requirements, since requirements reference them) ──
const useCases = [
    {
        id: "seed-uc-knowledge",
        name: "Knowledge Management",
        description: "Creating, organizing, and navigating structured knowledge chunks and their relationships",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        order: 0
    },
    {
        id: "seed-uc-ai-agent",
        name: "AI Agent Integration",
        description: "Enabling AI agents to consume, create, and enrich knowledge via CLI and API",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        order: 1
    }
];
for (const uc of useCases) {
    await db.insert(useCase).values(uc).catch(e => console.error("  \u2717 use_case:", e));
}
console.log(`  \u2713 ${useCases.length} use cases`);

// ─── 5. Requirements ───────────────────────────────────────────────
const requirements = [
    {
        id: "seed-req-crud",
        title: "Chunk CRUD operations",
        description: "Users can create, read, update, and delete knowledge chunks",
        steps: [
            { keyword: "given" as const, text: "a logged-in user" },
            { keyword: "when" as const, text: "the user creates a chunk with title and content" },
            { keyword: "then" as const, text: "the chunk is stored in the database" },
            { keyword: "and" as const, text: "the chunk appears in the chunk list" },
            { keyword: "when" as const, text: "the user updates the chunk content" },
            { keyword: "then" as const, text: "the chunk version history is preserved" },
            { keyword: "when" as const, text: "the user deletes the chunk" },
            { keyword: "then" as const, text: "the chunk and its connections are removed" }
        ],
        status: "passing",
        priority: "must",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-knowledge",
        order: 0
    },
    {
        id: "seed-req-search",
        title: "Semantic search returns relevant results",
        description: "Embedding-based search surfaces contextually related chunks",
        steps: [
            { keyword: "given" as const, text: "chunks with embeddings exist in the database" },
            { keyword: "when" as const, text: "the user performs a semantic search query" },
            { keyword: "then" as const, text: "results are ranked by cosine similarity" },
            { keyword: "and" as const, text: "the most relevant chunks appear first" }
        ],
        status: "passing",
        priority: "should",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-ai-agent",
        order: 1
    },
    {
        id: "seed-req-plan",
        title: "Plan step tracking works end-to-end",
        description: "Users can create plans with steps and track progress through statuses",
        steps: [
            { keyword: "given" as const, text: "a user creates a plan with multiple steps" },
            { keyword: "when" as const, text: "the user marks a step as done" },
            { keyword: "then" as const, text: "the plan progress is updated accordingly" },
            { keyword: "and" as const, text: "linked chunks are accessible from the plan" }
        ],
        status: "untested",
        priority: "should",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-knowledge",
        order: 2
    }
];
for (const req of requirements) {
    await db.insert(requirement).values(req).catch(e => console.error("  \u2717 requirement:", e));
}
console.log(`  \u2713 ${requirements.length} requirements`);

// Link requirements to relevant chunks
const reqChunkLinks = [
    { requirementId: "seed-req-crud", chunkId: ids.apiChunks },
    { requirementId: "seed-req-crud", chunkId: ids.schemaChunks },
    { requirementId: "seed-req-search", chunkId: ids.semantic },
    { requirementId: "seed-req-search", chunkId: ids.enrich },
    { requirementId: "seed-req-plan", chunkId: ids.arch }
];
for (const rc of reqChunkLinks) {
    await db.insert(requirementChunk).values(rc).catch(e => console.error("  \u2717 requirement_chunk:", e));
}
console.log(`  \u2713 ${reqChunkLinks.length} requirement-chunk links`);

// ─── 6. Plans ──────────────────────────────────────────────────────

const [planCompleted] = await db
    .insert(plan)
    .values({
        title: "Add user avatars",
        description:
            "Let users upload a profile picture. Shown on chunk detail pages and in the top-right nav.",
        status: "completed",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        completedAt: new Date(),
    })
    .returning();

if (!planCompleted) throw new Error("failed to seed completed plan");

await db.insert(planTask).values([
    { planId: planCompleted.id, title: "Add avatar column to user table", status: "done", order: 0 },
    { planId: planCompleted.id, title: "Wire upload endpoint", status: "done", order: 1 },
    { planId: planCompleted.id, title: "Render avatar in nav", status: "done", order: 2 },
]);

const [planInProgress] = await db
    .insert(plan)
    .values({
        title: "Federated chunk search",
        description:
            "Search chunks across all linked codebases from one query. Returns results grouped by codebase.",
        status: "in_progress",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
    })
    .returning();

if (!planInProgress) throw new Error("failed to seed in_progress plan");

// Link the first requirement if one exists
await db.insert(planRequirement).values({
    planId: planInProgress.id,
    requirementId: "seed-req-plan",
    order: 0,
}).catch(e => console.error("  ✗ planRequirement:", e));

await db.insert(planAnalyzeItem).values([
    {
        planId: planInProgress.id,
        kind: "chunk",
        chunkId: ids.apiChunks,
        text: "Existing search service entry point",
        order: 0,
    },
    {
        planId: planInProgress.id,
        kind: "file",
        filePath: "packages/api/src/search/service.ts",
        text: "Main search entry point",
        metadata: { lineStart: 1, lineEnd: 50 },
        order: 1,
    },
    {
        planId: planInProgress.id,
        kind: "risk",
        text: "Cross-codebase indexes may blow up memory for 10+ codebases",
        metadata: { severity: "medium" },
        order: 2,
    },
    {
        planId: planInProgress.id,
        kind: "assumption",
        text: "All codebases share the same embedding model",
        metadata: { verified: false },
        order: 3,
    },
    {
        planId: planInProgress.id,
        kind: "question",
        text: "Should archived codebases be searchable?",
        metadata: { answered: false },
        order: 4,
    },
]);

await db.insert(planTask).values([
    { planId: planInProgress.id, title: "Extend search service to accept codebase list", status: "done", order: 0 },
    { planId: planInProgress.id, title: "Group results by codebase in response", status: "in_progress", order: 1 },
    { planId: planInProgress.id, title: "Add federated mode toggle to search page", status: "pending", order: 2 },
    { planId: planInProgress.id, title: "Integration test: 3 codebases, 1 query", status: "pending", order: 3 },
    { planId: planInProgress.id, title: "Update CLAUDE.md with federated search docs", status: "pending", order: 4 },
]);

const [planAnalyzing] = await db
    .insert(plan)
    .values({
        title: "Plans as a central entity",
        description:
            "Make Plan the home for a unit of work — description, linked requirements, structured analyze fields, and enriched tasks.",
        status: "analyzing",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
    })
    .returning();

if (!planAnalyzing) throw new Error("failed to seed analyzing plan");

await db.insert(planAnalyzeItem).values([
    {
        planId: planAnalyzing.id,
        kind: "risk",
        text: "Dropping session data loses review history",
        metadata: { severity: "low" },
        order: 0,
    },
    {
        planId: planAnalyzing.id,
        kind: "assumption",
        text: "Existing plan data is mostly seed/scratch, safe to wipe",
        metadata: { verified: true },
        order: 1,
    },
]);

console.log("  ✓ 3 plans with tasks and analyze items");

// ─── 8. Document Chunks ───────────────────────────────────────────
import { document } from "./schema/document";

await db.delete(document).where(eq(document.userId, DEV_USER_ID)).catch(() => {});

const seedDocs = [
    {
        id: "seed-doc-getting-started",
        title: "Getting Started Guide",
        sourcePath: "docs/guide/getting-started.md",
        contentHash: "abc123",
        description: "Step-by-step guide for setting up fubbik locally",
        codebaseId: CODEBASE_ID,
        userId: DEV_USER_ID,
    },
    {
        id: "seed-doc-architecture",
        title: "Architecture Reference",
        sourcePath: "docs/guide/architecture.md",
        contentHash: "def456",
        description: "System architecture, data flow, and design decisions",
        codebaseId: CODEBASE_ID,
        userId: DEV_USER_ID,
    },
    {
        id: "seed-doc-api-reference",
        title: "API Reference",
        sourcePath: "docs/guide/api-reference.md",
        contentHash: "ghi789",
        description: "Complete REST API documentation with examples",
        codebaseId: CODEBASE_ID,
        userId: DEV_USER_ID,
    },
];

for (const doc of seedDocs) {
    await db.insert(document).values(doc).catch(e => console.error(`  ✗ doc ${doc.title}:`, e));
}
console.log(`  ✓ ${seedDocs.length} documents`);

// Document-linked chunks (content sections from imported docs)
const docChunks = [
    {
        id: "seed-doc-chunk-setup",
        title: "Local Development Setup",
        type: "document",
        content: `## Prerequisites

- **Node.js** 18+ and **bun** runtime
- **pnpm** package manager
- **PostgreSQL** 14+ with pgvector and pg_trgm extensions
- **Ollama** (optional, for AI enrichment and semantic search)

## Quick Start

\`\`\`bash
# Clone and install
git clone https://github.com/Pontusg45/fubbik.git
cd fubbik && pnpm install

# Set up database
createdb fubbik
psql -d fubbik -c "CREATE EXTENSION vector; CREATE EXTENSION pg_trgm;"

# Configure environment
cp apps/server/.env.example apps/server/.env
# Edit DATABASE_URL in .env

# Push schema and seed
pnpm db:push && pnpm seed

# Start development
pnpm dev
\`\`\`

The web UI runs at \`http://localhost:3001\` and the API at \`http://localhost:3000/docs\`.`,
        documentId: "seed-doc-getting-started",
        documentOrder: 0,
    },
    {
        id: "seed-doc-chunk-concepts",
        title: "Core Concepts: Chunks, Connections, and Codebases",
        type: "document",
        content: `## Chunks

The central entity in fubbik. A chunk is a discrete unit of knowledge — a convention, architecture decision, runbook, API reference, or any structured insight about your codebase.

Each chunk has:
- **Title and content** (markdown)
- **Type** — note, document, reference, schema, checklist
- **Tags** — categorization via normalized tag types
- **Scope** — key-value metadata for filtering
- **Health score** — computed from freshness, completeness, richness, connectivity, and coverage

## Connections

Directed, typed edges between chunks. Relation types: \`related_to\`, \`part_of\`, \`depends_on\`, \`extends\`, \`references\`, \`supports\`, \`contradicts\`, \`alternative_to\`.

Connections are global — they work across codebases, enabling cross-project knowledge linking.

## Codebases & Workspaces

Chunks can belong to codebases (identified by git remote URL). Workspaces group related codebases (e.g., frontend + backend + infra). The CLI auto-detects your codebase via git remote.`,
        documentId: "seed-doc-getting-started",
        documentOrder: 1,
    },
    {
        id: "seed-doc-chunk-arch-layers",
        title: "Architecture: Repository → Service → Route Pattern",
        type: "document",
        content: `## Backend Architecture

Fubbik follows a strict three-layer architecture:

### Repository Layer (\`packages/db/src/repository/\`)
- Pure data access, no business logic
- Returns \`Effect<T, DatabaseError>\`
- Uses Drizzle ORM for type-safe queries

### Service Layer (\`packages/api/src/*/service.ts\`)
- Composes repository Effects
- Adds business logic, validation
- Introduces \`NotFoundError\`, \`AuthError\`, \`ValidationError\`

### Route Layer (\`packages/api/src/*/routes.ts\`)
- Elysia route handlers
- Calls \`Effect.runPromise(requireSession(ctx).pipe(...))\`
- Schema validation via Elysia \`t\` schemas

### Error Handling

Errors propagate through Effect to a global \`.onError\` handler that maps \`_tag\` to HTTP status codes:
- \`ValidationError\` → 400
- \`AuthError\` → 401
- \`NotFoundError\` → 404
- \`DatabaseError\` → 500`,
        documentId: "seed-doc-architecture",
        documentOrder: 0,
    },
    {
        id: "seed-doc-chunk-api-overview",
        title: "API Overview: Authentication and Endpoints",
        type: "reference",
        content: `## Authentication

Fubbik uses Better Auth for session-based authentication. All API endpoints (except \`/api/health\`) require an authenticated session.

## Key Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| \`/api/chunks\` | GET | List chunks (supports filtering, search, pagination) |
| \`/api/chunks\` | POST | Create chunk |
| \`/api/chunks/:id\` | GET | Chunk detail with connections, health score |
| \`/api/chunks/search/semantic\` | GET | Embedding-based search |
| \`/api/graph\` | GET | Knowledge graph data (nodes + edges) |
| \`/api/plans\` | GET/POST | Plan management |
| \`/api/requirements\` | GET/POST | Requirements with BDD steps |
| \`/api/sessions\` | GET/POST | Implementation session tracking |
| \`/api/context/for-file\` | GET | File-relevant chunks + requirements |
| \`/api/chunks/export/claude-md\` | GET | AI-optimized context export |

## OpenAPI

Full interactive documentation at \`/docs\` (Swagger UI).`,
        documentId: "seed-doc-api-reference",
        documentOrder: 0,
    },
];

for (const dc of docChunks) {
    await db.insert(chunk).values({ ...dc, userId: DEV_USER_ID }).catch(e => console.error(`  ✗ doc chunk ${dc.title}:`, e));
}
console.log(`  ✓ ${docChunks.length} document chunks`);

// Connect doc chunks to codebase
for (const dc of docChunks) {
    await db.insert(chunkCodebase).values({ chunkId: dc.id, codebaseId: CODEBASE_ID }).catch(() => {});
}
console.log(`  ✓ ${docChunks.length} doc chunk-codebase links`);

// Connect doc chunks to existing chunks
const docConnections = [
    { id: "seed-conn-setup-env", sourceId: "seed-doc-chunk-setup", targetId: ids.env, relation: "references" },
    { id: "seed-conn-setup-docker", sourceId: "seed-doc-chunk-setup", targetId: ids.docker, relation: "references" },
    { id: "seed-conn-concepts-schema", sourceId: "seed-doc-chunk-concepts", targetId: ids.schemaChunks, relation: "references" },
    { id: "seed-conn-concepts-graph", sourceId: "seed-doc-chunk-concepts", targetId: ids.graph, relation: "references" },
    { id: "seed-conn-arch-repo", sourceId: "seed-doc-chunk-arch-layers", targetId: ids.repo, relation: "part_of" },
    { id: "seed-conn-arch-service", sourceId: "seed-doc-chunk-arch-layers", targetId: ids.service, relation: "part_of" },
    { id: "seed-conn-arch-routes", sourceId: "seed-doc-chunk-arch-layers", targetId: ids.routes, relation: "part_of" },
    { id: "seed-conn-arch-effect", sourceId: "seed-doc-chunk-arch-layers", targetId: ids.effect, relation: "depends_on" },
    { id: "seed-conn-api-eden", sourceId: "seed-doc-chunk-api-overview", targetId: ids.eden, relation: "references" },
    { id: "seed-conn-api-auth", sourceId: "seed-doc-chunk-api-overview", targetId: ids.auth, relation: "references" },
];
for (const conn of docConnections) {
    await db.insert(chunkConnection).values(conn).catch(e => console.error(`  ✗ doc connection:`, e));
}
console.log(`  ✓ ${docConnections.length} doc chunk connections`);

// ─── 9. Vocabulary ─────────────────────────────────────────────────
const vocabEntries = [
    { id: "seed-vocab-chunk", word: "chunk", category: "actor", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-connection", word: "connection", category: "target", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-codebase", word: "codebase", category: "target", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-plan", word: "plan", category: "actor", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-requirement", word: "requirement", category: "target", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-create", word: "create", category: "action", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-search", word: "search", category: "action", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-enrich", word: "enrich", category: "action", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-passing", word: "passing", category: "state", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-draft", word: "draft", category: "state", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID }
];
for (const ve of vocabEntries) {
    await db.insert(vocabularyEntry).values(ve).catch(e => console.error("  \u2717 vocabulary:", e));
}
console.log(`  \u2713 ${vocabEntries.length} vocabulary entries`);

// ─── 8. Collection ─────────────────────────────────────────────────
await db
    .insert(collection)
    .values({
        id: "seed-collection-arch",
        name: "Architecture Decisions",
        description: "All architecture-related document chunks",
        filter: { type: "document", tags: "architecture" },
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID
    })
    .catch(e => console.error("  \u2717 collection:", e));
console.log("  \u2713 1 collection");

// ─── 9. Workspace ──────────────────────────────────────────────────
const WORKSPACE_ID = "seed-workspace-fubbik";
await db
    .insert(workspace)
    .values({
        id: WORKSPACE_ID,
        name: "Fubbik Platform",
        description: "Main workspace for the fubbik knowledge framework",
        userId: DEV_USER_ID
    })
    .catch(e => console.error("  \u2717 workspace:", e));

await db
    .insert(workspaceCodebase)
    .values({
        workspaceId: WORKSPACE_ID,
        codebaseId: CODEBASE_ID
    })
    .catch(e => console.error("  \u2717 workspace_codebase:", e));
console.log("  \u2713 1 workspace with codebase assignment");

// ─── 10. Second codebase (fubbik-docs) ─────────────────────────────
const CODEBASE_DOCS_ID = "seed-codebase-docs";
await db
    .insert(codebase)
    .values({
        id: CODEBASE_DOCS_ID,
        name: "fubbik-docs",
        remoteUrl: "https://github.com/user/fubbik-docs",
        localPaths: ["/Users/pontus/projects/fubbik-docs"],
        userId: DEV_USER_ID
    })
    .catch(e => console.error("  \u2717 docs codebase:", e));
console.log("  \u2713 1 docs codebase");

// Associate docs chunks with the docs codebase
const docsChunkIds = [ids.docsArch, ids.docsContent, ids.docsDeploy];
for (const chunkId of docsChunkIds) {
    await db.insert(chunkCodebase).values({ chunkId, codebaseId: CODEBASE_DOCS_ID }).catch(e => console.error("  \u2717 docs chunk_codebase:", e));
}
console.log(`  \u2713 ${docsChunkIds.length} docs chunk-codebase associations`);

// Add docs codebase to the workspace
await db
    .insert(workspaceCodebase)
    .values({ workspaceId: WORKSPACE_ID, codebaseId: CODEBASE_DOCS_ID })
    .catch(e => console.error("  \u2717 workspace_codebase (docs):", e));
console.log("  \u2713 docs codebase added to workspace");

// ─── 11. More requirements ─────────────────────────────────────────
const moreRequirements = [
    {
        id: "seed-req-graph-layouts",
        title: "Graph visualization supports multiple layouts",
        description: "The graph view provides force-directed, hierarchical, and radial layout algorithms",
        steps: [
            { keyword: "given" as const, text: "a knowledge graph with at least 10 nodes" },
            { keyword: "when" as const, text: "the user selects the hierarchical layout" },
            { keyword: "then" as const, text: "nodes are arranged in a tree structure based on part_of edges" },
            { keyword: "when" as const, text: "the user selects the radial layout" },
            { keyword: "then" as const, text: "nodes are arranged in concentric rings from the most-connected node" }
        ],
        status: "passing",
        priority: "must",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-knowledge",
        order: 3
    },
    {
        id: "seed-req-cli-color",
        title: "CLI supports colored output",
        description: "CLI commands use ANSI colors for improved readability in terminal environments",
        steps: [
            { keyword: "given" as const, text: "a terminal that supports ANSI colors" },
            { keyword: "when" as const, text: "the user runs fubbik list" },
            { keyword: "then" as const, text: "chunk types are color-coded" },
            { keyword: "and" as const, text: "tags are displayed in a distinct color" }
        ],
        status: "passing",
        priority: "should",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-ai-agent",
        order: 4
    },
    {
        id: "seed-req-health-stale",
        title: "Knowledge health detects stale chunks",
        description: "The health endpoint identifies chunks that have not been updated in a configurable time window",
        steps: [
            { keyword: "given" as const, text: "chunks exist that were last updated more than 90 days ago" },
            { keyword: "when" as const, text: "the user visits the knowledge health page" },
            { keyword: "then" as const, text: "stale chunks are listed with their last-updated date" },
            { keyword: "and" as const, text: "the user can click through to edit each stale chunk" }
        ],
        status: "failing",
        priority: "should",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-knowledge",
        order: 5
    },
    {
        id: "seed-req-workspace-queries",
        title: "Workspace-aware chunk queries",
        description: "Chunk list and search endpoints can be scoped to a workspace, returning chunks from all codebases in that workspace",
        steps: [
            { keyword: "given" as const, text: "a workspace containing two codebases" },
            { keyword: "when" as const, text: "the user queries chunks scoped to the workspace" },
            { keyword: "then" as const, text: "chunks from both codebases are returned" },
            { keyword: "and" as const, text: "results indicate which codebase each chunk belongs to" }
        ],
        status: "untested",
        priority: "could",
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID,
        useCaseId: "seed-uc-knowledge",
        order: 6
    }
];
for (const req of moreRequirements) {
    await db.insert(requirement).values(req).catch(e => console.error("  \u2717 requirement:", e));
}
console.log(`  \u2713 ${moreRequirements.length} more requirements`);

// Link new requirements to chunks
const moreReqChunkLinks = [
    { requirementId: "seed-req-graph-layouts", chunkId: ids.graph },
    { requirementId: "seed-req-graph-layouts", chunkId: ids.graphLayout },
    { requirementId: "seed-req-cli-color", chunkId: ids.cli },
    { requirementId: "seed-req-health-stale", chunkId: ids.arch },
    { requirementId: "seed-req-workspace-queries", chunkId: ids.arch }
];
for (const rc of moreReqChunkLinks) {
    await db.insert(requirementChunk).values(rc).catch(e => console.error("  \u2717 requirement_chunk:", e));
}
console.log(`  \u2713 ${moreReqChunkLinks.length} more requirement-chunk links`);

// (Second plan seeding moved to section 6 — Plans)

// ─── 13. More vocabulary ────────────────────────────────────────────
const moreVocabEntries = [
    { id: "seed-vocab-tag", word: "tag", category: "target", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-workspace", word: "workspace", category: "target", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-template", word: "template", category: "target", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-link", word: "link", category: "action", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-update", word: "update", category: "action", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-delete", word: "delete", category: "action", expects: ["target"], codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-failing", word: "failing", category: "state", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID },
    { id: "seed-vocab-active", word: "active", category: "state", expects: null, codebaseId: CODEBASE_ID, userId: DEV_USER_ID }
];
for (const ve of moreVocabEntries) {
    await db.insert(vocabularyEntry).values(ve).catch(e => console.error("  \u2717 vocabulary:", e));
}
console.log(`  \u2713 ${moreVocabEntries.length} more vocabulary entries`);

// ─── 14. More collections ───────────────────────────────────────────
const moreCollections = [
    {
        id: "seed-collection-api-refs",
        name: "API References",
        description: "All API reference chunks",
        filter: { type: "reference" },
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID
    },
    {
        id: "seed-collection-recent",
        name: "Recent Changes",
        description: "Chunks updated in the last 7 days",
        filter: { sort: "updated", after: "7" },
        userId: DEV_USER_ID,
        codebaseId: CODEBASE_ID
    }
];
for (const col of moreCollections) {
    await db.insert(collection).values(col).catch(e => console.error("  \u2717 collection:", e));
}
console.log(`  \u2713 ${moreCollections.length} more collections`);

// ─── 15. More applies-to patterns ───────────────────────────────────
const moreAppliesToPatterns = [
    { id: "seed-at-auth", chunkId: ids.auth, pattern: "packages/auth/**", note: "Auth package" },
    { id: "seed-at-service", chunkId: ids.service, pattern: "packages/api/src/*/service.ts", note: "Service layer files" },
    { id: "seed-at-repo", chunkId: ids.repo, pattern: "packages/db/src/repository/**", note: "Repository layer" },
    { id: "seed-at-eden", chunkId: ids.eden, pattern: "apps/web/src/utils/**", note: "Frontend utility files" }
];
for (const at of moreAppliesToPatterns) {
    await db.insert(chunkAppliesTo).values(at).catch(e => console.error("  \u2717 applies_to:", e));
}
console.log(`  \u2713 ${moreAppliesToPatterns.length} more applies-to patterns`);

// ─── 16. More file references ───────────────────────────────────────
const moreFileRefs = [
    { id: "seed-fr-repo", chunkId: ids.repo, path: "packages/db/src/repository/chunk.ts", relation: "documents" },
    { id: "seed-fr-service", chunkId: ids.service, path: "packages/api/src/chunks/service.ts", relation: "documents" },
    { id: "seed-fr-routes", chunkId: ids.routes, path: "apps/web/src/routes/__root.tsx", relation: "documents" },
    { id: "seed-fr-graph", chunkId: ids.graph, path: "apps/web/src/features/graph/graph-view.tsx", relation: "documents" },
    { id: "seed-fr-turbo", chunkId: ids.turbo, path: "turbo.json", relation: "configures" }
];
for (const fr of moreFileRefs) {
    await db.insert(chunkFileRef).values(fr).catch(e => console.error("  \u2717 file_ref:", e));
}
console.log(`  \u2713 ${moreFileRefs.length} more file references`);

console.log(`\n\u2705 Database seeded: ${chunks.length} chunks, ${connections.length} connections, ${seedTagTypes.length} tag types, ${seedTags.length} tags, plus codebases, patterns, refs, requirements, use cases, plans, vocabulary, collections, workspaces`);

// Build a lookup for enrichment prompts (replacing old tags property)
const chunkTagNames = new Map<string, string[]>();
for (const ct of chunkTagAssociations) {
    const tagName = seedTags.find(t => t.id === ct.tagId)?.name ?? "";
    const existing = chunkTagNames.get(ct.chunkId) ?? [];
    existing.push(tagName);
    chunkTagNames.set(ct.chunkId, existing);
}

// Attempt to enrich all seeded chunks via Ollama
const OLLAMA_URL = process.env.OLLAMA_URL ?? "http://localhost:11434";
try {
    const res = await fetch(`${OLLAMA_URL}/api/tags`, { signal: AbortSignal.timeout(2000) });
    if (res.ok) {
        console.log("\nEnriching chunks via Ollama...");
        for (const c of chunks) {
            try {
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
Tags: ${(chunkTagNames.get(c.id) ?? []).join(", ")}
Content: ${c.content}`,
                        format: "json",
                        stream: false
                    })
                });
                if (!genRes.ok) continue;
                const genData = (await genRes.json()) as { response: string };
                const metadata = JSON.parse(genData.response) as { summary: string; aliases: string[]; notAbout: string[] };

                const embRes = await fetch(`${OLLAMA_URL}/api/embeddings`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        model: "nomic-embed-text",
                        prompt: `search_document: ${c.title}\n${metadata.summary}\n${c.content}`
                    })
                });
                if (!embRes.ok) continue;
                const embData = (await embRes.json()) as { embedding: number[] };

                const embStr = `[${embData.embedding.join(",")}]`;
                await db.execute(
                    sql`UPDATE chunk SET summary = ${metadata.summary}, aliases = ${JSON.stringify(metadata.aliases)}::jsonb, not_about = ${JSON.stringify(metadata.notAbout)}::jsonb, embedding = ${embStr}::vector WHERE id = ${c.id}`
                );
                console.log(`  \u2713 Enriched: ${c.title}`);
            } catch {
                console.log(`  \u2717 Failed: ${c.title}`);
            }
        }
    } else {
        console.log("\nOllama not available \u2014 skipping enrichment");
    }
} catch {
    console.log("\nOllama not available \u2014 skipping enrichment");
}

// Seed default instance settings
import { instanceSettings } from "./schema/settings";

const defaultInstanceSettings: Array<{ key: string; value: unknown }> = [
    { key: "aiEnabled", value: true },
    { key: "ollamaUrl", value: process.env.OLLAMA_URL ?? "http://localhost:11434" },
    { key: "enrichmentEnabled", value: true },
    { key: "semanticSearchEnabled", value: true },
    { key: "aiSuggestionsEnabled", value: true },
    { key: "vocabularySuggestEnabled", value: true },
    { key: "registrationEnabled", value: true },
    { key: "maxChunksPerCodebase", value: 10000 }
];

for (const setting of defaultInstanceSettings) {
    await db
        .insert(instanceSettings)
        .values({ key: setting.key, value: setting.value })
        .onConflictDoNothing();
}
console.log("\nSeeded default instance settings");

process.exit(0);
