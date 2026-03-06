import { config } from "dotenv";
import { resolve } from "path";
import { eq } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";
import { chunk, chunkConnection } from "./schema/chunk";
import { user } from "./schema/auth";

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
    emailVerified: false,
  });
}

// Clear existing chunks for dev user
await db.delete(chunk).where(eq(chunk.userId, DEV_USER_ID));

const chunks = [
  {
    id: "c-architecture",
    title: "Project Architecture",
    type: "document",
    tags: ["architecture", "overview"],
    content: `Fubbik is a local-first knowledge framework built as a TypeScript monorepo.

## Structure

- apps/web — TanStack Start frontend (SSR, Tailwind, shadcn-ui)
- apps/server — Elysia backend (REST API, Better Auth, OpenTelemetry)
- apps/cli — Commander.js CLI tool
- packages/api — Shared Elysia API plugin with Eden treaty types
- packages/auth — Better Auth with Drizzle adapter
- packages/config — Shared TypeScript config
- packages/db — Drizzle ORM schema and Postgres connection
- packages/env — Environment validation with Arktype + t3-env

## Key Decisions

- Bun as runtime and package manager
- Turborepo for build orchestration
- Arktype for validation (not Zod) — implements Standard Schema v1
- Eden treaty for end-to-end type-safe API client
- Session-based auth with httpOnly cookies`,
  },
  {
    id: "c-api-design",
    title: "API Design Patterns",
    type: "reference",
    tags: ["api", "elysia", "patterns"],
    content: `The API layer uses Elysia with a shared plugin pattern exported from packages/api.

## Eden Treaty

Type-safe client generated from the Elysia server definition. No code generation step — types flow directly from server to client via the Api type export.

## Authentication

Session resolution happens in a shared resolve() middleware. All routes after the middleware have access to session. In development, a dev user is injected automatically when no session exists.

## Error Handling

Protected routes check session and use set.status = 401 pattern (not error() helper, which isn't typed in resolve chains). Eden returns { data, error } on the client.

## Endpoints

- GET /api/health — public health check
- GET /api/me — current user info
- GET /api/chunks — list chunks (filterable by type, search, limit)
- GET /api/chunks/:id — chunk detail with connections
- POST /api/chunks — create chunk
- PATCH /api/chunks/:id — update chunk
- DELETE /api/chunks/:id — delete chunk
- GET /api/stats — chunk/connection/tag counts`,
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

- chunk — id, title, content, type, tags (jsonb), userId FK, timestamps
- chunk_connection — source/target chunk links with relation type

## Configuration

- drizzle-orm/node-postgres driver (pg library)
- Schema defined in packages/db/src/schema/
- drizzle-kit for migrations and push
- DATABASE_URL from @fubbik/env/server`,
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

## Client Side (apps/web)

- createAuthClient() pointed at VITE_SERVER_URL
- useSession() hook for reactive session state
- signIn.email() and signUp.email() for auth flows
- signOut() with redirect

## Web Middleware

- TanStack Start middleware calls authClient.getSession()
- Server function getUser() wraps middleware for route loaders
- Dashboard catches auth errors to allow guest access

## Dev Mode

- API resolve() injects a dev user when NODE_ENV !== production
- No login required for local development`,
  },
  {
    id: "c-frontend",
    title: "Frontend Stack",
    type: "document",
    tags: ["frontend", "tanstack", "tailwind"],
    content: `The web app uses TanStack Start with React, Tailwind CSS, and shadcn-ui.

## Routing

- TanStack Router with file-based routes in src/routes/
- Route tree auto-generated by @tanstack/router-cli
- SSR via custom entry-server.ts (serves static assets + SSR handler)

## Key Routes

- / — landing page with feature showcase and API status
- /dashboard — stats, recent chunks, system health
- /chunks/new — create chunk form
- /chunks/$chunkId — chunk detail with connections and delete

## State Management

- TanStack Query for server state (health, chunks, stats)
- TanStack Form for sign-in/sign-up with Arktype validators
- Eden treaty client in src/utils/api.ts

## UI Components

- shadcn-ui components in src/components/ui/
- FubbikLogo SVG component (graph nodes using currentColor)
- UserMenu with auth state (sign in / user dropdown)
- Sonner for toast notifications`,
  },
  {
    id: "c-env",
    title: "Environment Configuration",
    type: "reference",
    tags: ["env", "config", "arktype"],
    content: `Environment validation uses @t3-oss/env-core with Arktype schemas.

## Server Env (packages/env/src/server.ts)

- DATABASE_URL — string >= 1
- BETTER_AUTH_SECRET — string >= 32
- BETTER_AUTH_URL — string.url
- CORS_ORIGIN — string.url
- NODE_ENV — 'development' | 'production' | 'test' (defaults to 'development')

## Web Env (packages/env/src/web.ts)

- VITE_SERVER_URL — string.url
- Falls back to process.env for SSR runtime

## Notes

- Arktype implements Standard Schema v1, works directly with createEnv
- No .default() on arktype types (returns tuple, not schema) — use runtimeEnv spread instead
- dotenv/config loaded in server.ts for non-Vite contexts`,
  },
  {
    id: "c-deployment",
    title: "Deployment & Docker",
    type: "document",
    tags: ["deployment", "docker", "railway"],
    content: `Multi-stage Docker builds for both web and server apps. Deployed on Railway.

## Docker

- Base image: oven/bun:1.3.9 (builder), oven/bun:1.3.9-slim (runner)
- All workspace package.json files copied for bun install --frozen-lockfile
- VITE_SERVER_URL passed as ARG for build-time inlining
- Web CMD: bun run dist/server/entry-server.js
- Server CMD: bun run dist/index.mjs

## Entry Server (apps/web/src/entry-server.ts)

- Custom TanStack Start server handler
- Collects static files from dist/client/ at startup
- Serves with correct MIME types and cache headers (1yr for /assets/)
- Falls through to SSR handler for all other routes

## Railway

- Three services: web, server, Postgres
- RAILWAY_DOCKERFILE_PATH for Docker builds
- Reference variables for DATABASE_URL: $\{{Postgres.DATABASE_URL}}
- Custom domain support (CORS_ORIGIN must match)`,
  },
  {
    id: "c-cli",
    title: "CLI Tool",
    type: "reference",
    tags: ["cli", "commander"],
    content: `Command-line interface built with Commander.js in apps/cli.

## Commands

- fubbik init — initialize a new knowledge base (.fubbik/ directory)
- fubbik health — check API server connection
- fubbik add — add a new chunk (interactive prompts for title, type, tags, content)
- fubbik get <id> — get chunk by ID (--json for raw output)
- fubbik list — list all chunks (--type, --tag filters, --json)
- fubbik search <query> — search by title, content, or tags
- fubbik update <id> — update chunk fields
- fubbik remove <id> — delete chunk

## Local Store

- Uses a local JSON file store at .fubbik/store.json
- CRUD operations in src/store.ts
- Built as standalone binary: bun build --compile --outfile dist/fubbik`,
  },
  {
    id: "c-tooling",
    title: "Development Tooling",
    type: "reference",
    tags: ["tooling", "turbo", "testing"],
    content: `Development tools and workflows.

## Turborepo

- turbo.json defines build, dev, lint, check-types, test tasks
- dev task is persistent, cache disabled
- bun dev starts web (vite) + server (bun --hot) in parallel

## Type Checking

- bun check-types runs tsc across all packages
- Each package has its own tsconfig extending @fubbik/config
- Strict mode enabled

## Testing

- Vitest configured (not yet extensively used)
- bun test runs vitest

## Code Quality

- oxlint for linting
- oxfmt for formatting
- sherif for workspace dependency validation
- knip for unused dependency detection

## Ports (Development)

- 3000 — Elysia server
- 3001 — Vite dev server (web)`,
  },
  {
    id: "c-gotchas",
    title: "Known Gotchas & Workarounds",
    type: "note",
    tags: ["gotchas", "debugging"],
    content: `Issues encountered and their solutions.

## TanStack Form

- form.Subscribe requires explicit selector prop in v1.23+
- No @tanstack/arktype-form-adapter exists — use manual validator functions

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

## Docker

- All workspace package.json files must be copied before bun install
- --no-cache needed when Dockerfile changes aren't picked up
- VITE_SERVER_URL must be passed as Docker ARG for build-time inlining

## Drizzle

- Schema index must use extensionless imports for drizzle-kit compatibility
- node-postgres driver works with Bun but needs correct DATABASE_URL`,
  },
];

for (const c of chunks) {
  await db.insert(chunk).values({
    ...c,
    userId: DEV_USER_ID,
  });
  console.log(`  ✓ ${c.title}`);
}

// Add connections
const connections = [
  { id: "conn-1", sourceId: "c-architecture", targetId: "c-database", relation: "depends on" },
  { id: "conn-2", sourceId: "c-architecture", targetId: "c-api-design", relation: "references" },
  { id: "conn-3", sourceId: "c-architecture", targetId: "c-frontend", relation: "references" },
  { id: "conn-4", sourceId: "c-api-design", targetId: "c-auth", relation: "depends on" },
  { id: "conn-5", sourceId: "c-api-design", targetId: "c-database", relation: "uses" },
  { id: "conn-6", sourceId: "c-frontend", targetId: "c-api-design", relation: "consumes" },
  { id: "conn-7", sourceId: "c-frontend", targetId: "c-auth", relation: "uses" },
  { id: "conn-8", sourceId: "c-env", targetId: "c-deployment", relation: "referenced by" },
  { id: "conn-9", sourceId: "c-deployment", targetId: "c-architecture", relation: "deploys" },
  { id: "conn-10", sourceId: "c-gotchas", targetId: "c-frontend", relation: "relates to" },
  { id: "conn-11", sourceId: "c-gotchas", targetId: "c-api-design", relation: "relates to" },
  { id: "conn-12", sourceId: "c-cli", targetId: "c-api-design", relation: "consumes" },
  { id: "conn-13", sourceId: "c-tooling", targetId: "c-architecture", relation: "supports" },
];

await db.delete(chunkConnection);
for (const conn of connections) {
  await db.insert(chunkConnection).values(conn);
}
console.log(`  ✓ ${connections.length} connections`);

console.log("\n✅ Database seeded successfully");
process.exit(0);
