# fubbik

This file provides context about the project for AI assistants.

## Project Overview

- **Ecosystem**: Typescript
- **Purpose**: Local-first knowledge framework for storing, navigating, and evolving structured knowledge about codebases. Designed for both humans (web UI, graph visualization) and machines (CLI, API, VS Code extension).

## Tech Stack

- **Runtime**: bun
- **Package Manager**: pnpm

### Frontend

- Framework: tanstack-start
- CSS: tailwind
- UI Library: shadcn-ui
- State: tanstack-store
- Graph: @xyflow/react (React Flow)

### Backend

- Framework: elysia
- API Client: elysia eden treaty
- Validation: arktype
- Error Handling: effect (typed errors via Effect.tryPromise, tagged errors)

### Database

- Database: postgres
- ORM: drizzle
- Extensions: pgvector (embeddings), pg_trgm (fuzzy text search)

### Authentication

- Provider: better-auth

### Additional Features

- Testing: vitest
- AI: vercel-ai
- Embeddings: Ollama (nomic-embed-text) for local vector embeddings
- Logging: winston
- Observability: opentelemetry

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend application (TanStack Start)
│   ├── server/      # Backend API server (Elysia)
│   ├── cli/         # CLI application (Commander.js)
│   └── vscode/      # VS Code / Cursor extension
├── packages/
│   ├── api/         # API layer (Elysia routes, Eden types)
│   ├── auth/        # Authentication (Better Auth + Drizzle adapter)
│   ├── config/      # Shared TypeScript config
│   ├── db/          # Database schema (Drizzle ORM)
│   └── env/         # Environment validation (Arktype + t3-env)
└── docs/
    └── superpowers/
        ├── specs/   # Design specifications
        └── plans/   # Implementation plans
```

## Core Concepts

### Chunks

The central entity — discrete units of knowledge (conventions, architecture decisions, runbooks, etc.). Each chunk has:
- `title`, `content`, `type` (note, document, reference, schema, checklist)
- `tags` (via normalized join table with tag types)
- `scope` (JSONB key-value metadata)
- `appliesTo` (glob patterns linking chunks to file areas, via `chunk_applies_to` table)
- `fileReferences` (explicit bidirectional links to files/symbols, via `chunk_file_ref` table)
- `rationale`, `alternatives`, `consequences` (optional "why" decision context fields)
- `embedding` (768-dim vector for semantic search)
- AI-generated: `summary`, `aliases`, `notAbout`
- Version history (append-only `chunk_version` table)

### Codebases

Chunks can be organized per-codebase. A codebase is identified by git remote URL (normalized) or local paths. Chunks can belong to multiple codebases or none (global).
- `codebase` table with `remoteUrl`, `localPaths`, `name`
- `chunk_codebase` join table (many-to-many)
- CLI auto-detects codebase via git remote
- Web UI has a codebase switcher in the nav

### Connections

Directed edges between chunks: `sourceId → targetId` with a `relation` type (related_to, part_of, depends_on, extends, references, supports, contradicts, alternative_to).

### Templates

Built-in + user-created chunk templates (Convention, Architecture Decision, Runbook, API Endpoint). Built-in templates are seeded via SQL migration and are read-only. Users can duplicate and customize.

### Plans

Implementation tracking entities for AI agents and humans. Plans have ordered steps with individual status tracking.
- `plan` table with `title`, `description`, `status` (draft/active/completed/archived), `userId`, `codebaseId`
- `plan_step` table with `description`, `status` (pending/in_progress/done/skipped/blocked), `order`, `parentStepId` (nesting), `note`, optional `chunkId` link
- `plan_chunk_ref` join table linking plans to chunks with `relation` (context/created/modified)
- Web UI: `/plans` list, `/plans/new` create, `/plans/:id` detail with interactive checklist
- CLI: `fubbik plan create/list/show/step-done/add-step/activate/complete`

### Chunk Health Scores

Per-chunk health scores (0-100) computed on-demand from:
- **Freshness** (0-25): days since last update
- **Completeness** (0-25): has rationale/alternatives/consequences
- **Richness** (0-25): content length + AI enrichment
- **Connectivity** (0-25): number of connections
Exposed via chunk detail API response and shown as a badge on the detail page.

## Architecture Patterns

### Backend: Repository -> Service -> Route

- **Repositories** (`packages/db/src/repository/`): Return `Effect<T, DatabaseError>`. Pure data access, no business logic.
- **Services** (`packages/api/src/*/service.ts`): Compose repository Effects, add business logic, introduce `NotFoundError`/`AuthError`/`ValidationError`.
- **Routes** (`packages/api/src/*/routes.ts`): Call `Effect.runPromise(requireSession(ctx).pipe(...))`. Errors propagate to global `.onError` handler.
- **Global error handler** (`packages/api/src/index.ts`): Extracts Effect errors from FiberFailure, maps `_tag` to HTTP status codes (ValidationError->400, AuthError->401, NotFoundError->404, DatabaseError->500).

### Frontend: Feature-based Structure

- Route files in `apps/web/src/routes/`
- Feature components in `apps/web/src/features/` (e.g., `features/auth/`, `features/graph/`, `features/codebases/`)
- Shared UI in `apps/web/src/components/ui/`

### VS Code Extension

- Standalone package at `apps/vscode/`, does NOT import from other fubbik packages
- Communicates with the fubbik API via HTTP (fetch-based)
- Bundled to CJS via esbuild
- Webview sidebar for chunk list, webview panel for chunk detail and create form

## API Endpoints

### Core
- `GET /api/health` — system health check
- `GET /api/health/knowledge` — knowledge health (orphan, stale, thin chunks)
- `GET /api/stats` — aggregate stats

### Chunks
- `GET /api/chunks` — list (supports `codebaseId`, `global`, `search`, `type`, `tags`, `sort`, etc.)
- `POST /api/chunks` — create (with optional `codebaseIds`, `rationale`, `alternatives`, `consequences`)
- `GET /api/chunks/:id` — detail (includes connections, codebases, appliesTo, fileReferences)
- `PATCH /api/chunks/:id` — update
- `DELETE /api/chunks/:id` — delete
- `GET /api/chunks/export` / `POST /api/chunks/import` — bulk export/import
- `GET /api/chunks/search/semantic` — embedding-based search
- `POST /api/chunks/check-similar` — duplicate detection (requires Ollama)

### Chunk Sub-resources
- `GET/PUT /api/chunks/:id/applies-to` — glob pattern metadata
- `GET/PUT /api/chunks/:id/file-refs` — file reference links
- `GET /api/chunks/:id/history` — version history

### File References
- `GET /api/file-refs/lookup?path=<path>` — reverse lookup (which chunks reference this file)

### Codebases
- `GET /api/codebases` — list
- `POST /api/codebases` — create
- `GET /api/codebases/detect?remoteUrl=&localPath=` — detect codebase from git remote or path
- `PATCH /api/codebases/:id` — update
- `DELETE /api/codebases/:id` — delete

### Templates
- `GET /api/templates` — list (built-in + user's custom)
- `POST /api/templates` — create custom
- `PATCH /api/templates/:id` — update custom
- `DELETE /api/templates/:id` — delete custom (built-in protected)

### Plans
- `GET /api/plans` — list plans (supports `codebaseId`, `status`)
- `POST /api/plans` — create plan (with optional `steps` array)
- `GET /api/plans/:id` — detail (includes steps, chunk refs, progress)
- `PATCH /api/plans/:id` — update plan (title, description, status)
- `DELETE /api/plans/:id` — delete plan
- `POST /api/plans/:id/steps` — add step
- `PATCH /api/plans/:id/steps/:stepId` — update step (status, note, description)
- `DELETE /api/plans/:id/steps/:stepId` — delete step
- `POST /api/plans/:id/chunks` — add chunk reference
- `DELETE /api/plans/:id/chunks/:refId` — remove chunk reference

### Context
- `GET /api/context/for-file?path=<path>` — get chunks relevant to a file (file-refs + appliesTo matching)

### Other
- `GET /api/graph` — graph data (nodes + edges + tags, supports `codebaseId`)
- `POST/DELETE /api/connections` — manage chunk connections
- `GET/POST/PATCH/DELETE /api/tags` — tag management
- `GET/POST/PATCH/DELETE /api/tag-types` — tag type management
- `POST /api/chunks/:id/enrich` — AI enrichment

The server exposes Swagger/OpenAPI at `/docs` (e.g., `http://localhost:3000/docs`).

## Web Pages

- `/` — landing page
- `/dashboard` — overview with stats, recent chunks, favorites
- `/chunks` — chunk list with filters, grouping, search
- `/chunks/new` — create chunk (with template selector)
- `/chunks/:id` — chunk detail (content, connections, metadata, decision context)
- `/chunks/:id/edit` — edit chunk
- `/graph` — knowledge graph visualization (force-directed, hierarchical, radial layouts; tag grouping; path finding)
- `/tags` — tag management
- `/codebases` — codebase management
- `/templates` — template management
- `/knowledge-health` — orphan, stale, and thin chunk detection
- `/plans` — plan list with progress bars
- `/plans/new` — create plan with step builder
- `/plans/:id` — plan detail with interactive step checklist
- `/reviews/queue` — review queue for AI-generated draft chunks
- `/login` — authentication

## CLI Commands

- `fubbik init` — initialize knowledge base
- `fubbik add/get/list/search/update/remove` — chunk CRUD
- `fubbik link/unlink` — manage connections
- `fubbik tags` — tag management
- `fubbik codebase add/list/remove/current` — codebase management
- `fubbik export/import` — bulk operations
- `fubbik enrich` — AI enrichment
- `fubbik health/stats` — system info
- `fubbik plan create/list/show/step-done/add-step/activate/complete` — plan management
- `fubbik check-files [files...] [--staged]` — check files against chunk knowledge
- `fubbik hooks install/uninstall` — git pre-commit hook management
- `fubbik context-for <path>` — generate context document for a file
- `fubbik completions <shell>` — generate shell completions
- List/add/search support `--global` and `--codebase <name>` flags for scoping

## Ollama (Optional)

Required for chunk enrichment (summary, aliases, not_about generation) and semantic search.

- `ollama pull nomic-embed-text` — embedding model
- `ollama pull llama3.2` — generation model for metadata
- `OLLAMA_URL` env var (default: `http://localhost:11434`)
- Without Ollama, all other features work normally

## Environment Variables

- `PORT` — Server port (default: `3000`)
- `CORS_ORIGIN` — Comma-separated allowed origins (default: `http://localhost:3001`)
- `DATABASE_URL` — PostgreSQL connection string
- `BETTER_AUTH_SECRET` — Auth secret (min 32 chars)
- `BETTER_AUTH_URL` — Auth server URL
- `OLLAMA_URL` — Ollama server URL (default: `http://localhost:11434`)

## Common Commands

- `pnpm install` — install dependencies
- `pnpm dev` — start development server (web + API)
- `pnpm build` — build for production
- `pnpm test` — run tests
- `pnpm ci` — run full CI pipeline (type-check, lint, test, build, format check, sherif)
- `pnpm run check-types` — type-check all packages (uses `tsgo`)
- `pnpm db:push` — push database schema
- `pnpm db:studio` — open database UI
- `pnpm kill:server` — kill process on port 3000
- `pnpm kill:web` — kill process on port 3001
- `pnpm kill:all` — kill both

## Local Development with Caddy

Optional HTTPS setup via Caddy reverse proxy:
- `app.fubbik.test:8443` → `localhost:3001` (web)
- `api.fubbik.test:8443` → `localhost:3000` (API)
- Configured in `~/.config/caddy/Caddyfile`
- Domains in `/etc/hosts`

## VS Code Extension

Located at `apps/vscode/`. Build with `cd apps/vscode && node esbuild.mjs`.

Test with: `code --extensionDevelopmentPath=/Users/pontus/projects/fubbik/apps/vscode /Users/pontus/projects/fubbik`

Configure `fubbik.serverUrl` (default: `http://localhost:3000`) and `fubbik.webAppUrl` (default: `http://localhost:3001`) in VS Code settings.

## Maintenance

Keep CLAUDE.md updated when:

- Adding/removing dependencies
- Changing project structure
- Adding new features or services
- Modifying build/dev workflows
- Adding new API endpoints or pages

AI assistants should suggest updates to this file when they notice relevant changes.
