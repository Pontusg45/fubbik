import { resolve } from "path";

import { config } from "dotenv";
import { eq } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

import { user } from "./schema/auth";
import { chunk, chunkConnection } from "./schema/chunk";

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

// Clear existing chunks for dev user
await db.delete(chunk).where(eq(chunk.userId, DEV_USER_ID));

const chunks = [
    {
        id: "c-architecture",
        title: "Project Architecture",
        type: "document",
        tags: ["architecture", "overview", "monorepo"],
        content: `Fubbik is a local-first knowledge framework built as a TypeScript monorepo.

## Structure

- apps/web — TanStack Start frontend (SSR, Tailwind, shadcn-ui)
- apps/server — Elysia backend (REST API, Better Auth, OpenTelemetry)
- apps/cli — Commander.js CLI tool with local-first storage
- packages/api — Shared Elysia API plugin with Eden treaty types
- packages/auth — Better Auth with Drizzle adapter
- packages/config — Shared TypeScript config
- packages/db — Drizzle ORM schema, repositories, and Postgres connection
- packages/env — Environment validation with Arktype + t3-env

## Key Decisions

- Bun as runtime and package manager
- Turborepo for build orchestration
- Arktype for validation (not Zod) — implements Standard Schema v1
- Eden treaty for end-to-end type-safe API client
- Session-based auth with httpOnly cookies
- Effect library for typed error handling in the API layer
- Local-first CLI with bidirectional sync to server

## Data Flow

Web/CLI → Eden treaty client → Elysia routes → Effect services → Drizzle repositories → PostgreSQL

Errors bubble up as Effect failures, get extracted in Elysia onError middleware, and return as typed HTTP responses.`
    },
    {
        id: "c-api-design",
        title: "API Design Patterns",
        type: "reference",
        tags: ["api", "elysia", "eden", "patterns"],
        content: `The API layer uses Elysia with a shared plugin pattern exported from packages/api.

## Eden Treaty

Type-safe client generated from the Elysia server definition. No code generation step — types flow directly from server to client via the Api type export. The client is created with \`treaty<Api>(serverUrl, { fetch: { credentials: "include" } })\`.

## UnwrapEden Pattern

Eden returns \`{ data, error }\` for every call. The \`unwrapEden()\` utility throws on error and returns typed data, removing boilerplate from every API call site.

## Session Resolution

A shared \`resolve()\` middleware extracts the session from request headers via Better Auth. All routes after the middleware receive \`ctx.session\`. Read routes use \`optionalSession()\` (returns null for guests), write routes use \`requireSession()\` (fails with AuthError).

## Error Handling

The \`onError()\` middleware extracts Effect errors from FiberFailure and maps tagged errors to HTTP status codes:
- ValidationError → 400
- AuthError → 401
- NotFoundError → 404
- AiError → 502
- DatabaseError → 500

## Endpoints

- GET /api/health — public health check (no auth)
- GET /api/me — current user info (auth required)
- GET /api/chunks — list chunks with filtering (public read)
- GET /api/chunks/:id — chunk detail with connections (public read)
- POST /api/chunks — create chunk (auth required)
- PATCH /api/chunks/:id — update chunk (auth required)
- DELETE /api/chunks/:id — delete chunk (auth required)
- GET /api/stats — chunk/connection/tag counts (public read)
- GET /api/graph — all chunks and connections for graph view (public read)
- GET /api/tags — all tags with counts (public read)
- POST /api/chunks/import — bulk import (auth required)
- GET /api/chunks/export — export all chunks (public read)
- POST /api/ai/summarize — AI-powered chunk summary (auth required)
- POST /api/ai/suggest-connections — AI connection suggestions (auth required)
- POST /api/ai/generate — generate chunk from prompt (auth required)`
    },
    {
        id: "c-effect-patterns",
        title: "Effect Library Patterns",
        type: "reference",
        tags: ["effect", "error-handling", "patterns"],
        content: `The API layer uses the Effect library for typed, composable error handling.

## Error Definitions

Custom errors extend \`Data.TaggedError\` from Effect, each with a unique \`_tag\` discriminator:
- NotFoundError — carries \`resource: string\` (e.g., "Chunk")
- AuthError — no payload, just signals auth failure
- ValidationError — carries \`message: string\`
- DatabaseError — carries \`cause: unknown\` (wraps Drizzle errors)
- AiError — carries \`cause: unknown\` (wraps AI SDK errors)

## Repository Pattern

Every database function wraps Drizzle queries in \`Effect.tryPromise()\`:
- \`try\` block contains the async Drizzle query
- \`catch\` block always converts to \`DatabaseError({ cause })\`
- Return type is \`Effect<TData, DatabaseError>\`

## Service Layer Composition

Services compose repository calls using Effect pipelines:
- \`Effect.flatMap()\` for sequential operations (get chunk → get connections)
- \`Effect.all()\` with \`{ concurrency: "unbounded" }\` for parallel queries
- \`Effect.tap()\` for side effects (e.g., setting HTTP status)
- \`Effect.map()\` for data transformation

## Route Execution

Routes call \`Effect.runPromise()\` to execute the Effect pipeline. Errors are caught by Elysia's \`onError\` middleware which extracts the tagged error from Effect's FiberFailure wrapper using a special Symbol lookup.

## Why Effect Over Try/Catch

- Errors are part of the type signature — you know exactly what can fail
- Pipelines compose cleanly — no nested try/catch blocks
- Parallel execution built in via Effect.all
- Tagged errors enable exhaustive switch/case in error handlers`
    },
    {
        id: "c-database",
        title: "Database Schema",
        type: "schema",
        tags: ["database", "drizzle", "postgres"],
        content: `PostgreSQL database managed with Drizzle ORM.

## Auth Tables (Better Auth)

- user — id, name, email, emailVerified, image, timestamps
- session — token-based sessions with userId FK, ip/userAgent tracking
- account — OAuth/credential accounts linked to users
- verification — email verification tokens

## Knowledge Tables

- chunk — id (text PK), title, content, type, tags (jsonb string[]), userId FK, createdAt, updatedAt
- chunk_connection — id (text PK), sourceId FK, targetId FK, relation, createdAt
- chunk_version — id (text PK), chunkId FK, version (int), title, content, type, tags, createdAt

## Indexes

- chunk_userId_idx on chunk.userId
- chunk_type_idx on chunk.type
- connection_sourceId_idx on chunk_connection.sourceId
- connection_targetId_idx on chunk_connection.targetId
- connection_unique_idx on (sourceId, targetId, relation) — prevents duplicate connections

## Search

Uses PostgreSQL trigram similarity (\`pg_trgm\` extension) for fuzzy search. Queries use \`similarity(title, searchTerm)\` for ranking and \`ilike\` for substring matching as fallback.

## Versioning

Chunk updates create a version snapshot before modifying. The version stores the previous state (title, content, type, tags) with an incrementing version number. Enables full edit history per chunk.

## Configuration

- drizzle-orm/node-postgres driver
- Schema in packages/db/src/schema/
- Repository functions in packages/db/src/repository/
- drizzle-kit for migrations and push
- Test files must NOT be in schema/ directory (breaks drizzle-kit's CJS loader)`
    },
    {
        id: "c-repository-layer",
        title: "Repository Layer Pattern",
        type: "reference",
        tags: ["database", "repository", "drizzle", "effect"],
        content: `The repository layer in packages/db/src/repository/ wraps all Drizzle queries with Effect for typed error handling.

## Structure

Each domain has its own repository file:
- chunk.ts — CRUD for chunks (list, get, create, update, delete, export)
- connection.ts — create/delete chunk connections
- stats.ts — aggregate counts (chunks, connections, tags)
- graph.ts — fetch all chunks and connections for visualization
- tags.ts — unique tags with counts using jsonb_array_elements_text
- chunk-version.ts — version snapshot management
- health.ts — database connectivity check

## Conventions

- Every function returns \`Effect<TData, DatabaseError>\`
- All errors are wrapped in DatabaseError with original cause preserved
- Read functions accept optional userId — omitting it returns data for all users
- Write functions always require userId for ownership
- List functions return \`{ items, total }\` for pagination support
- Drizzle conditions are built dynamically based on provided filters

## Package Exports

The repository is exported from packages/db via the \`./repository\` export path, keeping schema and repository concerns separate. Services in packages/api import from \`@fubbik/db/repository\`.`
    },
    {
        id: "c-auth",
        title: "Authentication Setup",
        type: "reference",
        tags: ["auth", "better-auth", "security"],
        content: `Authentication uses Better Auth with a Drizzle adapter.

## Server Side (packages/auth)

- betterAuth() configured with drizzleAdapter, pg provider
- Email/password enabled
- Cookies: sameSite=none, secure=true, httpOnly=true
- trustedOrigins set from CORS_ORIGIN env var
- Session stored in database, resolved via cookie on every request

## Client Side (apps/web)

- createAuthClient() pointed at VITE_SERVER_URL
- useSession() hook for reactive session state
- signIn.email() and signUp.email() for auth flows
- signOut() with redirect

## Web Middleware

- TanStack Start middleware calls authClient.getSession()
- Server function getUser() wraps middleware for route loaders
- Protected routes use beforeLoad with try/catch — redirect to /login on failure
- Dashboard catches auth errors to allow guest access

## API Auth Pattern

- resolve() middleware extracts session from headers for all routes
- requireSession() — returns session or fails with AuthError (for writes)
- optionalSession() — returns session or null (for reads)
- Dev mode: injects a fake dev user when NODE_ENV !== production

## Security Notes

- httpOnly cookies prevent XSS token theft
- CORS_ORIGIN must match the web domain exactly
- credentials: "include" required on Eden treaty client for cookie forwarding`
    },
    {
        id: "c-frontend",
        title: "Frontend Stack",
        type: "document",
        tags: ["frontend", "tanstack", "tailwind", "react"],
        content: `The web app uses TanStack Start with React, Tailwind CSS v4, and shadcn-ui.

## Routing

- TanStack Router with file-based routes in src/routes/
- Route tree auto-generated by @tanstack/router-plugin
- SSR via custom entry-server.ts (serves static assets + SSR handler)
- File naming: \`chunks.$chunkId.tsx\` for params, \`chunks.$chunkId_.edit.tsx\` (underscore) to escape layout nesting

## Key Routes

- / — landing page with feature showcase and API status
- /login, /sign-up — auth forms
- /dashboard — stats, recent chunks, system health, import/export
- /chunks — paginated list with search, type filters, keyboard nav (j/k/Enter/n)
- /chunks/new — create form with AI generation, templates, duplicate detection
- /chunks/$chunkId — detail view with markdown, connections, AI section, version history
- /chunks/$chunkId/edit — edit form with markdown editor
- /graph — interactive knowledge graph (dagre layout + React Flow)
- /tags — tag cloud with counts

## State Management

- TanStack Query for all server state (health, chunks, stats, graph)
- TanStack Form for sign-in/sign-up with field-level validation
- Local component state with useState for UI interactions
- queryClient.invalidateQueries() for optimistic cache updates after mutations

## UI Patterns

- shadcn-ui components in src/components/ui/ (Base UI primitives + CVA variants)
- Feature components in src/features/ organized by domain (auth, chunks, nav, editor, search)
- Mobile nav via Sheet component (hamburger menu, md:hidden)
- Toast notifications via Sonner
- Markdown rendering with react-markdown + @tailwindcss/typography prose classes`
    },
    {
        id: "c-eden-client",
        title: "Eden Treaty Client Setup",
        type: "reference",
        tags: ["eden", "api", "elysia", "type-safety"],
        content: `Eden treaty provides end-to-end type safety from Elysia server to React client with zero code generation.

## How It Works

1. The server exports its Elysia app type: \`export type Api = typeof api\`
2. The client imports this type and creates a treaty client: \`treaty<Api>(serverUrl)\`
3. Every API call is fully typed — params, body, query, and response

## Client Configuration

The treaty client is created in apps/web/src/utils/api.ts with:
- Server URL from VITE_SERVER_URL environment variable
- \`credentials: "include"\` for cookie-based auth forwarding
- Type imported from \`@fubbik/api\` workspace package

## Response Pattern

Every Eden call returns \`{ data, error }\`. The \`unwrapEden()\` utility in utils/eden.ts:
- Throws if error is present
- Returns typed data with null/error types excluded
- Used in every useQuery/useMutation callback for clean error handling

## Usage in Components

Queries: \`api.api.chunks.get({ query: { limit: "5" } })\`
Mutations: \`api.api.chunks.post({ title, content, type, tags })\`
Params: \`api.api.chunks({ id: chunkId }).get()\`

## Benefits

- No OpenAPI spec generation, no codegen step
- Types update instantly when server changes
- IDE autocomplete for all endpoints, params, and responses
- Compile-time errors if API contract breaks`
    },
    {
        id: "c-env",
        title: "Environment Configuration",
        type: "reference",
        tags: ["env", "config", "arktype", "validation"],
        content: `Environment validation uses @t3-oss/env-core with Arktype schemas. Validated at startup — fails fast with clear errors if env vars are missing or invalid.

## Server Env (packages/env/src/server.ts)

Required:
- DATABASE_URL — non-empty string (Postgres connection)
- BETTER_AUTH_SECRET — string >= 32 characters
- BETTER_AUTH_URL — valid URL
- CORS_ORIGIN — valid URL

Optional with defaults:
- NODE_ENV — 'development' | 'production' | 'test' (default: 'development')
- PORT — string (default: '3000')

Optional feature flags:
- OPENAI_API_KEY — enables AI features
- OPENAI_MODEL — model selection
- RATE_LIMIT_MAX, RATE_LIMIT_DURATION_MS — rate limiting config

## Web Env (packages/env/src/web.ts)

- VITE_SERVER_URL — valid URL pointing to the API server
- Falls back to process.env for SSR runtime (import.meta.env not populated in SSR)

## Key Pattern

Uses \`emptyStringAsUndefined: true\` so empty strings in env files are treated as missing. Arktype schemas validate types at runtime with clear error messages. Shared across all packages via \`@fubbik/env/server\` and \`@fubbik/env/web\` export paths.`
    },
    {
        id: "c-deployment",
        title: "Deployment & Docker",
        type: "document",
        tags: ["deployment", "docker", "railway"],
        content: `Multi-stage Docker builds for both web and server apps. Deployed on Railway with three services.

## Docker Builds

Both Dockerfiles follow the same pattern:
1. Builder stage (oven/bun:1.3.9): Copy all workspace package.json files → bun install --frozen-lockfile → copy source → build
2. Runner stage (oven/bun:1.3.9-slim): Copy package.json + node_modules + dist → run

Key details:
- VITE_SERVER_URL passed as Docker ARG for build-time inlining in the web build
- Web CMD: bun run dist/server/entry-server.js
- Server CMD: bun run dist/index.mjs

## Entry Server

The web app has a custom entry-server.ts that:
- Collects all static files from dist/client/ at startup
- Serves assets with correct MIME types
- Applies 1-year cache headers for hashed /assets/ files
- Falls through to TanStack Start SSR handler for routes

## Railway Setup

Three services in one project:
- **server** — Elysia API, Dockerfile at apps/server/Dockerfile
- **web** — TanStack Start SSR, Dockerfile at apps/web/Dockerfile
- **Postgres** — managed database, internal hostname postgres.railway.internal

Environment:
- DATABASE_URL uses Railway internal hostname (not reachable externally)
- DATABASE_PUBLIC_URL available via TCP proxy for external access (drizzle-kit push, seeding)
- CORS_ORIGIN on server must match the web domain
- Deploy with \`railway service <name> && railway up --detach\``
    },
    {
        id: "c-cli",
        title: "CLI Tool",
        type: "reference",
        tags: ["cli", "commander", "local-first"],
        content: `Command-line interface built with Commander.js in apps/cli. Designed for both human use and AI agent integration.

## Commands

Core CRUD:
- fubbik init — initialize .fubbik/ directory with store.json
- fubbik add — create chunk (interactive or --title/--content-file flags)
- fubbik get <id> — fetch chunk by ID
- fubbik cat <id> — raw content output (no formatting, for piping)
- fubbik list — list chunks (--type, --tag, --limit, --offset, --sort, --fields)
- fubbik search <query> — fuzzy search (--limit, --offset, --fields)
- fubbik update <id> — modify chunk fields (--content-file supports stdin via -)
- fubbik remove <id> — delete chunk

Relationships:
- fubbik link <source> <target> — create connection with --relation
- fubbik unlink <id> — remove connection

Bulk operations:
- fubbik bulk-add <file> — import from JSONL file (one chunk per line)
- fubbik export — JSON or markdown with YAML frontmatter (--format, --dir)
- fubbik import — from JSON array or markdown directory

Analysis:
- fubbik stats — knowledge base overview (type counts, tag counts, sync status)
- fubbik tags — list unique tags with counts
- fubbik diff — compare local store vs server (local-only, server-only, modified)

Infrastructure:
- fubbik health — check API connectivity
- fubbik sync — bidirectional sync with server (--push, --pull)

## Global Flags

- --json — machine-readable JSON output
- -q, --quiet — minimal output (just IDs for creates)
- --content-file <path> — read content from file (- for stdin)
- --fields <list> — project specific fields in output

## Local Store

File-based JSON at .fubbik/store.json with chunks array. Local IDs use time-based format: c-{timestamp-base36}. The store tracks serverUrl and lastSync for sync operations.`
    },
    {
        id: "c-ai-features",
        title: "AI Integration",
        type: "reference",
        tags: ["ai", "openai", "vercel-ai"],
        content: `AI features use the Vercel AI SDK with OpenAI provider.

## Server Endpoints

- POST /api/ai/summarize — generates a concise summary of a chunk's content
- POST /api/ai/suggest-connections — analyzes a chunk and suggests related chunks to link
- POST /api/ai/generate — creates a full chunk (title, content, type, tags) from a natural language prompt

## Implementation

Uses \`generateText()\` and \`generateObject()\` from the Vercel AI SDK:
- Model configured via OPENAI_API_KEY and OPENAI_MODEL env vars
- Structured output via Zod schemas for type-safe AI responses
- Each endpoint wraps the AI call in Effect.tryPromise with AiError handling

## Frontend Integration

- AI Generate card on /chunks/new — enter a prompt, get a pre-filled chunk form
- AI Section on chunk detail — summarize button and suggest connections button
- Loading states with spinner icons during generation
- Results displayed inline (summary text, connection suggestions as linkable cards)

## Feature Gating

AI features are optional — they only work when OPENAI_API_KEY is set. The frontend always shows the UI but displays errors if the API key isn't configured.`
    },
    {
        id: "c-tooling",
        title: "Development Tooling",
        type: "reference",
        tags: ["tooling", "turbo", "testing", "ci"],
        content: `Development tools and workflows.

## Turborepo

- turbo.json defines build, dev, lint, check-types, test tasks
- dev task is persistent, cache disabled
- \`bun dev\` starts web (vite on :3001) + server (bun --hot on :3000) in parallel
- Filter syntax: \`bun dev:web\`, \`bun dev:server\`

## Type Checking

- \`bun check-types\` runs tsgo (native TypeScript) across all packages
- @typescript/native-preview installed as root devDependency
- Each package has its own tsconfig extending @fubbik/config
- Strict mode enabled everywhere

## Testing

- Vitest for unit and integration tests
- API tests use treaty client against a standalone Elysia app
- Schema tests validate table column existence
- \`bun test\` runs vitest across all packages

## Code Quality

- oxlint for fast linting (Rust-based)
- oxfmt for formatting (Rust-based, no Prettier)
- sherif for workspace dependency validation (catches version mismatches)
- knip for unused dependency/export detection

## CI Pipeline

\`bun ci\` runs: check-types → lint → test → build → format check → sherif

## Database Commands

- \`bun db:push\` — push schema to database (drizzle-kit push)
- \`bun db:studio\` — open Drizzle Studio UI
- \`bun db:generate\` — generate migration files
- \`bun seed\` — seed database with sample data`
    },
    {
        id: "c-monorepo-patterns",
        title: "Monorepo & Workspace Patterns",
        type: "reference",
        tags: ["monorepo", "bun", "turbo", "workspace"],
        content: `The monorepo uses Bun workspaces with Turborepo for build orchestration.

## Workspace Layout

Root package.json defines workspaces at \`apps/*\` and \`packages/*\`. All internal packages use the \`@fubbik/\` scope with \`workspace:*\` version specifiers.

## Catalog Versioning

Shared dependency versions are defined in root package.json's \`catalog\` section. Packages reference them with \`catalog:\` — ensures all packages use the same version of typescript, elysia, better-auth, arktype, vitest, etc.

## Package Exports

Each package declares explicit export paths:
- \`@fubbik/api\`: \`.\` (main plugin), \`./context\` (Session type)
- \`@fubbik/db\`: \`.\` (schema + connection), \`./repository\` (data access), \`./errors\` (DatabaseError)
- \`@fubbik/env\`: \`./server\`, \`./web\` (separate validation per environment)
- \`@fubbik/auth\`: \`.\` (auth instance), \`./client\` (browser client)

## Build Order

Turborepo handles dependency resolution. packages/config has no deps, packages/env depends on nothing internal, packages/db depends on packages/env, packages/auth depends on packages/db, packages/api depends on all packages.

## Shared Config

@fubbik/config provides base TypeScript configuration. Each package extends it with its own tsconfig.json.`
    },
    {
        id: "c-ui-components",
        title: "UI Component Architecture",
        type: "reference",
        tags: ["frontend", "shadcn", "components", "tailwind"],
        content: `The UI uses shadcn-ui components built on Base UI primitives with Tailwind CSS v4.

## Component Library (src/components/ui/)

Built with CVA (class-variance-authority) for variant-based styling. Key components:
- Button — variants: default, destructive, outline, ghost, secondary, link; sizes: xs through xl plus icon sizes
- Card — composed of Card, CardHeader, CardPanel, CardTitle, CardDescription
- Badge — variants: default, secondary, outline, destructive; sizes: default, sm
- Dialog, Sheet — overlay components for modals and slide-out panels
- Form — field-level error display integrated with TanStack Form
- Command — command palette component (cmdk-based)
- Separator, Tabs, Sidebar — layout primitives

## Tailwind v4 Patterns

- Uses \`@plugin "@tailwindcss/typography"\` directive in CSS (not tailwind.config)
- Dark mode via next-themes with \`dark:\` variant prefix
- Prose classes require wrapping div: \`<div className="prose dark:prose-invert">\`

## Feature Components (src/features/)

Organized by domain:
- auth/ — SignInForm, SignUpForm, UserMenu
- chunks/ — AiSection, LinkChunkDialog, DeleteConnectionButton, VersionHistory, templates
- editor/ — MarkdownEditor with live preview toggle
- nav/ — MobileNav (Sheet-based hamburger menu)
- search/ — CommandSearch (command palette for quick navigation)

## Composition Pattern

Base UI components use \`render\` prop for element composition — e.g., a Button that renders as a Link: \`<Button render={<Link to="/path" />}>\``
    },
    {
        id: "c-tanstack-router",
        title: "TanStack Router Patterns",
        type: "reference",
        tags: ["frontend", "routing", "tanstack"],
        content: `File-based routing with TanStack Router in apps/web/src/routes/.

## File Naming Conventions

- \`index.tsx\` — root route (/)
- \`dashboard.tsx\` — /dashboard
- \`chunks.index.tsx\` — /chunks (index of chunks section)
- \`chunks.$chunkId.tsx\` — /chunks/:chunkId (dynamic param)
- \`chunks.$chunkId_.edit.tsx\` — /chunks/:chunkId/edit (underscore escapes layout nesting)
- \`chunks.new.tsx\` — /chunks/new (static segment)
- \`__root.tsx\` — root layout (wraps all routes)

## Layout Nesting

Without underscore: \`chunks.$chunkId.edit.tsx\` becomes a child of \`chunks.$chunkId.tsx\` and requires <Outlet/> in the parent. With underscore: \`chunks.$chunkId_.edit.tsx\` is a sibling route with its own layout.

## Route Guards (beforeLoad)

Protected routes use beforeLoad to check auth:
- Call getUser() server function
- On failure: throw redirect({ to: "/login" })
- Return session in context for use in component

Guest-accessible routes catch the error silently and continue with null session.

## Search Params

Validated via validateSearch option. Used for pagination, filtering:
- \`/chunks?type=note&q=search&page=2\`
- Updates via navigate() with search object

## Route Tree

Auto-generated by @tanstack/router-plugin into routeTree.gen.ts. Never edit manually — regenerated on file changes during dev.`
    },
    {
        id: "c-gotchas",
        title: "Known Gotchas & Workarounds",
        type: "note",
        tags: ["gotchas", "debugging", "troubleshooting"],
        content: `Issues encountered and their solutions.

## TanStack Form

- form.Subscribe requires explicit selector prop in v1.23+
- No @tanstack/arktype-form-adapter exists — use manual validator functions

## TanStack Router

- File naming with underscore suffix (e.g., \`$chunkId_.edit.tsx\`) escapes layout nesting — without it, the route becomes a child requiring <Outlet/> in parent
- Route tree must be regenerated when adding/removing route files

## Arktype

- type().default() returns a tuple, not a Standard Schema — don't use it with t3-env
- Provide defaults via runtimeEnv spread instead

## Elysia

- error() helper not available in resolve() chain — use set.status pattern
- CORS must include all HTTP methods used (GET, POST, PATCH, DELETE, OPTIONS)

## Vite / TanStack Start

- VITE_* vars are build-time only — not available in SSR runtime
- Custom entry-server.ts needed for production static file serving
- import.meta.env not populated in SSR for VITE_ vars — fallback to process.env

## react-markdown v10

- No longer accepts className prop directly — must wrap in a <div> with prose classes
- Requires @tailwindcss/typography plugin for prose styling

## Tailwind v4

- Plugin registration uses \`@plugin "@tailwindcss/typography"\` in CSS, not config file
- No tailwind.config.js — configuration is in CSS

## Docker

- All workspace package.json files must be copied before bun install
- --no-cache needed when Dockerfile changes aren't picked up
- VITE_SERVER_URL must be passed as Docker ARG for build-time inlining

## Drizzle

- Schema directory must NOT contain test files — drizzle-kit's CJS loader chokes on vitest imports
- node-postgres driver works with Bun but needs correct DATABASE_URL
- Railway internal hostname not reachable externally — use DATABASE_PUBLIC_URL for drizzle-kit push

## Effect

- FiberFailure wraps errors with a Symbol key — extraction requires Symbol.for("effect/Runtime/FiberFailure/Cause")
- Effect.runPromise throws on failure — must be caught by Elysia onError middleware`
    },
    {
        id: "c-search-patterns",
        title: "Search & Filtering",
        type: "reference",
        tags: ["search", "postgres", "trigram"],
        content: `Full-text search using PostgreSQL trigram similarity.

## How Search Works

The chunks list endpoint accepts a \`search\` query parameter. When provided:
1. Filters using trigram similarity (\`%\` operator) on title and content
2. Falls back to ilike substring matching for partial matches
3. Orders results by similarity score descending (most relevant first)

Without search, results are ordered by updatedAt descending (most recent first).

## Frontend Search

- Chunks index page has a search input with Enter-to-search
- Search term is stored in URL search params (\`?q=term\`)
- Debounced duplicate detection on the new chunk form (500ms delay)
- Command palette (Cmd+K) for quick navigation across chunks

## Filtering

- Type filter: note, document, reference, schema, checklist
- Tag filter in CLI: --tag flag
- Pagination: limit/offset with total count for page calculation
- CLI supports --fields for projecting specific columns in output

## Performance

- Indexes on userId and type for fast filtering
- Trigram index on title for similarity search
- Count query runs separately from data query for accurate pagination`
    },
    {
        id: "c-chunk-templates",
        title: "Chunk Templates",
        type: "reference",
        tags: ["frontend", "templates", "ux"],
        content: `Pre-built templates for common chunk types, available on the new chunk form.

## Available Templates

- **Meeting Notes** — structured agenda/discussion/action items format (type: note, tags: meeting, notes)
- **Decision Record** — context/options/decision/consequences format (type: document, tags: decision, adr)
- **API Reference** — endpoint/auth/request/response documentation (type: reference, tags: api, documentation)
- **Checklist** — task list with checkbox markdown (type: checklist, tags: tasks, todo)
- **Schema** — entity/field/relationship documentation (type: schema, tags: schema, data-model)

## How They Work

Templates are defined in src/features/chunks/templates.ts as static objects with name, description, content (markdown), type, and tags. Selecting a template pre-fills the form fields but leaves the title empty for the user to fill in.

## Template + AI Workflow

Users can either start from a template and fill in details, or use AI Generate to create a complete chunk from a prompt. Both approaches pre-fill the same form, allowing further editing before saving.`
    },
    {
        id: "c-graph-visualization",
        title: "Knowledge Graph Visualization",
        type: "reference",
        tags: ["graph", "visualization", "dagre", "react-flow"],
        content: `Interactive knowledge graph using React Flow with dagre auto-layout.

## Architecture

- Data: GET /api/graph returns all chunks (id, title) and connections (source, target, relation)
- Layout: Dagre (directed graph layout algorithm) computes node positions
- Rendering: React Flow displays interactive nodes and edges
- Navigation: clicking a node navigates to the chunk detail page

## Layout Algorithm

Dagre is configured with:
- Direction: top-to-bottom (rankdir: TB)
- Node spacing: 80px horizontal, 100px vertical
- Fixed node dimensions: 200x50px
- Positions computed in a useMemo based on data

## Interaction

- Nodes are clickable (navigate to chunk)
- Edges are animated (flowing animation)
- Edge labels show the relation type
- Pan, zoom, and drag supported via React Flow controls
- Background grid and zoom controls displayed

## Data Flow

The graph fetches data with TanStack Query, computes layout with dagre in useMemo, then syncs to React Flow's useNodesState/useEdgesState via useEffect. Dark color mode is enabled.`
    },
    {
        id: "c-version-history",
        title: "Chunk Version History",
        type: "reference",
        tags: ["versioning", "history", "chunks"],
        content: `Every chunk edit creates a version snapshot, enabling full edit history.

## How It Works

1. User submits an edit via PATCH /api/chunks/:id
2. Service fetches the current chunk state
3. Gets the next version number for this chunk
4. Creates a chunk_version record with the PREVIOUS state (title, content, type, tags)
5. Applies the update to the chunk

This means versions represent what the chunk looked like BEFORE each edit. The current state is always in the chunk table itself.

## Schema

chunk_version table:
- id (text PK)
- chunkId (FK to chunk)
- version (integer, auto-incrementing per chunk)
- title, content, type, tags — snapshot of previous state
- createdAt — when the edit happened

## Frontend

The chunk detail page shows a "Version History" section at the bottom. Each version displays:
- Version number and timestamp
- Previous title, type, and tags
- Content preview (truncated)

Versions are fetched via GET /api/chunks/:id/history and displayed in reverse chronological order.`
    }
];

for (const c of chunks) {
    await db
        .insert(chunk)
        .values({
            ...c,
            userId: DEV_USER_ID
        })
        .catch(e => console.error(`  ✗ ${c.title}:`, e));
    console.log(`  ✓ ${c.title}`);
}

// Add connections
const connections = [
    // Architecture connects to everything
    { id: "conn-1", sourceId: "c-architecture", targetId: "c-database", relation: "depends on" },
    { id: "conn-2", sourceId: "c-architecture", targetId: "c-api-design", relation: "references" },
    { id: "conn-3", sourceId: "c-architecture", targetId: "c-frontend", relation: "references" },
    { id: "conn-4", sourceId: "c-architecture", targetId: "c-cli", relation: "references" },
    { id: "conn-5", sourceId: "c-architecture", targetId: "c-monorepo-patterns", relation: "defines" },

    // API design
    { id: "conn-6", sourceId: "c-api-design", targetId: "c-auth", relation: "depends on" },
    { id: "conn-7", sourceId: "c-api-design", targetId: "c-database", relation: "uses" },
    { id: "conn-8", sourceId: "c-api-design", targetId: "c-effect-patterns", relation: "implements" },
    { id: "conn-9", sourceId: "c-api-design", targetId: "c-eden-client", relation: "types consumed by" },

    // Effect patterns
    { id: "conn-10", sourceId: "c-effect-patterns", targetId: "c-repository-layer", relation: "used in" },
    { id: "conn-11", sourceId: "c-effect-patterns", targetId: "c-ai-features", relation: "used in" },

    // Database
    { id: "conn-12", sourceId: "c-database", targetId: "c-repository-layer", relation: "accessed via" },
    { id: "conn-13", sourceId: "c-database", targetId: "c-search-patterns", relation: "enables" },
    { id: "conn-14", sourceId: "c-database", targetId: "c-version-history", relation: "stores" },

    // Frontend
    { id: "conn-15", sourceId: "c-frontend", targetId: "c-eden-client", relation: "uses" },
    { id: "conn-16", sourceId: "c-frontend", targetId: "c-auth", relation: "uses" },
    { id: "conn-17", sourceId: "c-frontend", targetId: "c-tanstack-router", relation: "built with" },
    { id: "conn-18", sourceId: "c-frontend", targetId: "c-ui-components", relation: "built with" },

    // Feature pages
    { id: "conn-19", sourceId: "c-chunk-templates", targetId: "c-frontend", relation: "part of" },
    { id: "conn-20", sourceId: "c-graph-visualization", targetId: "c-frontend", relation: "part of" },
    { id: "conn-21", sourceId: "c-search-patterns", targetId: "c-frontend", relation: "used in" },
    { id: "conn-22", sourceId: "c-version-history", targetId: "c-frontend", relation: "displayed in" },

    // CLI
    { id: "conn-23", sourceId: "c-cli", targetId: "c-api-design", relation: "consumes" },
    { id: "conn-24", sourceId: "c-cli", targetId: "c-ai-features", relation: "triggers" },

    // Infrastructure
    { id: "conn-25", sourceId: "c-env", targetId: "c-deployment", relation: "referenced by" },
    { id: "conn-26", sourceId: "c-deployment", targetId: "c-architecture", relation: "deploys" },
    { id: "conn-27", sourceId: "c-tooling", targetId: "c-architecture", relation: "supports" },
    { id: "conn-28", sourceId: "c-tooling", targetId: "c-monorepo-patterns", relation: "orchestrates" },

    // Gotchas cross-references
    { id: "conn-29", sourceId: "c-gotchas", targetId: "c-frontend", relation: "relates to" },
    { id: "conn-30", sourceId: "c-gotchas", targetId: "c-api-design", relation: "relates to" },
    { id: "conn-31", sourceId: "c-gotchas", targetId: "c-database", relation: "relates to" },
    { id: "conn-32", sourceId: "c-gotchas", targetId: "c-deployment", relation: "relates to" },
    { id: "conn-33", sourceId: "c-gotchas", targetId: "c-effect-patterns", relation: "relates to" },

    // AI features
    { id: "conn-34", sourceId: "c-ai-features", targetId: "c-api-design", relation: "extends" },
    { id: "conn-35", sourceId: "c-ai-features", targetId: "c-chunk-templates", relation: "complements" }
];

await db.delete(chunkConnection);
for (const conn of connections) {
    await db
        .insert(chunkConnection)
        .values(conn)
        .catch(e => console.error(`  ✗ ${conn.id}:`, e));
}
console.log(`  ✓ ${connections.length} connections`);

console.log(`\n✅ Database seeded: ${chunks.length} chunks, ${connections.length} connections`);
process.exit(0);
