/**
 * Chunks module — the main knowledge base.
 *
 * Organised into thematic sections. Each chunk has a short `summary` (for the
 * neighbors / search UI) and a substantial `content` body. Where there's a
 * meaningful decision, `rationale` / `consequences` carry the "why" so the
 * knowledge graph preserves reasoning, not just facts.
 *
 * The fixture DSL handles id generation, codebase linkage and tag wiring; the
 * connections module (next) declares the topology between these chunks.
 */

import { eq } from "drizzle-orm";

import { chunk } from "../../schema/chunk";
import { loadChunkFixtures, type ChunkFixture } from "../fixtures";
import type { SeedContext } from "../context";

// ---------------------------------------------------------------------------
// 1) Architecture & system overview
// ---------------------------------------------------------------------------

const ARCHITECTURE: ChunkFixture[] = [
    {
        name: "arch",
        title: "Fubbik Architecture Overview",
        type: "document",
        summary: "Local-first knowledge framework: chunks + typed connections + vector search, orchestrated through Elysia + Effect + Drizzle + TanStack Start.",
        content: `Fubbik stores structured knowledge as **chunks** (atomic units) connected by **typed relationships** (a directed graph) with a **vector embedding** (semantic similarity). It is designed to be used by humans *and* machines — a web UI for browsing, a CLI for capture, an MCP server for AI agents, and a VS Code extension for in-editor access.

## Core data flow

\`\`\`
CLI / Web / VS Code / MCP agent
        ↓ Eden Treaty (type-safe RPC)
Elysia API (routes)
        ↓ Effect (typed errors)
Service layer (business logic)
        ↓
Repository layer (Drizzle ORM)
        ↓
PostgreSQL + pgvector + pg_trgm
\`\`\`

## Monorepo layout

- \`apps/web\` — TanStack Start SSR frontend, force-directed graph
- \`apps/server\` — Elysia backend wiring + OpenTelemetry + winston logging
- \`apps/cli\` — Commander.js CLI (\`fubbik …\`)
- \`apps/vscode\` — standalone VS Code / Cursor extension
- \`packages/api\` — Elysia routes, Effect services, Eden-treaty type export
- \`packages/auth\` — Better Auth wiring + Drizzle adapter
- \`packages/config\` — shared TypeScript config
- \`packages/db\` — schema, repositories, migrations, seed
- \`packages/env\` — environment variable validation via Elysia \`t\`
- \`packages/mcp\` — MCP server for AI agent integration

## Key technologies

- **Runtime**: Bun
- **Frontend**: TanStack Start, Tailwind, shadcn-ui (base-ui), @xyflow/react
- **Backend**: Elysia, Effect (typed errors), Drizzle ORM, Better Auth
- **Database**: PostgreSQL 16+ with pgvector and pg_trgm
- **AI**: Ollama (local LLM — llama3.2 for enrichment, nomic-embed-text for embeddings)
- **Build**: Turborepo, pnpm`,
        rationale: "Local-first so knowledge stays queryable without internet; typed errors keep failure modes explicit; vector search + graph structure together cover both 'fuzzy' and 'precise' knowledge retrieval.",
        consequences: "Every feature has to plug through Elysia + Effect + Drizzle. Adding a non-Postgres store or a non-Effect service would fight the grain of the codebase.",
        tags: ["architecture", "typescript", "onboarding"]
    }
];

// ---------------------------------------------------------------------------
// 2) Backend conventions
// ---------------------------------------------------------------------------

const BACKEND: ChunkFixture[] = [
    {
        name: "repo-pattern",
        title: "Repository → Service → Route",
        type: "convention",
        summary: "Three strict layers. Repositories return Effect<T, DatabaseError>. Services compose + add domain errors. Routes just runPromise.",
        content: `All backend code is organised in three layers and never skips one:

## 1. Repository (\`packages/db/src/repository/*\`)

Pure data access. Returns \`Effect<T, DatabaseError>\`. No business logic, no HTTP concepts. Uses Drizzle for queries.

\`\`\`ts
export function getChunkById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.id, id)];
            if (userId) conditions.push(eq(chunk.userId, userId));
            const [row] = await db.select().from(chunk).where(and(...conditions));
            return row ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
\`\`\`

## 2. Service (\`packages/api/src/*/service.ts\`)

Composes repository Effects and introduces typed domain errors (\`NotFoundError\`, \`ValidationError\`, \`AuthError\`). No raw SQL, no HTTP status codes.

\`\`\`ts
export function getChunkDetail(id: string, userId?: string) {
    return getChunkById(id, userId).pipe(
        Effect.flatMap(row =>
            row ? Effect.succeed(row) : Effect.fail(new NotFoundError({ resource: "Chunk" }))
        )
    );
}
\`\`\`

## 3. Route (\`packages/api/src/*/routes.ts\`)

Thin wrapper. \`Effect.runPromise\` the service call; the global \`.onError\` handler maps error \`_tag\` to HTTP status.

\`\`\`ts
.get("/chunks/:id", ctx =>
    Effect.runPromise(
        requireSession(ctx).pipe(
            Effect.flatMap(session => chunkService.getChunkDetail(ctx.params.id, session.user.id))
        )
    )
)
\`\`\``,
        rationale: "Each layer is testable in isolation. Repositories don't know about HTTP; services don't know about SQL; routes don't know about business logic. Typed errors mean status codes are derived, not sprinkled.",
        consequences: "A route that needs a new DB query must go through all three layers. No shortcuts — but this is the price of never having to hunt for where an error surfaces from.",
        tags: ["repository-pattern", "service-layer", "convention", "effect", "elysia"]
    },
    {
        name: "typed-errors",
        title: "Typed errors via Effect tagged classes",
        type: "convention",
        summary: "NotFoundError, AuthError, ValidationError, DatabaseError, AiError are Data.TaggedError classes. Global .onError maps _tag → HTTP status.",
        content: `All backend errors are tagged Effect classes, not thrown exceptions:

\`\`\`ts
export class NotFoundError extends Data.TaggedError("NotFoundError")<{ resource: string }> {}
export class AuthError extends Data.TaggedError("AuthError")<{}> {}
export class ValidationError extends Data.TaggedError("ValidationError")<{ message: string }> {}
export class DatabaseError extends Data.TaggedError("DatabaseError")<{ cause?: unknown }> {}
export class AiError extends Data.TaggedError("AiError")<{ cause?: unknown }> {}
\`\`\`

The Elysia root mounts a single \`.onError\` that extracts the Effect failure cause from the FiberFailure and switches on \`_tag\`:

\`\`\`ts
.onError(({ error, set }) => {
    const effectError = extractEffectError(error);
    switch (effectError._tag) {
        case "ValidationError":  set.status = 400; return { message: effectError.message };
        case "AuthError":        set.status = 401; return { message: "Authentication required" };
        case "NotFoundError":    set.status = 404; return { message: \`\${effectError.resource} not found\` };
        case "AiError":          set.status = 502; return { message: "AI service error" };
        case "DatabaseError":    set.status = 500; return { message: "Internal server error" };
    }
});
\`\`\`

Routes therefore never write status codes. A service that fails with \`NotFoundError\` automatically becomes a 404 at the edge.`,
        rationale: "Errors are part of the type signature. TS will not compile a route that forgets to handle a failure mode the service can produce.",
        consequences: "Adding a new error class means updating the global handler in one place. Throwing a bare Error bypasses the typed system and will render as a generic 500.",
        tags: ["typed-errors", "effect", "error-handling", "convention"]
    },
    {
        name: "eden-treaty",
        title: "Eden Treaty for type-safe RPC",
        type: "reference",
        summary: "Frontend imports `type Api` from @fubbik/api and calls `api.api.chunks.get()` with full end-to-end typing.",
        content: `\`packages/api/src/index.ts\` exports \`export type Api = typeof api\`. The web app imports this type and wraps it with Eden Treaty:

\`\`\`ts
// apps/web/src/utils/api.ts
import { treaty } from "@elysiajs/eden";
import type { Api } from "@fubbik/api";
export const api = treaty<Api>(env.VITE_SERVER_URL, { fetch: { credentials: "include" } });
\`\`\`

Call it like an object:

\`\`\`ts
const { data, error } = await api.api.chunks({ id: chunkId }).get();
const { data } = await api.api["chunk-types"].post({ id: "runbook", label: "Runbook" });
\`\`\`

Path params nest as functions; query/body get typed from the Elysia \`t\` schema defined on each route. A schema change on the server produces a compile error on the client — **no drift possible**.`,
        rationale: "The client is always in sync with the server contract. Renaming a route or changing a body shape surfaces the break immediately in every consumer.",
        tags: ["eden-treaty", "elysia", "typescript", "reference"]
    },
    {
        name: "env-validation",
        title: "Environment validation with Elysia t",
        type: "convention",
        summary: "All env vars validated at module load through `packages/env`; type-safe access everywhere.",
        content: `\`packages/env\` exports validated env objects for server and web:

\`\`\`ts
// packages/env/server.ts
import { t } from "elysia";

const EnvSchema = t.Object({
    DATABASE_URL: t.String(),
    BETTER_AUTH_SECRET: t.String({ minLength: 32 }),
    PORT: t.Optional(t.String()),
    OLLAMA_URL: t.Optional(t.String())
});
\`\`\`

Consumers import a pre-validated object, not \`process.env\`:

\`\`\`ts
import { env } from "@fubbik/env/server";
const port = env.PORT ?? "3000";  // typed string | undefined, validated at boot
\`\`\`

Any missing required var fails at process start with a clear message, not at first use somewhere deep in request handling.`,
        tags: ["elysia", "convention", "typescript"]
    }
];

// ---------------------------------------------------------------------------
// 3) Data model
// ---------------------------------------------------------------------------

const DATA_MODEL: ChunkFixture[] = [
    {
        name: "schema-chunks",
        title: "Database Schema: chunk table",
        type: "schema",
        summary: "The atomic unit of knowledge. Title + content + type (FK to catalog) + vector embedding + scope + enrichment fields.",
        content: `\`chunk\` is the core table. Every piece of knowledge is exactly one chunk.

Selected columns:

- \`id\` text PK (ULID-like strings for seed data, UUIDs otherwise)
- \`title\` text not null, \`content\` text default ''
- \`type\` text **FK → chunk_type.id** (catalog-driven, restrict on delete)
- \`user_id\` text FK → user.id
- \`embedding\` vector(768) — nomic-embed-text output
- \`summary\`, \`aliases\` (jsonb str[]), \`not_about\` (jsonb str[]) — Ollama enrichment
- \`scope\` jsonb — arbitrary {key: value} metadata
- \`rationale\`, \`alternatives\`, \`consequences\` — optional "why" decision context
- \`origin\` — 'human' | 'ai'
- \`review_status\` — 'draft' | 'reviewed' | 'approved'
- \`archived_at\` — soft-delete for archiving
- \`document_id\`, \`document_order\` — optional source document linkage
- \`is_entry_point\` boolean — marks a chunk as a top-level reading start
- \`created_at\`, \`updated_at\`, \`embedding_updated_at\` — auditable timestamps

## Indexes

- pgvector index on \`embedding\` for cosine distance (<=>) queries
- pg_trgm GIN index for fuzzy title/content search
- B-tree on user_id, type, created_at for common filters`,
        tags: ["chunks", "drizzle", "postgres", "pgvector"]
    },
    {
        name: "schema-connection",
        title: "Database Schema: chunk_connection",
        type: "schema",
        summary: "Directed edges between chunks. `relation` is FK to connection_relation catalog; inverse_of_id makes pairs discoverable.",
        content: `\`chunk_connection\` holds directed edges:

- \`id\` text PK
- \`source_id\` FK → chunk.id (ON DELETE CASCADE)
- \`target_id\` FK → chunk.id (ON DELETE CASCADE)
- \`relation\` text **FK → connection_relation.id** (ON DELETE RESTRICT)
- unique index on (source_id, target_id, relation)

## Relation catalog

Builtins (all with \`built_in = true\`):

| Slug | Label | Arrow | Direction | Inverse |
|------|-------|-------|-----------|---------|
| related_to | Related to | dashed | bidirectional | — |
| part_of | Part of | solid | forward | contains |
| depends_on | Depends on | solid | forward | required_by |
| extends | Extends | solid | forward | extended_by |
| references | References | dotted | forward | referenced_by |
| supports | Supports | solid | forward | supported_by |
| contradicts | Contradicts | solid | bidirectional | — |
| alternative_to | Alternative to | dashed | bidirectional | — |

Plus 5 auto-seeded inverses (\`required_by\`, \`contains\`, \`extended_by\`, \`referenced_by\`, \`supported_by\`) linked via \`inverse_of_id\` so the chunk detail page shows the right label when traversing from either endpoint.`,
        tags: ["connections", "drizzle"]
    },
    {
        name: "schema-catalogs",
        title: "Catalog tables: chunk_type + connection_relation",
        type: "schema",
        summary: "DB-driven vocabularies for chunk kinds and relation kinds. Per-user/codebase scoping + builtins + CRUD UI.",
        content: `Two catalog tables that mirror the same pattern:

\`\`\`
chunk_type           connection_relation
├── id (slug, PK)    ├── id (slug, PK)
├── label            ├── label
├── description      ├── description
├── icon             ├── arrow_style: solid|dashed|dotted
├── color            ├── direction: forward|bidirectional
├── examples (jsonb) ├── color
├── display_order    ├── inverse_of_id (self-fk)
├── built_in         ├── display_order
├── user_id (null=global)    ├── built_in
└── codebase_id (null=global)├── user_id / codebase_id
                             └── ...
\`\`\`

Builtins are seeded with \`built_in = true\`; the CRUD UI at \`/settings/vocabulary\` rejects edits to these. Per-user or per-codebase rows can be added freely.`,
        rationale: "Before the catalog tables, adding a chunk type meant editing 5+ hardcoded maps (icon, color, legend, filter, seed). Now it's a DB row and the UI picks it up on next query.",
        consequences: "The UI must tolerate an unknown icon string gracefully (fall back to FileText). The fixture loader rejects tags referring to missing catalog rows.",
        tags: ["catalog-driven", "vocabulary", "migration"]
    },
    {
        name: "schema-plan",
        title: "Database Schema: plan + plan_task",
        type: "schema",
        summary: "Plans are the central unit of work. Tasks, analyze items (risk/assumption/question/chunk/file), requirements links, dependencies, external links.",
        content: `A plan is the top-level container:

- \`plan\` — title, description, status, codebaseId, metadata (JSONB), completedAt
- \`plan_task\` — title, description, acceptance_criteria (JSONB string[]), status, metadata
- \`plan_task_chunk\` — many-to-many task→chunk with relation (context|created|modified)
- \`plan_task_dependency\` — directed dependency edges; auto-unblock on parent done
- \`plan_analyze_item\` — 5 discriminated kinds (chunk/file/risk/assumption/question) with kind-specific metadata
- \`plan_requirement\` — many-to-many link to requirement entities
- \`plan_external_link\` / \`plan_task_external_link\` — outbound URLs (GitHub, Linear, Slack, Figma) with \`system\` slug + label

## Metadata JSONB

Plans and tasks each have a \`metadata\` JSONB column for free-form state:
\`\`\`json
{ "tokenEstimate": 12000, "effortHours": 2, "gpuHours": 0.5, "priorityHint": "p1" }
\`\`\`

Promote a field out of metadata into a first-class column once multiple callers write the same shape. The escape-hatch makes adding experimental state cheap without schema churn.`,
        tags: ["plans", "drizzle"]
    },
    {
        name: "scope-jsonb",
        title: "Chunk scope (JSONB metadata)",
        type: "reference",
        summary: "`chunk.scope` is a free-form {key:value} map. Used for arbitrary per-chunk tags that don't warrant a table.",
        content: `\`scope\` is a JSONB column on \`chunk\`. Common uses:

- \`{"layer": "service"}\` — which architectural layer the chunk describes
- \`{"lang": "sql"}\` — content language when titles aren't enough
- \`{"sourceUrl": "..."}\` — where the chunk came from if not native to fubbik

Queries use the \`@>\` operator:

\`\`\`sql
SELECT * FROM chunk WHERE scope @> '{"layer":"service"}'::jsonb;
\`\`\`

## Promotion rule

When three or more chunks share the same scope key with the same semantics, consider promoting it to a column or a tag type. Scope is the landing zone for "we don't know yet if this matters" data.`,
        tags: ["chunks", "convention"]
    }
];

// ---------------------------------------------------------------------------
// 4) Frontend
// ---------------------------------------------------------------------------

const FRONTEND: ChunkFixture[] = [
    {
        name: "tanstack-start",
        title: "TanStack Start for SSR + routing",
        type: "reference",
        summary: "File-based routes under apps/web/src/routes, createFileRoute per file, SSR via entry-server.ts.",
        content: `The web app uses TanStack Start — file-based routing, SSR, and React Query integration.

## Route files

\`apps/web/src/routes/chunks.$chunkId.tsx\` → \`/chunks/:chunkId\`. Each file exports a \`Route\` made by \`createFileRoute\`:

\`\`\`ts
export const Route = createFileRoute("/chunks/$chunkId")({
    component: ChunkDetail,
    validateSearch: search => ({ tab: search.tab as string | undefined }),
    beforeLoad: async () => {
        let session = null;
        try { session = await getUser(); } catch {}
        return { session };
    }
});
\`\`\`

## Adding a route

1. Create the file. TanStack plugin auto-generates \`routeTree.gen.ts\`.
2. If URLs are under a nested path, use dotted file names: \`settings.vocabulary.tsx\` → \`/settings/vocabulary\`.
3. Touch \`vite.config.ts\` if the dev server doesn't pick up a brand-new route file — a known plugin limitation.

## SSR

\`apps/web/src/entry-server.ts\` is the SSR entry point. \`apps/web/node-server.js\` is a custom Node wrapper used for certain deployments.`,
        tags: ["tanstack", "typescript", "reference"]
    },
    {
        name: "base-ui-render",
        title: "Use render prop, not asChild",
        type: "convention",
        summary: "shadcn-ui is built on base-ui, not Radix. Dialog triggers and menu items use `render={<Component/>}` — not `asChild`.",
        content: `The shadcn-ui components in this project are built on \`@base-ui/react\`, which uses the **render prop** pattern (accepting a React element), not Radix's \`asChild\`.

## Correct:

\`\`\`tsx
<DropdownMenuTrigger render={<Button variant="ghost" />}>
    Menu
</DropdownMenuTrigger>
\`\`\`

## Wrong (will error or be ignored):

\`\`\`tsx
<DropdownMenuTrigger asChild>
    <Button variant="ghost">Menu</Button>
</DropdownMenuTrigger>
\`\`\`

## Exception

\`DropdownMenuSeparator\` and \`DropdownMenuLabel\` in this project use plain HTML elements (not base-ui primitives) to avoid base-ui's \`Menu.Group\` context requirement. Use them directly.`,
        rationale: "Using asChild on a base-ui component silently produces the wrong DOM and breaks focus management.",
        tags: ["base-ui", "render-prop", "convention"]
    },
    {
        name: "feature-structure",
        title: "Frontend feature-based structure",
        type: "convention",
        summary: "Routes in apps/web/src/routes/, feature-scoped components in apps/web/src/features/<feature>/, shared UI in components/ui.",
        content: `Frontend code is organised by feature rather than by technical concern:

\`\`\`
apps/web/src/
├── routes/                  — file-based pages
├── features/
│   ├── chunks/             — chunk list, detail, editor, related
│   ├── graph/              — graph-view, nodes, layout, filter dialog
│   ├── plans/              — plan list, detail, task board
│   ├── auth/               — login form, user menu
│   ├── vocabularies/       — catalog hooks (useChunkTypes, etc.)
│   └── ...
├── components/ui/           — shared primitives (shadcn + base-ui)
└── hooks/                   — cross-feature hooks (use-debounced-value, etc.)
\`\`\`

Shared page scaffolding lives in \`components/ui/page.tsx\`:
- \`PageContainer\` (maxWidth: 3xl|4xl|5xl|6xl)
- \`PageHeader\` (icon, title, description, count, actions)
- \`PageLoading\` (skeleton list)
- \`PageEmpty\` (icon, title, description, action)

Avoid importing feature X components into feature Y. If the component is genuinely shared, promote it to \`components/\`.`,
        tags: ["typescript", "convention", "architecture"]
    },
    {
        name: "react-flow-graph",
        title: "Graph rendering with React Flow",
        type: "reference",
        summary: "@xyflow/react drives the force-directed graph at /graph. Nodes + edges come from a custom layout worker.",
        content: `The \`/graph\` page uses \`@xyflow/react\` (React Flow) as the rendering primitive. Layout is computed off the main thread in a Web Worker (\`layout.worker.ts\`) running a custom force simulation (\`force-layout.ts\`).

## Node types

- \`chunk\` — default node type; uses \`GraphNode\` component
- \`group\` — transparent overlay boxes for tag/type/codebase grouping

Both components are memoised via \`React.memo\` with shallow data-equality so hovering or selecting one node doesn't re-render the other 200.

## Performance

- \`onlyRenderVisibleElements\` prop — React Flow skips DOM for off-screen nodes
- \`React.memo\` with shallow data-equality on \`GraphNode\` + \`GraphGroupNode\`
- Layout worker runs off the main thread; main-thread fallback behind a settings toggle
- Layout position cache in sessionStorage keyed on sorted chunk + edge IDs + grouping mode
- Skip Phase 1 simulation when only the grouping changes (regroup path, 400 ms → 1 ms)`,
        tags: ["react-flow", "graph", "performance", "tanstack"]
    },
    {
        name: "graph-layout",
        title: "Graph force layout",
        type: "reference",
        summary: "Two-phase simulation: position group centers, hard-separate, then grid-place chunks within each group.",
        content: `\`runGroupedLayout\` runs three phases:

### Phase 1 — group-level simulation

Treat each tag/type/codebase group as a super-node. Run 200 iterations of spring-repulsion with an annealing temperature. Spring length is \`max(350, avgRelationLen * 1.5, radiusA + radiusB + pad)\` so big connected groups don't get dragged through each other.

### Phase 1.5 — hard geometric separation

After the soft simulation, walk all pairs and push any still overlapping by half the penetration each. Up to 40 passes. This fixes the case where 3+ groups pile at the origin when the springs can't out-pull the repulsion.

### Phase 2 — grid placement

For each group, place its first-wins members in a grid sized to produce a roughly square visual block:

\`\`\`
cols = ceil(sqrt(n * NODE_H / NODE_W))
\`\`\`

Cell size NODE_W=200, NODE_H=72. Grid centre = group centre.

### Post-step (client-side, in graph-view)

Re-space the grid with **measured** per-column widths and per-row heights. Long chunk titles can exceed the default cell; the client-side respacing prevents overlap when the grid is drawn.`,
        rationale: "Separating phases lets us skip the expensive phase 1 when only grouping changes (warm-start from previous positions). Client-side respacing is needed because the worker can't see measured DOM sizes.",
        tags: ["graph", "performance", "reference"]
    },
    {
        name: "graph-filter-dialog",
        title: "Pre-filter dialog + prefilter pipeline",
        type: "reference",
        summary: "Dialog opens on /graph entry. Tags, types, focus+depth, group-by. URL params drive everything; reopening stays idempotent.",
        content: `On entering \`/graph\` with no filter params, the filter dialog auto-opens. Fields:

- **Tags** — multi-select from the tag catalog
- **Types** — multi-select from chunk_type catalog (pills show color + count + tooltip)
- **Focus chunk + depth** — BFS N hops outward from one chunk
- **Group by** — tag | type | codebase | none

## Apply flow

1. Dialog's Apply navigates to \`/graph?tags=a,b&types=document&groupBy=type\`
2. \`validateSearch\` parses the params into a typed \`GraphSearch\` object
3. \`prefilter\` useMemo reads the params into \`GraphFilterValues\`
4. \`filteredGraph\` useMemo calls \`applyPrefilter(data, spec)\` — pure function shared with the dialog's live-preview
5. Graph re-renders with the filtered set; URL is the source of truth

## Live preview

The dialog uses the same \`applyPrefilter\` function on the graph's already-fetched data, so the count ("113 of 138 chunks will render") is authoritative. Apply is disabled when the count is zero.

## Top-left inline panel

\`GraphFilterForm\` is extracted as a shared component. The dialog buffers edits until Apply; the top-left inline panel commits every change to the URL immediately. One source of truth, two commit behaviors.`,
        tags: ["graph", "reference", "fixture-dsl"]
    },
    {
        name: "mermaid-export",
        title: "Export graph as Mermaid",
        type: "guide",
        summary: "Export button in graph toolbar → modal with text, copy, download .mmd, LR/TB direction toggle, truncation warning at 100 nodes.",
        content: `Click the settings gear (top-right of the graph) → **Export as Mermaid**. The modal shows:

- Current node + edge count
- LR (Left → Right) / TB (Top → Bottom) direction toggle
- Generated Mermaid text in a textarea (selectable, readonly)
- **Copy** button (clipboard) and **Download .mmd** button
- Amber warning when exceeding 100 nodes (Mermaid layout degrades past that)

## Output format

\`\`\`
flowchart LR
    nabc123["Chunk Title"]
    ndef456["Another Chunk"]
    nabc123 -->|part_of| ndef456
\`\`\`

The generator is a pure function (\`buildMermaidFromGraph\`) that reads visible React Flow nodes + edges; the server also exposes \`GET /api/graph/export/mermaid\` for CLI/MCP integrations.

## What's exported

Only the current visible set — the prefilter is respected. Group nodes are filtered out of the chunk count.`,
        tags: ["mermaid-export", "graph"]
    }
];

// ---------------------------------------------------------------------------
// 5) AI features
// ---------------------------------------------------------------------------

const AI: ChunkFixture[] = [
    {
        name: "ollama",
        title: "Ollama for local AI",
        type: "reference",
        summary: "Local LLM via Ollama. llama3.2 for enrichment, nomic-embed-text for 768-dim embeddings. Configured via OLLAMA_URL.",
        content: `Fubbik uses Ollama (not a cloud API) for all AI features. This is deliberate — **no chunk content ever leaves the user's machine**.

## Models

- **Generation**: \`llama3.2\` — used for summary generation, alias extraction, notAbout (exclusion) term generation
- **Embeddings**: \`nomic-embed-text\` — 768-dimensional vectors stored in pgvector

## Installation

\`\`\`
ollama pull llama3.2
ollama pull nomic-embed-text
\`\`\`

\`OLLAMA_URL\` env var defaults to \`http://localhost:11434\`. Features requiring Ollama fail gracefully (log + skip) when unavailable so the rest of fubbik keeps working.

## Concurrency

Enrichment runs with concurrency 3 via Effect.all to avoid flooding the local Ollama process.`,
        tags: ["ollama", "semantic-search", "reference"]
    },
    {
        name: "embeddings",
        title: "Vector embeddings with pgvector",
        type: "reference",
        summary: "chunk.embedding is vector(768). Auto-refreshed on title/content edit. Semantic search uses cosine distance <=>.",
        content: `\`chunk.embedding\` is a \`vector(768)\` column populated by Ollama's nomic-embed-text model.

## Refresh policy

- On \`title\` or \`content\` update, an embedding refresh is enqueued (fire-and-forget).
- \`embedding_updated_at\` tracks freshness; the \`/knowledge-health\` page surfaces chunks whose embedding lags their content.

## Queries

Cosine distance operator \`<=>\` returns a value in [0, 2]. Convert to similarity:

\`\`\`sql
SELECT id, 1 - (embedding <=> $1::vector) AS similarity
FROM chunk
WHERE embedding IS NOT NULL
ORDER BY embedding <=> $1::vector
LIMIT 10
\`\`\`

## Index

pgvector HNSW index on \`embedding\` via migration. Postgres 16+ required.`,
        tags: ["embeddings", "pgvector", "semantic-search", "postgres"]
    },
    {
        name: "semantic-search",
        title: "Semantic search",
        type: "reference",
        summary: "GET /api/chunks/search/semantic?q=... returns chunks by embedding similarity. CLI: fubbik search 'X' --semantic.",
        content: `Semantic search compares a query embedding to each chunk's embedding and returns the top matches by cosine similarity.

## Endpoints

- \`GET /api/chunks/search/semantic?q=\` — returns \`{ id, title, summary, type, aliases, scope, similarity }[]\`
- \`GET /api/chunks/:id/neighbors?k=10\` — k nearest neighbors in embedding space, excluding self

## CLI

\`\`\`
fubbik search "how do we handle auth" --semantic
\`\`\`

## Rate limits

30 semantic queries per user per minute. The embedding call to Ollama is the slow part; the pgvector query is sub-millisecond.`,
        tags: ["semantic-search", "search", "reference"]
    },
    {
        name: "enrichment",
        title: "Chunk enrichment (summary, aliases, notAbout)",
        type: "reference",
        summary: "Ollama fills `summary` (1-2 sentences), `aliases` (synonyms), `not_about` (exclusion terms). Runs on create + on title/content edit.",
        content: `On every chunk create or title/content update, a background enrichment job runs:

1. Generate \`summary\` — 1–2 sentence TL;DR (via llama3.2)
2. Extract \`aliases\` — synonyms / alternative names the chunk might be searched under
3. Extract \`not_about\` — terms that would *incorrectly* match this chunk (negative disambiguation)

All three feed into search quality: aliases widen the match set; notAbout lets search exclude chunks that would otherwise look relevant.

## Running on demand

\`POST /api/chunks/:id/enrich\` re-runs enrichment explicitly.
\`fubbik enrich --all\` walks every chunk at concurrency 3.`,
        tags: ["enrichment-ai", "ollama", "chunks"]
    },
    {
        name: "semantic-neighbors",
        title: "Semantic neighbors view",
        type: "guide",
        summary: "Each chunk detail page shows its 10 nearest neighbors by embedding distance. Helps find near-duplicates and related context.",
        content: `On \`/chunks/:id\` there's a **Semantic neighbors** section listing the chunk's 10 nearest embedding neighbors with a similarity percentage.

## Uses

- **Duplicate hunting** — chunks at 95 %+ similarity are likely the same knowledge captured twice
- **Gentle gradations** — tags reflect what humans labelled; embeddings capture what's actually similar regardless of labels
- **Context discovery** — read X, see it's neighbors with Y, widen your reading

## Backing query

\`GET /api/chunks/:id/neighbors?k=10\` with pgvector's \`<=>\` operator. Returns empty with a note when the chunk has no embedding (enrichment not run).`,
        tags: ["neighbors", "semantic-search", "chunks"]
    },
    {
        name: "context-export",
        title: "Context export: token-budgeted chunk bundle for AI",
        type: "reference",
        summary: "GET /api/chunks/export/context?forPath=...&maxTokens=... builds an LLM-ready chunk bundle scored by relevance + health.",
        content: `\`/api/chunks/export/context\` produces a single text bundle suitable for dropping into an LLM context window.

Scoring combines:
1. **Type weight** — conventions and schemas rank above notes
2. **Health score** — well-connected, fresh, complete chunks rank higher
3. **Review status** — approved > reviewed > draft
4. **File relevance** (when \`forPath\` is set) — boosts chunks whose \`applies_to\` globs or \`file_ref\` paths match

The greedy selection fills a token budget. Also powers \`GET /api/chunks/export/claude-md\` which writes a project-level \`CLAUDE.md\` from chunks tagged \`claude-context\`.

## CLI

\`\`\`
fubbik context --for src/features/auth/login.tsx
fubbik context-for src/features/auth/login.tsx --include-deps
fubbik context-dir apps/web/src/routes
fubbik sync-claude-md --watch
\`\`\``,
        tags: ["context-export", "claude-md", "cli", "semantic-search"]
    }
];

// ---------------------------------------------------------------------------
// 6) Knowledge framework
// ---------------------------------------------------------------------------

const KNOWLEDGE: ChunkFixture[] = [
    {
        name: "chunks-concept",
        title: "Chunks as atomic knowledge units",
        type: "document",
        summary: "A chunk is one atomic idea: a decision, a convention, a schema, a runbook step. Small and focused beats big and vague.",
        content: `A **chunk** is the smallest unit of knowledge fubbik represents — one atomic idea with a title and body. The goal is "one decision per chunk" or "one concept per chunk", not "one page per chunk".

## Good chunks

- "Always use render prop, not asChild" (one convention, one rule)
- "Effect error handling in the service layer" (one pattern)
- "Plan status: draft → analyzing → ready → in_progress → completed → archived" (one machine)

## Chunks that should be split

- "Backend architecture" covering 10 patterns → should be 10 chunks with connections
- "Our stack" listing 15 technologies → 15 chunks (or a doc with links)

## Types

Seven builtin chunk types: note, document, guide, reference, schema, checklist, convention. Per-codebase custom types possible via \`/settings/vocabulary\`.`,
        rationale: "Atomic chunks are reusable across contexts; composite pages aren't. Fine granularity also gives better semantic search precision.",
        consequences: "Requires discipline when importing a long doc — split it, or import as a document with auto-chunking.",
        tags: ["chunks", "convention", "onboarding"]
    },
    {
        name: "connections-concept",
        title: "Typed connections + inverse relations",
        type: "document",
        summary: "Connections are directed edges with a typed relation. Inverse pairs (depends_on ↔ required_by) let the graph read from either direction.",
        content: `A **connection** is a directed edge from one chunk to another with a typed \`relation\` (a slug from the \`connection_relation\` catalog).

## Inverses

Five forward relations have explicit inverses:

| Forward | Inverse |
|---------|---------|
| depends_on | required_by |
| part_of | contains |
| extends | extended_by |
| references | referenced_by |
| supports | supported_by |

Stored via \`inverse_of_id\` self-FK on \`connection_relation\`. The chunk detail page renders "A depends_on B" on A's page as "B is required_by A" when viewed from B — same edge, direction-appropriate label.

## Bidirectional relations

\`related_to\`, \`contradicts\`, \`alternative_to\` are symmetric. They render identically from either endpoint and don't need an inverse.

## Guidance

- Use specific relations where possible. \`part_of\` and \`depends_on\` are high-value.
- Reserve \`related_to\` for weak associations; overuse dilutes the graph.
- \`contradicts\` is uncommon but valuable — captures design debates.`,
        tags: ["connections", "convention", "reference"]
    },
    {
        name: "tags-concept",
        title: "Tags + tag types",
        type: "reference",
        summary: "Tags are labels grouped under tag types. Tag types provide color + grouping; tags provide filterable labels.",
        content: `Tags live in two layers:

- \`tag_type\` — broad category: \`feature\`, \`techstack\`, \`infrastructure\`, \`pattern\`, etc. Carries a color.
- \`tag\` — specific label: \`authentication\`, \`elysia\`, \`docker\`, \`repository-pattern\`. Belongs to at most one tag type (nullable FK).

The graph can group nodes by tag type (all nodes tagged with anything under \`feature\` cluster together) or show individual tag-name groups.

## When to tag vs type vs connect

- **Type** (chunk_type): what kind of knowledge (schema, convention, note…)
- **Tag**: cross-cutting concern (authentication, performance)
- **Connection**: relationship between two specific chunks

A chunk is often 1 type + several tags + several connections.`,
        tags: ["chunks", "reference"]
    },
    {
        name: "health-scores",
        title: "Chunk health scores",
        type: "reference",
        summary: "Per-chunk score in [0, 100]: freshness 25 + completeness 25 + richness 25 + connectivity 25.",
        content: `Every chunk gets a computed-on-demand health score out of 100:

- **Freshness (0–25)** — days since last update; newer = higher
- **Completeness (0–25)** — presence of rationale / alternatives / consequences fields
- **Richness (0–25)** — content length + AI enrichment fields (summary, aliases, notAbout)
- **Connectivity (0–25)** — number of outgoing + incoming connections

Shown as a badge on the chunk detail page. \`/knowledge-health\` aggregates across the KB to surface thin, orphan, or stale chunks.

## Gaming the score is fine

High-score chunks are genuinely more usable — they're findable (rich + connected) and current (fresh). You can't fake your way to a useful KB.`,
        tags: ["health", "chunks"]
    },
    {
        name: "staleness-detection",
        title: "Staleness detection",
        type: "reference",
        summary: "chunk_staleness flags chunks needing attention. Reasons: file_changed, age, diverged_duplicate. Dismissable + suppressible.",
        content: `\`chunk_staleness\` is a table of flags indicating a chunk may need attention:

- \`reason = 'file_changed'\` — a referenced file has changed since the chunk's \`updated_at\`
- \`reason = 'age'\` — chunk hasn't been touched in > N days (configurable, default 90)
- \`reason = 'diverged_duplicate'\` — two chunks start near-identical and drift apart over time

Flags can be **dismissed** (soft-delete) when the user has reviewed and confirmed the chunk is still correct, or **suppressed** (for duplicate pairs) to mark an ongoing tolerated divergence.

## Surfaces

- Dashboard "Attention Needed" widget
- Nav badge on Dashboard link with the undismissed count
- Amber banner on the chunk detail page

## Scans

\`POST /api/chunks/stale/scan-age\` triggers an age-based scan. File-change detection is incremental — each codebase tracks the last git SHA it scanned in \`staleness_scan\`.`,
        tags: ["staleness", "health"]
    },
    {
        name: "applies-to-refs",
        title: "applies_to globs + file_ref paths",
        type: "reference",
        summary: "Two ways chunks link to files: glob patterns (applies_to) for broad coverage, explicit paths (file_ref) for precision.",
        content: `A chunk can be linked to files in two ways:

### \`chunk_applies_to\` — glob patterns

\`\`\`
chunk "Effect error handling" applies_to "packages/api/src/**/service.ts"
chunk "Architecture overview" applies_to "**/*"
\`\`\`

Broad coverage. Used by \`fubbik context-for <path>\` to surface "what applies to this area".

### \`chunk_file_ref\` — explicit paths

\`\`\`
chunk "Seed architecture" file_ref "packages/db/src/seed/index.ts" relation="documents"
\`\`\`

Precise bidirectional link. Used by \`/api/file-refs/lookup?path=...\` for reverse queries ("which chunks reference this exact file?").

## Use both

Globs + explicit refs together cover breadth and depth. The graph density map (\`/density\`) aggregates both to visualize knowledge coverage per folder.`,
        tags: ["chunks", "reference"]
    },
    {
        name: "decision-context",
        title: "Decision context: rationale / alternatives / consequences",
        type: "convention",
        summary: "ADR-style fields on chunks. Capture the why, what else was considered, and what the decision costs — not just the decision.",
        content: `Chunks have three optional decision-context fields that mirror the ADR (Architecture Decision Record) pattern:

- \`rationale\` — why this choice
- \`alternatives\` (jsonb string[]) — other options considered and rejected
- \`consequences\` — what this choice costs (trade-offs, future constraints)

The chunk detail page renders these as an amber "Decision context" aside when any are set.

## When to use

Any chunk of type \`convention\` or \`document\` that represents a choice (as opposed to a description of an externality). Schemas, runbooks, and notes generally don't need them.

## Value over time

Fresh decisions are obvious; old decisions aren't. Decision context ages well — six months in, \`rationale\` is often the only way to remember why we chose Effect over try/catch everywhere.`,
        tags: ["convention", "chunks"]
    }
];

// ---------------------------------------------------------------------------
// 7) Plans + requirements
// ---------------------------------------------------------------------------

const PLANS_REQS: ChunkFixture[] = [
    {
        name: "plans-overview",
        title: "Plans: the central unit of work",
        type: "document",
        summary: "A plan holds description, linked requirements, analyze items (chunks/files/risks/assumptions/questions), and enriched tasks.",
        content: `A **plan** captures a piece of work end-to-end:

- \`title\`, \`description\` (markdown), \`status\` (draft/analyzing/ready/in_progress/completed/archived — labels only, ungated)
- **Requirements** — many-to-many links to BDD \`requirement\` entities for traceability
- **Analyze items** — structured notes in 5 kinds (chunk, file, risk, assumption, question)
- **Tasks** — enriched units of work with title, description, acceptance_criteria, dependencies, metadata

## Sub-features

- **Task dependencies** (\`plan_task_dependency\`) — marking a task \`done\` auto-unblocks dependents in \`blocked\` state
- **Task ↔ chunk links** (\`plan_task_chunk\`) — context / created / modified relations
- **Plan ↔ requirement links** — requirements roll up to plans for a single traceability view
- **Metadata JSONB** (plan + task) — free-form \`{ tokenEstimate, effortHours, … }\`
- **External links** — GitHub / Linear / Slack URLs attached to plans and tasks

## UI

- \`/plans\` — list with status pills + task progress
- \`/plans/new\` — simple title + description form
- \`/plans/:id\` — sticky header + four sections (description, requirements, analyze, tasks)

## CLI

\`\`\`
fubbik plan create "Title"
fubbik plan list
fubbik plan show <id>
fubbik plan status <id> in_progress
fubbik plan add-task <planId> "Task title"
fubbik plan task-done <planId> <taskId>
fubbik plan link-requirement <planId> <reqId>
\`\`\``,
        tags: ["plans", "reference"]
    },
    {
        name: "analyze-items",
        title: "Plan analyze items: structured reflection",
        type: "reference",
        summary: "Five kinds: chunk, file, risk (severity), assumption (verified flag), question (answer). Kind-specific metadata in JSONB.",
        content: `\`plan_analyze_item\` is a discriminated table holding five kinds of structured pre-implementation notes:

- **chunk** — "this existing chunk is relevant to the plan" (references chunkId)
- **file** — "this file will be touched" (filePath + optional line range in metadata)
- **risk** — "this could go wrong" (severity: low|medium|high in metadata)
- **assumption** — "we're assuming X" (verified: bool in metadata)
- **question** — "we still need to answer X" (answer: string in metadata once resolved)

Each row has \`kind\`, \`order\`, \`chunkId\` (for kind=chunk), \`filePath\` (for kind=file), \`text\`, and \`metadata\` JSONB for kind-specific fields.

The plan detail page groups items by kind. CRUD:

- \`GET /api/plans/:id/analyze\`
- \`POST /api/plans/:id/analyze\`
- \`PATCH /api/plans/:id/analyze/:itemId\`
- \`DELETE /api/plans/:id/analyze/:itemId\`
- \`POST /api/plans/:id/analyze/reorder\``,
        tags: ["plans", "reference"]
    },
    {
        name: "requirements-bdd",
        title: "Requirements: BDD-style Given/When/Then",
        type: "reference",
        summary: "requirement.steps is jsonb[] of {keyword, text} where keyword ∈ given|when|then|and|but. Priority: must|should|could|wont.",
        content: `Requirements capture behavior in BDD form:

\`\`\`json
[
    { "keyword": "given", "text": "a fresh fubbik install with seed data" },
    { "keyword": "when",  "text": "the user visits /graph" },
    { "keyword": "then",  "text": "the filter dialog appears with 7 chunk types listed" }
]
\`\`\`

- \`title\`, \`description\`, \`steps\` (jsonb array)
- \`priority\` — must | should | could | wont (MoSCoW)
- \`status\` — passing | failing | untested (auto-updates to passing on implementation session completion)
- \`codebaseId\` — scope to one codebase
- \`useCaseId\` — link to a parent use case for a 3-level narrative (use case → requirement → implementation)

## UI

- \`/requirements\` tabbed: Requirements list / Plans list / Traceability dashboard
- \`/requirements/:id\` — detail with plan coverage, BDD steps, export
- Full traceability: use case → requirement → plan → session`,
        tags: ["requirements", "reference"]
    },
    {
        name: "task-dependencies",
        title: "Task dependencies + auto-unblock",
        type: "reference",
        summary: "plan_task_dependency links task → dependsOn. On marking a task done, dependents in blocked state flip to pending.",
        content: `\`plan_task_dependency\` is a directed-edge table:

\`\`\`
taskId --depends on--> dependsOnTaskId
\`\`\`

Typical flow:

1. Task B is created with dependency on A.
2. B starts life as \`pending\` or \`blocked\`.
3. When A flips to \`done\`, the service calls \`unblockDependentsOf(A)\` which updates any dependents in status \`blocked\` → \`pending\`.

Does not cascade beyond one hop. If B → A is unblocked but C → B, C stays blocked until B also goes done.

## Cycle protection

The repository layer rejects cycles — creating a dependency that would form a cycle returns \`ValidationError\`.`,
        tags: ["plans", "reference"]
    },
    {
        name: "plan-metadata",
        title: "Plan + task metadata JSONB",
        type: "reference",
        summary: "Free-form {key: value} on plan and plan_task. Holds tokenEstimate, effortHours, blockedOn, deployment flags, etc.",
        content: `Both \`plan\` and \`plan_task\` carry a \`metadata\` JSONB column. It's a deliberate escape-hatch for fields that don't warrant a column yet.

## Examples

\`\`\`json
// plan.metadata
{ "tokenEstimate": 15000, "deployEnv": "staging" }

// plan_task.metadata
{ "effortHours": 2, "gpuHours": 0.5, "requiresReview": true, "reviewerHint": "@alice" }
\`\`\`

## Promotion rule

When three or more plans or tasks share the same metadata key with the same semantics, promote it to a first-class column (with a migration). Priority + effort + dueAt are the next likely promotions.`,
        tags: ["plans", "reference"]
    },
    {
        name: "external-links",
        title: "External links on plans + tasks",
        type: "reference",
        summary: "plan_external_link and plan_task_external_link hold GitHub/Linear/Slack/Figma URLs with system slug + optional label.",
        content: `Both plans and tasks can carry outbound URLs:

\`\`\`
plan_external_link (planId, system, url, label, order)
plan_task_external_link (taskId, system, url, label, order)
\`\`\`

\`system\` is a free-form slug (\`github\`, \`linear\`, \`slack\`, \`figma\`, \`url\`) used by the UI to pick an icon.

## Endpoints

- \`GET /POST /DELETE /api/plans/:id/links[/:linkId]\`
- Same prefix tree under \`/tasks/:taskId/links\`

## Value

Ends the "where was that ticket?" dance. A plan becomes a hub that points out to the systems the work lives in.`,
        tags: ["plans", "reference"]
    },
    {
        name: "activity-audit",
        title: "Audit trail via the activity log",
        type: "reference",
        summary: "Plan + task create / update / status-change / delete emit to activityLog. Filter the /api/activity feed by entityType.",
        content: `Mutations to plans and tasks emit rows into the existing \`activityLog\` table:

\`\`\`
{ entityType: 'plan' | 'plan_task', entityId, entityTitle, action, codebaseId, userId, createdAt }
\`\`\`

Actions: \`created\`, \`updated\`, \`status_changed\`, \`deleted\`.

## Viewing

- \`/activity\` — global feed across all entity types
- \`GET /api/activity?entityType=plan\` — filtered
- A per-plan side panel on \`/plans/:id\` (future work) can read with \`entityId=<planId>\`

## Why not a new table

The existing \`activityLog\` was already carrying chunk + tag + codebase events. Extending entityType keeps the audit unified; per-entity tables would fragment the feed.`,
        tags: ["activity", "plans"]
    }
];

// ---------------------------------------------------------------------------
// 8) Integrations
// ---------------------------------------------------------------------------

const INTEGRATIONS: ChunkFixture[] = [
    {
        name: "mcp-server",
        title: "MCP server for AI agent integration",
        type: "reference",
        summary: "Model Context Protocol server at packages/mcp/ exposes fubbik tools to Claude, Cursor, and other MCP clients.",
        content: `\`packages/mcp\` implements a Model Context Protocol (MCP) server, giving AI agents structured access to fubbik.

## Tool domains

- \`tools.ts\` — core chunk/codebase queries
- \`session-tools.ts\` — implementation-session lifecycle
- \`plan-tools.ts\` — plan CRUD, import_plan_markdown, begin_implementation, mark_plan_step
- \`requirement-tools.ts\` — requirement CRUD + traceability
- \`suggestion-tools.ts\` — AI-facing suggestions
- \`context-tools.ts\` — get_context, get_context_for_task

Each file exports \`registerXTools(server: McpServer)\`. Tools use a shared \`apiFetch\` helper pointed at the local fubbik server.

## Notable tools

- \`create_plan\` / \`create_plan_from_requirements\`
- \`import_plan_markdown\` — bulk import a markdown plan file
- \`begin_implementation\` / \`mark_plan_step\` — drive a session through a plan
- \`sync_claude_md\` — regenerate a project's CLAUDE.md from tagged chunks`,
        tags: ["mcp", "semantic-search"]
    },
    {
        name: "vscode-ext",
        title: "VS Code / Cursor extension",
        type: "reference",
        summary: "apps/vscode — standalone extension. Sidebar, status bar, quick-add, chunk-detail webviews. Talks HTTP to the local fubbik server.",
        content: `\`apps/vscode\` is a standalone package. It does **not** import from other fubbik packages — it talks to the fubbik server via HTTP to stay decoupled (and so it can ship as a single .vsix bundle).

## Features

- **Sidebar** — filterable chunk list scoped to the active codebase
- **Status bar** — chunk count, click to open the search dropdown
- **Quick add** — command palette action to create a chunk from the current editor selection
- **Chunk detail webview** — full detail pane, editable
- **File-aware surfacing** — current file path → relevant chunks via \`/api/context/for-file\`

## Build

\`\`\`
cd apps/vscode && node esbuild.mjs
\`\`\`

## Configuration

Settings: \`fubbik.serverUrl\` (default \`http://localhost:3000\`) and \`fubbik.webAppUrl\` (default \`http://localhost:3001\`).

## Debug

\`code --extensionDevelopmentPath=/Users/pontus/projects/fubbik/apps/vscode /Users/pontus/projects/fubbik\``,
        tags: ["vscode"]
    },
    {
        name: "cli-overview",
        title: "CLI overview",
        type: "reference",
        summary: "`fubbik` command. Init, add/get/list/search, link/unlink, context export, plans, hooks. Auto-detects codebase via git remote.",
        content: `\`fubbik\` is the Commander.js CLI in \`apps/cli/\`. It auto-detects the current codebase from the git remote, so running \`fubbik list\` in a repo shows only that codebase's chunks by default.

## Chunk CRUD

\`\`\`
fubbik add "Title" --type convention --tags performance
fubbik add -i                        # opens $EDITOR for content
fubbik add --template "Architecture Decision"
fubbik quick "Title"                 # one-liner, stdin pipe for content
fubbik get <id>
fubbik list --tag convention --json
fubbik search "query" --semantic
fubbik update <id> --title "..."
fubbik remove <id>
\`\`\`

## Connections

\`\`\`
fubbik link <sourceId> <targetId> --relation depends_on
fubbik unlink <connectionId>
\`\`\`

## Context export

\`\`\`
fubbik context --for src/routes/auth.tsx
fubbik context-for apps/web/src/routes/graph.tsx --include-deps
fubbik context-dir packages/api/src
fubbik sync-claude-md
\`\`\`

## Plans, hooks, import

See \`fubbik --help\` for the full list. Relevant: \`fubbik hooks install\` (git pre-commit), \`fubbik import <path>\` (markdown or folder).`,
        tags: ["cli", "reference"]
    },
    {
        name: "claude-md-sync",
        title: "CLAUDE.md generation from tagged chunks",
        type: "guide",
        summary: "Chunks tagged `claude-context` roll up into .claude/CLAUDE.md. Run `fubbik sync-claude-md` or use --watch.",
        content: `\`fubbik sync-claude-md\` generates a project-level \`CLAUDE.md\` from chunks tagged with \`claude-context\` (or any tag you point it at).

## Invocation

\`\`\`
fubbik sync-claude-md
fubbik sync-claude-md --watch       # regenerate on chunk changes
\`\`\`

## Where

Writes to \`.claude/CLAUDE.md\` by default; configurable via \`fubbik.config.json\`.

## Ordering

Chunks are ordered by \`display_order\` (if set via scope) then by type precedence (conventions first), then alphabetically.

## Tagging discipline

The CLAUDE.md is only as good as the tag. Only tag chunks that:
- Capture a decision the team wants to preserve
- Are conventions the AI should respect
- Are references the AI should read before writing in this area`,
        tags: ["claude-md", "cli", "semantic-search"]
    },
    {
        name: "git-hooks",
        title: "Git pre-commit hook",
        type: "reference",
        summary: "`fubbik hooks install` adds a pre-commit hook that checks staged files for chunk coverage and flags gaps.",
        content: `\`fubbik hooks install\` writes a pre-commit hook to \`.git/hooks/pre-commit\` that runs:

\`\`\`
fubbik check-files --staged
\`\`\`

Which inspects staged files and flags ones with no associated chunks (via \`applies_to\` or \`file_ref\`). Non-blocking — prints a warning unless \`--strict\`.

## Uninstall

\`\`\`
fubbik hooks uninstall
\`\`\`

## Value

Surfaces "we're modifying an area with no captured knowledge" at commit time, while context is fresh. Developers can either add a chunk immediately or dismiss.`,
        tags: ["git-hooks", "cli"]
    }
];

// ---------------------------------------------------------------------------
// 9) System / performance meta-knowledge
// ---------------------------------------------------------------------------

const SYSTEM: ChunkFixture[] = [
    {
        name: "catalog-pattern",
        title: "Catalog-driven vocabularies",
        type: "convention",
        summary: "chunk_type + connection_relation tables replace hardcoded enums. Builtins + per-user/codebase custom rows + CRUD UI.",
        content: `Instead of hardcoding chunk types and relations in app code, fubbik keeps them in DB tables with a \`built_in\` flag + per-user/codebase scoping.

## Shape

\`\`\`
chunk_type          connection_relation
├── id (slug, PK)   ├── id (slug, PK)
├── label           ├── label
├── description     ├── description
├── icon (string)   ├── arrow_style
├── color           ├── direction
├── examples        ├── inverse_of_id
├── display_order   ├── …
├── built_in        └── built_in
├── user_id          user_id
└── codebase_id      codebase_id
\`\`\`

## Consumption

Frontend: \`useChunkTypes()\` / \`useConnectionRelations()\` hooks with 24-hour staleTime. \`resolveChunkTypeIcon(iconName)\` maps catalog icon strings to Lucide components.

Backend: \`/api/chunk-types\` and \`/api/connection-relations\` (with optional codebaseId). FKs on \`chunk.type\` and \`chunk_connection.relation\` restrict to valid slugs.

## CRUD

\`/settings/vocabulary\` — protects builtins, allows custom types per codebase. Inverse pairs link explicitly via \`inverse_of_id\`.`,
        rationale: "Before the catalog tables, adding a chunk type meant editing 5+ hardcoded maps scattered across the codebase. Now it's a row insert.",
        consequences: "UI must tolerate unknown icon strings (falls back to FileText). Every schema migration that changes an enum slug needs a data backfill.",
        tags: ["catalog-driven", "convention", "vocabulary", "migration"]
    },
    {
        name: "graph-perf",
        title: "Graph rendering performance playbook",
        type: "reference",
        summary: "Four wins shipped: React.memo on nodes, viewport culling, regroup skip, session layout cache. Baseline 540ms → 1ms on groupBy toggle.",
        content: `The \`/graph\` page was re-running a 200-iteration force simulation on every filter toggle. Four compounding fixes brought layout-duration on a 138-chunk graph from ~540 ms → ~1 ms (on cached paths).

## 1. React.memo on GraphNode + GraphGroupNode

Shallow data-equality on \`label\`, \`type\`, \`connectionCount\`, \`tags\`, \`codebaseName\` for nodes; \`label\` + \`color\` for groups. Positions handled by React Flow outside the equality check.

## 2. \`onlyRenderVisibleElements\` on ReactFlow

DOM mount skipped for off-screen nodes. MiniMap + fitView still work because they read from React Flow's internal store, not the DOM.

## 3. Skip Phase 1 on groupBy-only changes

\`runRegroupLayout\` — when the chunk + edge set matches the previous run and only grouping changed, seed new group centers from member centroids of the previous layout, run Phase 1.5 hard-separation, then Phase 2 grid placement. 400 ms → 1 ms.

## 4. Session-scoped layout cache

\`layout-cache.ts\`: hash \`(algorithm, groupingMode + tagGroupMembership, sortedNodeIds, sortedEdgeIds)\`. On hit, skip the worker entirely — set positions directly from cache. LRU-capped at 20 entries, written to sessionStorage.

## Instrumentation

\`seed/graph-timings.ts\` exposes \`mark\` / \`measure\` helpers. Three measures land in the devtools console:
- \`graph-perf:time-to-first-node\`
- \`graph-perf:layout-duration\` (full simulation)
- \`graph-perf:layout-regroup\` (regroup fast-path)
- \`graph-perf:layout-cache-hit\``,
        tags: ["performance", "graph", "react-flow"]
    },
    {
        name: "seed-system",
        title: "Seed system (high-level)",
        type: "document",
        summary: "packages/db/src/seed/ — orchestrator, 14 modules, factories, fixture DSL, scenarios, strict errors, post-seed verify, self-documenting chunks.",
        content: `See the \`seed-system\` + \`self-documenting\` tagged chunks for the deep dive. Big-picture:

- **Orchestrator** \`seed/index.ts\` — parses \`--scenario\`, \`--only\`, \`--skip\`, \`--reset=none\`. MODULE_REGISTRY declares dependency order.
- **14 modules** under \`seed/modules/\` — core, codebases, tags, chunks, connections, file-links, use-cases, requirements, plans, documents, vocabulary, workspaces, collections, self-documenting.
- **Factories** — \`makeChunk\` / \`makeConnection\` / \`makeTag\` / \`makeCodebase\`.
- **Fixture DSL** — \`ChunkFixture\` and \`ConnectionFixture\` types; the loader resolves names → IDs.
- **Scenarios** — \`minimal\` (core + self-doc), \`demo\` (default, everything), \`extended\` (hook for richer fixtures).
- **Strict errors** — \`trySeed()\` wraps every step, logs, re-throws. Broken fixtures fail the seed instead of masking.
- **Post-seed verify** — \`seed/verify.ts\` prints row counts + 3 FK integrity probes.
- **Self-documenting** — the seed itself inserts chunks describing its own architecture, linked via \`applies_to\` globs back to the seed source files.`,
        tags: ["seed-system", "self-documenting", "architecture"]
    },
    {
        name: "note-conventions",
        title: "Convention chunks are sticky",
        type: "note",
        summary: "Every `convention` chunk should be in CLAUDE.md generation and at the top of context-export bundles — that's the point of the type.",
        content: `Treat every \`convention\` chunk as pinned: they should appear in CLAUDE.md generation and near the top of \`context-export\` runs. That's the whole point of the \`convention\` type over plain \`note\`.

## Good convention chunks

- "Use render prop, not asChild" (base-ui gotcha)
- "Services introduce typed errors; routes don't write status codes" (error-handling)
- "Promote a JSONB metadata key to a column only when 3+ callers agree" (schema discipline)

## Bad convention chunks

- "We chose Elysia" (that's a decision/document, not a recurring rule)
- "Thoughts on caching" (too vague for a convention)`,
        tags: ["convention", "chunks"]
    }
];

// ---------------------------------------------------------------------------
// Compose + seed
// ---------------------------------------------------------------------------

const FIXTURES: ChunkFixture[] = [
    ...ARCHITECTURE,
    ...BACKEND,
    ...DATA_MODEL,
    ...FRONTEND,
    ...AI,
    ...KNOWLEDGE,
    ...PLANS_REQS,
    ...INTEGRATIONS,
    ...SYSTEM
];

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];
    if (!codebaseId) throw new Error("chunks module needs the fubbik codebase");
    await loadChunkFixtures(ctx, FIXTURES, { codebaseId });
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(chunk).where(eq(chunk.userId, ctx.userId));
}
