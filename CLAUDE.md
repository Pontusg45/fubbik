# fubbik

This file provides context about the project for AI assistants.

## Project Overview

- **Ecosystem**: Typescript
- **Purpose**: Local-first knowledge framework for storing, navigating, and evolving structured knowledge about codebases. Designed for both humans (web UI, graph visualization) and machines (CLI, API, VS Code extension, MCP server).

## Tech Stack

- **Runtime**: bun
- **Package Manager**: pnpm

### Frontend

- Framework: tanstack-start (SSR via entry-server.ts)
- CSS: tailwind
- UI Library: shadcn-ui (built on @base-ui/react — uses `render` prop, NOT `asChild`)
- Graph: @xyflow/react (React Flow)

### Backend

- Framework: elysia
- API Client: elysia eden treaty
- Validation: elysia `t` schema (NOT arktype — arktype was removed)
- Error Handling: effect (typed errors via Effect.tryPromise, tagged errors)

### Database

- Database: postgres
- ORM: drizzle
- Extensions: pgvector (embeddings), pg_trgm (fuzzy text search)

### Authentication

- Provider: better-auth

### Additional Features

- Testing: vitest
- AI: Ollama (local LLM for enrichment + embeddings — vercel-ai SDK was removed)
- Embeddings: Ollama (nomic-embed-text) for local vector embeddings
- Logging: winston
- Observability: opentelemetry
- MCP: Model Context Protocol server for AI agent integration

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
│   ├── auth/        # Authentication (Better Auth)
│   ├── config/      # Shared TypeScript config
│   ├── db/          # Database schema (Drizzle ORM)
│   ├── env/         # Environment validation
│   └── mcp/         # MCP server (AI agent tools)
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
- `embeddingUpdatedAt` (tracks when embedding was last refreshed)
- AI-generated: `summary`, `aliases`, `notAbout`
- Health score (computed on-demand): freshness + completeness + richness + connectivity (0-100)
- Version history (append-only `chunk_version` table)

### Codebases & Workspaces

Chunks can be organized per-codebase. A codebase is identified by git remote URL (normalized) or local paths. Chunks can belong to multiple codebases or none (global).
- `codebase` table with `remoteUrl`, `localPaths`, `name`
- `chunk_codebase` join table (many-to-many)
- `workspace` table groups related codebases (e.g., frontend + backend + infra)
- `workspace_codebase` join table
- CLI auto-detects codebase via git remote
- Web UI has a codebase/workspace switcher in the nav

### Connections

Directed edges between chunks: `sourceId → targetId` with a `relation` type (related_to, part_of, depends_on, extends, references, supports, contradicts, alternative_to). Connections are global (not codebase-scoped), enabling cross-project knowledge linking.

### Templates

Built-in + user-created chunk templates (Convention, Architecture Decision, Runbook, API Endpoint). Built-in templates are seeded via SQL migration and are read-only. Users can duplicate and customize.

### Plans

The central unit of work. Each plan holds a description, linked requirements, structured analyze fields, and enriched tasks.

- `plan` table: `title`, `description` (markdown), `status` (`draft | analyzing | ready | in_progress | completed | archived` — labels only, ungated), `userId`, `codebaseId`, `completedAt`
- `plan_requirement` — many-to-many link to existing `requirement` entities at the plan level
- `plan_analyze_item` — discriminated table holding five kinds: `chunk`, `file`, `risk`, `assumption`, `question`, each with kind-specific metadata (severity for risks, verified flag for assumptions, answer for questions, line range for files)
- `plan_task` — enriched tasks with `title`, `description`, `acceptanceCriteria` (JSONB string array), `status`
- `plan_task_chunk` — many-to-many linking tasks to multiple chunks with a relation (`context | created | modified`)
- `plan_task_dependency` — task dependencies; marking a task `done` auto-unblocks dependents in `blocked` state
- Web UI: `/plans` list, `/plans/new` (simple form), `/plans/:id` (sticky header + four sections: description, requirements, analyze, tasks)
- CLI: `fubbik plan create/list/show/status/add-task/task-done/link-requirement`

### Requirements

BDD-style requirements with Given/When/Then steps.
- `requirement` table with `title`, `description`, `steps` (JSONB array of `{keyword, text}`), `status` (passing/failing/untested), `priority` (must/should/could/wont)
- `requirement_chunk` join table linking requirements to chunks
- Plan steps can link to requirements via `requirementId` for full traceability
- Requirements auto-update to "passing" when implementation sessions complete

### Chunk Health Scores

Per-chunk health scores (0-100) computed on-demand from:
- **Freshness** (0-25): days since last update
- **Completeness** (0-25): has rationale/alternatives/consequences
- **Richness** (0-25): content length + AI enrichment
- **Connectivity** (0-25): number of connections
Exposed via chunk detail API response and shown as a badge on the detail page.

### Staleness Detection

Proactive detection of chunks that may need attention:
- `chunk_staleness` table tracks flags with `reason` (file_changed, age, diverged_duplicate), `detail`, and dismiss/suppress state
- `staleness_scan` table tracks per-codebase last scanned git commit SHA for incremental scans
- Age-based detection: flags chunks not updated in 90+ days (configurable threshold)
- Flags are dismissable (soft-delete with timestamp) or permanently suppressible (for duplicate pairs)
- Surfaced across the UI: dashboard "Attention Needed" widget, nav badge on Dashboard link, amber banners on chunk detail pages

## Architecture Patterns

### Backend: Repository -> Service -> Route

- **Repositories** (`packages/db/src/repository/`): Return `Effect<T, DatabaseError>`. Pure data access, no business logic.
- **Services** (`packages/api/src/*/service.ts`): Compose repository Effects, add business logic, introduce `NotFoundError`/`AuthError`/`ValidationError`.
- **Routes** (`packages/api/src/*/routes.ts`): Call `Effect.runPromise(requireSession(ctx).pipe(...))`. Errors propagate to global `.onError` handler.
- **Global error handler** (`packages/api/src/index.ts`): Extracts Effect errors from FiberFailure, maps `_tag` to HTTP status codes (ValidationError->400, AuthError->401, NotFoundError->404, DatabaseError->500).

### Frontend: Feature-based Structure

- Route files in `apps/web/src/routes/`
- Feature components in `apps/web/src/features/` (e.g., `features/auth/`, `features/graph/`, `features/codebases/`, `features/plans/`, `features/chunks/`)
- Shared UI in `apps/web/src/components/ui/`
- Shared page components: `PageContainer`, `PageHeader`, `PageLoading`, `PageEmpty` in `components/ui/page.tsx`
- UI components use base-ui `render` prop pattern (NOT Radix `asChild`)
- `DropdownMenuSeparator` and `DropdownMenuLabel` use plain HTML elements (NOT base-ui primitives) to avoid Menu.Group context requirement

### VS Code Extension

- Standalone package at `apps/vscode/`, does NOT import from other fubbik packages
- Communicates with the fubbik API via HTTP (fetch-based)
- Bundled to CJS via esbuild
- Webview sidebar with type/tag/sort filtering, file-aware chunk surfacing
- Webview panels for chunk detail, chunk creation, chunk editing
- Status bar showing chunk count
- Commands: search, quick-add note, open graph/dashboard in browser

### Context Modules (context-export vs context-for-file)

Two distinct context services exist in `packages/api/src/`:
- **context-for-file** (`GET /api/context/for-file`): Given a file path, finds relevant chunks via three strategies: direct file-ref matches, appliesTo glob patterns, and dependency-based codebase matching. Returns a flat list of chunks with `matchReason`.
- **context-export** (`GET /api/chunks/export/context`): Token-budgeted export for AI consumption. Scores all chunks by health, type, connections, and review status, optionally boosting file-relevant chunks (delegates to context-for-file internally), then greedily fills a token budget. Also powers CLAUDE.md generation (`GET /api/chunks/export/claude-md`).

### MCP Server

- Located at `packages/mcp/`
- Tools organized by domain: `tools.ts` (core), `session-tools.ts`, `plan-tools.ts`, `requirement-tools.ts`, `suggestion-tools.ts`, `context-tools.ts`
- Each file exports `registerXTools(server: McpServer)` function
- Uses `apiFetch` helper for server communication
- Key tools: `create_plan`, `create_plan_from_requirements`, `import_plan_markdown`, `begin_implementation`, `mark_plan_step`, `sync_claude_md`

## API Endpoints

### Core
- `GET /api/health` — system health check
- `GET /api/health/knowledge` — knowledge health (orphan, stale, thin chunks, stale embeddings, file refs)
- `GET /api/stats` — aggregate stats

### Chunks
- `GET /api/chunks` — list (supports `codebaseId`, `workspaceId`, `global`, `allCodebases`, `search`, `type`, `tags`, `sort`, etc.)
- `POST /api/chunks` — create (with optional `codebaseIds`, `rationale`, `alternatives`, `consequences`)
- `GET /api/chunks/:id` — detail (includes connections, codebases, appliesTo, fileReferences, healthScore)
- `PATCH /api/chunks/:id` — update (auto-re-enriches on title/content change)
- `DELETE /api/chunks/:id` — delete
- `GET /api/chunks/export` / `POST /api/chunks/import` — bulk export/import
- `GET /api/chunks/search/semantic` — embedding-based search
- `GET /api/chunks/search/federated` — cross-codebase search with codebase names
- `POST /api/chunks/check-similar` — duplicate detection (requires Ollama)
- `GET /api/chunks/export/context` — token-budgeted context export (supports `forPath` relevance boost)
- `GET /api/chunks/export/claude-md` — CLAUDE.md generation from tagged chunks
- `POST /api/chunks/import-docs` — bulk import from markdown files with frontmatter parsing

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

### Workspaces
- `GET /api/workspaces` — list workspaces
- `POST /api/workspaces` — create
- `GET /api/workspaces/:id` — detail with codebases
- `PATCH /api/workspaces/:id` — update
- `DELETE /api/workspaces/:id` — delete
- `POST /api/workspaces/:id/codebases` — add codebase
- `DELETE /api/workspaces/:id/codebases/:codebaseId` — remove codebase

### Templates
- `GET /api/templates` — list (built-in + user's custom)
- `POST /api/templates` — create custom
- `PATCH /api/templates/:id` — update custom
- `DELETE /api/templates/:id` — delete custom (built-in protected)

### Plans
- `GET /api/plans` — list (filters: `codebaseId`, `status`, `requirementId`, `includeArchived`)
- `POST /api/plans` — create (body: `title`, `description?`, `codebaseId?`, `requirementIds?`, `tasks?`)
- `GET /api/plans/:id` — detail (plan + requirements + analyze grouped by kind + tasks with chunks + dependencies)
- `PATCH /api/plans/:id` — update title/description/status/codebaseId
- `DELETE /api/plans/:id`
- `POST /api/plans/:id/requirements` / `DELETE /api/plans/:id/requirements/:requirementId` / `POST /api/plans/:id/requirements/reorder`
- `GET /api/plans/:id/analyze` / `POST /api/plans/:id/analyze` / `PATCH /api/plans/:id/analyze/:itemId` / `DELETE /api/plans/:id/analyze/:itemId` / `POST /api/plans/:id/analyze/reorder`
- `POST /api/plans/:id/tasks` / `PATCH /api/plans/:id/tasks/:taskId` / `DELETE /api/plans/:id/tasks/:taskId` / `POST /api/plans/:id/tasks/reorder`
- `POST /api/plans/:id/tasks/:taskId/chunks` / `DELETE /api/plans/:id/tasks/:taskId/chunks/:linkId`

### Requirements
- `GET /api/requirements` — list (supports filters: status, priority, codebaseId, search)
- `GET /api/requirements/stats` — counts by status
- `POST /api/requirements` — create with BDD steps
- `GET /api/requirements/:id` — detail
- `PATCH /api/requirements/:id` — update
- `DELETE /api/requirements/:id` — delete
- `GET /api/requirements/coverage` — chunk coverage metrics
- `GET /api/requirements/traceability` — requirement → plan → session traceability

### Context
- `GET /api/context/for-file?path=<path>&deps=<deps>` — chunks relevant to a file (file-refs + appliesTo + dependency matching)

### Staleness
- `GET /api/chunks/stale` — list undismissed staleness flags (supports `reason`, `codebaseId`, `limit`)
- `GET /api/chunks/stale/count` — count of undismissed flags (supports `codebaseId`)
- `POST /api/chunks/:id/dismiss-staleness` — dismiss a staleness flag (`:id` is the flag ID)
- `POST /api/chunks/suppress-duplicate` — permanently suppress a duplicate pair
- `POST /api/chunks/stale/scan-age` — trigger age-based staleness scan (body: `codebaseId?`, `thresholdDays?`)

### Other
- `GET /api/graph` — graph data (nodes + edges + tags, supports `codebaseId`, `workspaceId`)
- `POST/DELETE /api/connections` — manage chunk connections
- `GET/POST/PATCH/DELETE /api/tags` — tag management
- `GET/POST/PATCH/DELETE /api/tag-types` — tag type management
- `POST /api/chunks/:id/enrich` — AI enrichment

The server exposes Swagger/OpenAPI at `/docs` (e.g., `http://localhost:3000/docs`).

## Web Pages

- `/` — landing page with animated constellation background
- `/dashboard` — overview with clickable stats, recent chunks, favorites, recently viewed, health summary, "Attention Needed" staleness widget
- `/chunks` — chunk list with infinite scroll, filters, grouping, search, kanban view, inline row actions, quick review status toggle
- `/chunks/new` — create chunk (with template selector, duplicate detection, autosave)
- `/chunks/:id` — chunk detail with collapsible sections, inline tag editor, health badge, connection arrows, dependency tree (grouped by relation type), related chunk suggestions (embedding-based), staleness banners
- `/chunks/:id/edit` — edit chunk (with autosave, glob validation)
- `/graph` — knowledge graph visualization (force-directed, hierarchical, radial layouts; tag grouping with clickable legend filters; path finding; workspace view with cross-codebase edge styling; focus mode via double-click; saveable filter presets)
- `/requirements` — tabbed page: Requirements list | Plans list | Traceability dashboard
- `/requirements/:id` — requirement detail with plan coverage, BDD steps, export
- `/search` — full-text search across all entity types
- `/tags` — tag management
- `/codebases` — codebase management
- `/workspaces` — workspace management (group codebases)
- `/templates` — template management
- `/context` — file-path context search (find chunks relevant to a file)
- `/knowledge-health` — orphan, stale, thin chunks, stale embeddings, file references, knowledge gaps with "Create Requirement" action
- `/coverage` — chunk coverage + traceability views
- `/plans` — list of plans with status pills and task progress
- `/plans/new` — simple form (title + description + optional codebase)
- `/plans/:id` — sticky header + four sections (Description, Requirements, Analyze, Tasks)
- `/import` — dedicated markdown docs import with folder upload, preview table, codebase selection
- `/login` — authentication
- `/settings` — user/codebase/instance settings
- `/activity` — activity log with action type + entity type filters
- `/vocabulary` — controlled vocabulary management
- `/docs` — API documentation (Swagger UI)

## CLI Commands

- `fubbik init` — initialize knowledge base
- `fubbik add/get/list/search/update/remove` — chunk CRUD (colored output, table formatting)
- `fubbik add -i` — interactive chunk creation (opens $EDITOR)
- `fubbik add --template <name>` — create from template
- `fubbik link/unlink` — manage connections (with confirmation prompts)
- `fubbik tags` — tag management
- `fubbik codebase add/list/remove/current` — codebase management
- `fubbik export/import` — bulk operations
- `fubbik enrich` — AI enrichment
- `fubbik health/stats` — system info
- `fubbik plan create <title>` — create a plan
- `fubbik plan list` — list plans
- `fubbik plan show <id>` — show plan detail
- `fubbik plan status <id> <status>` — update plan status
- `fubbik plan add-task <planId> <title>` — add a task
- `fubbik plan task-done <planId> <taskId>` — mark a task done
- `fubbik plan link-requirement <planId> <requirementId>` — link a requirement
- `fubbik quick "title"` — one-liner chunk creation (auto-detects codebase, supports `--type`, `--tags`, pipe stdin for content)
- `fubbik check-files [files...] [--staged]` — check files against chunk knowledge
- `fubbik hooks install/uninstall` — git pre-commit hook management
- `fubbik context` — token-budgeted context export (with `--for <path>` file relevance boost)
- `fubbik context-for <path>` — generate context document for a file (with `--include-deps`)
- `fubbik context-dir <dir>` — generate CLAUDE.md-style context for a directory
- `fubbik sync-claude-md` — generate/watch .claude/CLAUDE.md from tagged chunks
- `fubbik import <path>` — import chunks from JSON file, .md file, or directory (use `--server`/`--codebase` for server mode)
- `fubbik completions <shell>` — generate shell completions (zsh)
- List/add/search support `--global` and `--codebase <name>` flags for scoping

## Seed Data

Run `pnpm seed` to populate the database with sample data:
- 1 dev user, 24 documentation chunks about fubbik itself
- 36 connections, 5 tag types, 40 tags, 69 chunk-tag associations
- 1 codebase (fubbik), 9 applies-to patterns, 5 file references
- 2 use cases, 3 requirements with BDD steps
- 1 plan with 5 steps, 10 vocabulary entries, 1 collection, 1 workspace
- Optional Ollama enrichment (summary, aliases, embeddings)

## Ollama (Optional)

Required for chunk enrichment (summary, aliases, not_about generation), semantic search, and duplicate detection.

- `ollama pull nomic-embed-text` — embedding model
- `ollama pull llama3.2` — generation model for metadata
- `OLLAMA_URL` env var (default: `http://localhost:11434`)
- Without Ollama, all other features work normally
- Embeddings auto-refresh on title/content edit (fire-and-forget, logged on failure)
- Enrichment concurrency: 3 (for enrich-all)

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
- `pnpm seed` — seed database with sample data
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

Features: sidebar with filtering, inline editing, file-aware chunk surfacing, search, quick-add, status bar.

## Maintenance

Keep CLAUDE.md updated when:

- Adding/removing dependencies
- Changing project structure
- Adding new features or services
- Modifying build/dev workflows
- Adding new API endpoints or pages

AI assistants should suggest updates to this file when they notice relevant changes.
