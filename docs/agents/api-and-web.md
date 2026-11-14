# API endpoints and web pages

Reference for CLAUDE.md. Read when adding or changing HTTP routes or web routes. The OpenAPI document (`openapi.json`) is the source of truth; this list is a convenience map.

### Core

- `GET /api/health` — system health check
- `GET /api/health/knowledge` — knowledge health (orphan, stale, thin chunks, stale embeddings, file refs)
- `GET /api/stats` — aggregate stats
- `GET /api/capabilities` — enabled `fubbik.toml` feature groups (unauthenticated; see "Feature switches")

### Chunks

- `GET /api/chunks` — list (supports `spaceId`, `search`, `type`, `tags` (comma-separated, any-of), `origin`, `reviewStatus`, `after`,
  `enrichment`, `minConnections`, `sort`, `limit`, `offset`); `tagMode=any|all` exists only on `GET /api/chunks/grouped`
- `POST /api/chunks` — create (with optional `spaceIds`, `rationale`, `alternatives`, `consequences`)
- `GET /api/chunks/:id` — detail (includes connections, spaces, appliesTo, fileReferences, healthScore)
- `PATCH /api/chunks/:id` — update (auto-re-enriches on title/content change)
- `DELETE /api/chunks/:id` — delete
- `GET /api/chunks/search/semantic` — embedding-based search
- `GET /api/chunks/search/federated` — cross-space search with space names
- `POST /api/chunks/check-similar` — duplicate detection (requires Ollama)
- `GET /api/chunks/export/context` — token-budgeted context export (supports `forPath` relevance boost)
- `GET /api/chunks/export/claude-md` — CLAUDE.md generation from tagged chunks (supports `maxTokens` budget)
- `POST /api/chunks/import-docs` — bulk import from markdown files with frontmatter parsing

### Chunk Sub-resources

- `GET/PUT /api/chunks/:id/applies-to` — glob pattern metadata
- `GET/PUT /api/chunks/:id/file-refs` — file reference links
- `GET /api/chunks/:id/history` — version history

### File References

- `GET /api/file-refs/lookup?path=<path>` — reverse lookup (which chunks reference this file)

### Spaces

- `GET /api/spaces` — list
- `POST /api/spaces` — create
- `GET /api/spaces/detect?remoteUrl=&localPath=` — detect space from git remote or path
- `PATCH /api/spaces/:id` — update
- `DELETE /api/spaces/:id` — delete

> The Rust server keeps one deprecated `/api/codebases` route, `GET /api/codebases/:id/generate-instructions` (alias of the space
> route). The rest of the old `/api/codebases` alias was not ported.

### Workspaces

- `GET /api/workspaces` — list workspaces
- `POST /api/workspaces` — create
- `GET /api/workspaces/:id` — detail with spaces
- `PATCH /api/workspaces/:id` — update
- `DELETE /api/workspaces/:id` — delete
- `POST /api/workspaces/:id/spaces` — add space
- `DELETE /api/workspaces/:id/spaces/:spaceId` — remove space

### Templates

- `GET /api/templates` — list (built-in + user's custom)
- `POST /api/templates` — create custom
- `PATCH /api/templates/:id` — update custom
- `DELETE /api/templates/:id` — delete custom (built-in protected)

### Plans

- `GET /api/plans` — list (filters: `spaceId`, `status`, `requirementId`, `includeArchived`)
- `POST /api/plans` — create (body: `title`, `description?`, `spaceId?`, `requirementIds?`, `tasks?`)
- `GET /api/plans/:id` — detail (plan + requirements + analyze grouped by kind + tasks with chunks + dependencies)
- `PATCH /api/plans/:id` — update title/description/status/spaceId
- `DELETE /api/plans/:id`
- `POST /api/plans/:id/requirements` / `DELETE /api/plans/:id/requirements/:requirementId` / `POST /api/plans/:id/requirements/reorder`
- `GET /api/plans/:id/analyze` / `POST /api/plans/:id/analyze` / `PATCH /api/plans/:id/analyze/:itemId` /
  `DELETE /api/plans/:id/analyze/:itemId` / `POST /api/plans/:id/analyze/reorder`
- `POST /api/plans/:id/tasks` / `PATCH /api/plans/:id/tasks/:taskId` / `DELETE /api/plans/:id/tasks/:taskId` /
  `POST /api/plans/:id/tasks/reorder`
- `POST /api/plans/:id/tasks/:taskId/chunks` / `DELETE /api/plans/:id/tasks/:taskId/chunks/:linkId`

### Requirements

- `GET /api/requirements` — list (supports filters: status, priority, spaceId, search)
- `GET /api/requirements/stats` — counts by status
- `POST /api/requirements` — create with BDD steps
- `GET /api/requirements/:id` — detail
- `PATCH /api/requirements/:id` — update
- `DELETE /api/requirements/:id` — delete
- `GET /api/requirements/coverage` — chunk coverage metrics
- `GET /api/requirements/traceability` — requirement → plan → session traceability

### Context

- `GET /api/context/for-file?path=<path>&spaceId=<id>&deps=<csv>&format=<fmt>&maxTokens=<n>` — chunks relevant to a file (five
  strategies: file-ref, applies-to, dependency, semantic, connected). Formats: `structured-md` (default), `structured-json`, `json-legacy`
- `GET /api/context/for-plan?planId=<id>&maxTokens=<n>&format=<fmt>` — chunks linked to a plan
- `GET /api/context/about?q=<concept>&maxTokens=<n>&spaceId=<id>&format=<fmt>` — semantic + text search for a concept
- `GET /api/context/for-files?paths=<csv>&maxTokens=<n>&spaceId=<id>&format=<fmt>` — chunks for multiple files
- `POST /api/context/snapshot` — create frozen context snapshot
- `GET /api/context/snapshot/:id` — retrieve snapshot (user-scoped, auth-checked)
- `GET /api/context/snapshots` — list user's snapshots
- `DELETE /api/context/snapshot/:id` — delete snapshot (user-scoped)

### Staleness

- `GET /api/chunks/stale` — list undismissed staleness flags (supports `reason`, `spaceId`, `limit`)
- `GET /api/chunks/stale/count` — count of undismissed flags (supports `spaceId`)
- `POST /api/chunks/:id/dismiss-staleness` — dismiss a staleness flag (`:id` is the flag ID)
- `POST /api/chunks/suppress-duplicate` — permanently suppress a duplicate pair
- `POST /api/chunks/stale/scan-age` — trigger age-based staleness scan (body: `spaceId?`, `thresholdDays?`)

### Features

- `GET /api/features` — list (filters: `spaceId`, `status`, `search`), includes delta count
- `POST /api/features` — create (body: `name`, `description?`, `priority?`, `color?`, `spaceIds?`)
- `GET /api/features/:id` — detail with spaces and deltas
- `PATCH /api/features/:id` — update
- `DELETE /api/features/:id` — delete (cascades deltas)
- `POST /api/features/:id/merge` — merge all deltas into base chunks permanently
- `POST /api/features/:id/reorder` — change priority
- `GET /api/features/active` — get user's active feature IDs
- `PUT /api/features/active` — set active features (body: `featureIds`)
- `GET /api/features/:id/deltas` — all deltas for a feature
- `GET /api/chunks/:id/deltas` — all deltas across features for a chunk
- `PUT /api/chunks/:id/deltas/:featureId` — upsert delta (body: `delta`)
- `DELETE /api/chunks/:id/deltas/:featureId` — delete delta

### Behavioral Matrices

- `GET/POST /api/matrices` — list/create matrices (`layer`, `spaceId` filters)
- `GET /api/matrices/:id` / `GET /api/matrices/:id/view` — detail / computed grid (cell statuses + summary)
- `PATCH/DELETE /api/matrices/:id`
- `POST/PATCH/DELETE /api/matrices/:id/dimensions[...]` + `/reorder`
- `POST/PATCH/DELETE /api/matrices/:id/rules[...]` + `/reorder` (rule body accepts `rationale`, `alternatives`, `consequences`,
  `counterexample`)
- `GET /api/matrices/:id/rules/:ruleId/history` — append-only rule version snapshots
- `PUT /api/matrices/:id/cells` — toggle a cell
- `POST/DELETE/GET /api/matrices/:id/cells/:cellId/requirements[...]` — link BDD requirements
- `POST/DELETE/GET /api/matrices/:id/cells/:cellId/code[...]` — link code (`kind = file | symbol | test`)
- `POST/GET /api/matrices/:id/cells/:cellId/test-results` — record/list `pass`/`fail` verification outcomes
- `GET /api/matrices/behaviors-for-file?path=` — reverse lookup: which behaviors govern a file

### Other

- `GET /api/graph` — graph data (nodes + edges + tags, supports `spaceId`, `workspaceId`)
- `POST/DELETE /api/connections` — manage chunk connections
- `GET/POST/PATCH/DELETE /api/tags` — tag management
- `GET/POST/PATCH/DELETE /api/tag-types` — tag type management
- `POST /api/chunks/:id/enrich` — AI enrichment
- `GET /api/scope-keys` — list registered scope keys
- `POST /api/scope-keys` — register a scope key (key, description, valueType, allowedValues)
- `DELETE /api/scope-keys/:id` — delete a scope key

The Rust server serves the OpenAPI document at `GET /api/openapi.json` and a Swagger UI page at `GET /api/docs` (both unauthenticated; the UI
loads from the jsdelivr CDN at a pinned `swagger-ui-dist` version, see `crates/fubbik-api/src/api_docs.rs`). `fubbik openapi` prints the
OpenAPI document; the checked-in copy is `openapi.json`, from which `pnpm gen:api` generates `packages/client/src/api-types.ts`.

## Web Pages

- `/` — landing page with animated constellation background
- `/dashboard` — overview with clickable stats, recent chunks, favorites, recently viewed, health summary, "Attention Needed" staleness
  widget
- `/chunks` — chunk list with infinite scroll, filters, grouping, search, kanban view, inline row actions, quick review status toggle
- `/chunks/new` — create chunk (with template selector, duplicate detection, autosave)
- `/chunks/:id` — chunk detail with collapsible sections, inline tag editor, health badge, connection arrows, dependency tree (grouped by
  relation type), related chunk suggestions (embedding-based), staleness banners, feature overlay indicators
- `/chunks/:id/edit` — edit chunk (with autosave, glob validation, save-to-feature dialog when features are active)
- `/features` — feature management (create, activate/deactivate, merge into base, archive, delete)
- `/graph` — knowledge graph visualization (force-directed, hierarchical, radial layouts; tag grouping with clickable legend filters; path
  finding; workspace view with cross-space edge styling; focus mode via double-click; saveable filter presets)
- `/requirements` — tabbed page: Requirements list | Plans list | Traceability dashboard
- `/requirements/:id` — requirement detail with plan coverage, BDD steps, export
- `/search` — full-text search across all entity types
- `/tags` — tag management
- `/spaces` — space management
- `/workspaces` — workspace management (group spaces)
- `/templates` — template management
- `/context` — file-path context search (find chunks relevant to a file)
- `/knowledge-health` — orphan, stale, thin chunks, stale embeddings, file references, knowledge gaps with "Create Requirement" action
- `/coverage` — chunk coverage + traceability views
- `/plans` — list of plans with status pills and task progress
- `/plans/new` — simple form (title + description + optional space)
- `/plans/:id` — sticky header + four sections (Description, Requirements, Analyze, Tasks)
- `/import` — dedicated markdown docs import with folder upload, preview table, space selection
- `/login` — authentication
- `/settings` — user/space/instance settings
- `/activity` — activity log with action type + entity type filters
- `/vocabulary` — controlled vocabulary management
- `/docs` — imported document browser, plus an "API" tab that embeds the server's Swagger UI (`GET /api/docs` on the API)

