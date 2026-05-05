# Context Fetching & Organization Improvements

## Problem

The context system has grown organically into two parallel pipelines with bugs, performance issues, and missed opportunities. Specifically:

1. **Bugs**: MCP snapshot tools are never registered (syntax error), snapshot retrieval has no auth check, feature overlays are never applied in any context path
2. **Performance**: N+1 queries in `context-for-file` Strategy 2 (up to 1000 individual DB calls when a batch function already exists), `context dir` CLI makes one HTTP call per file
3. **Inconsistency**: Two parallel context systems (old `context-for-file`/`context-export` vs newer `context/` module), duplicated `scoreChunk` function, MCP `sync_claude_md` produces different output than the server endpoint
4. **Retrieval quality**: No semantic search in the main context paths, no ranking within strategies, no path normalization for glob matching, tag filtering uses OR semantics only
5. **Organization**: No scope schema enforcement, codebase scoping inconsistent across strategies, no automatic staleness scanning

## Goal

22 improvements across 4 phases: Fix, Consolidate, Enhance, Organize. Each phase delivers standalone value and is shippable independently.

## Prior Art

The `2026-04-12-ai-context-improvements-design.md` spec designed the `context/` module (resolvers, formatter, snapshots, plan/concept/multi-file context). That work was implemented and is now the "new" system. This spec addresses what was left unfinished: the old system was never retired, bugs crept in, and retrieval quality improvements were deferred.

---

## Phase 1: Fix — Bugs, Performance & Correctness

### 1a. MCP Snapshot Tool Registration

**Problem**: In `packages/mcp/src/context-tools.ts`, the `create_context_snapshot` and `get_context_snapshot` tool definitions are outside the `registerContextTools` function body due to a misplaced closing brace. They are dead code — never registered with the MCP server.

**Fix**: Move the two `server.tool()` calls inside the `registerContextTools` function, before its closing brace.

**Files**: `packages/mcp/src/context-tools.ts`

### 1b. Snapshot Auth Check

**Problem**: `getSnapshot(id)` in `context/snapshot-service.ts` fetches a snapshot by UUID without verifying that the requesting user owns it. Any authenticated user who knows or guesses a snapshot UUID can read another user's snapshot.

**Fix**: Add `userId` as a required parameter to `getSnapshot` and include it in the query predicate: `WHERE id = :id AND userId = :userId`. Return `NotFoundError` if no match (don't leak existence). Update the route to pass the session user ID.

**Files**: `packages/api/src/context/snapshot-service.ts`, `packages/api/src/context/routes.ts`

### 1c. N+1 Query in context-for-file Strategy 2

**Problem**: Strategy 2 (applies-to glob matching) in `getContextForFile` calls `getAppliesToForChunk(c.id)` once per chunk — up to 1000 individual DB queries. A batch function `getAppliesToForChunks(chunkIds: string[])` already exists in `packages/db/src/repository/applies-to.ts` but is never called here.

**Fix**: Replace the per-chunk loop with a single call to `getAppliesToForChunks(allChunkIds)`. Group results by chunk ID in memory, then iterate to test glob matches.

**Files**: `packages/api/src/context-for-file/service.ts`

### 1d. MCP sync_claude_md Divergence

**Problem**: The `sync_claude_md` MCP tool in `packages/mcp/src/context-tools.ts` builds markdown inline by calling `GET /api/chunks?tags=<tag>&limit=100`. It does not call `GET /api/chunks/export/claude-md`. As a result, it omits the Requirements and Active Plans sections that the server endpoint includes.

**Fix**: Change the MCP tool to call `GET /api/chunks/export/claude-md?tag=<tag>` and return the response directly. Remove the inline markdown construction.

**Files**: `packages/mcp/src/context-tools.ts`

### 1e. context dir CLI Batching

**Problem**: The `context dir` CLI command makes one HTTP call per file in the directory (up to 200 serial HTTP requests). The `GET /context/for-files?paths=<csv>` endpoint already exists and accepts multiple paths.

**Fix**: Collect all file paths, then make a single call to `/context/for-files?paths=<csv>&maxTokens=<n>&codebaseId=<id>`. Remove the per-file loop.

**Files**: `apps/cli/src/commands/context-group.ts` (or the specific subcommand file for `context dir`)

---

## Phase 2: Consolidate — Unify the Two Context Systems

### 2a. Single Scoring Function

**Problem**: `scoreChunk` is defined identically in `context-export/service.ts` and `context/utils.ts`. If scoring logic changes, both must be updated manually.

**Fix**: Delete the `scoreChunk` function from `context-export/service.ts`. Import it from `context/utils.ts`.

**Files**: `packages/api/src/context-export/service.ts`, `packages/api/src/context/utils.ts`

### 2b. Route context-for-file Through the New Resolver

**Problem**: `GET /api/context/for-file` returns raw chunks without health scores, stale flags, or structured formatting. The newer `resolveForFiles` in `context/resolvers.ts` already wraps `getContextForFile` and adds enrichment, but the old route doesn't use it.

**Fix**: Change the `GET /api/context/for-file` route handler to call `resolveForFiles([path], userId, codebaseId)` followed by the formatter. Keep the same URL and query parameters for backwards compatibility. The `getContextForFile` function remains as the low-level retrieval engine — only the route wiring changes.

**Backwards compatibility**: The response shape changes from `{ chunks, requirements }` to structured markdown or JSON with health metadata. Add a `format` query param (default: `structured-md`, option: `json-legacy` for the old shape). Known callers:
- CLI `context for` — already supports structured format, no change needed
- MCP `get_context` — already calls `/context/for-files`, not this endpoint
- Web UI `ContextPage` (`apps/web/src/routes/context.tsx`) — uses the old JSON shape; update to consume the new format, or use `format=json-legacy` as a transitional step

**Files**: `packages/api/src/context-for-file/routes.ts`

### 2c. Route context-export Through the New Pipeline

**Problem**: `exportContext` in `context-export/service.ts` has its own chunk-fetching, scoring, and formatting logic that duplicates what the `context/` module provides.

**Fix**: Refactor `exportContext` to:
1. Fetch chunks using the existing approved-first-then-all logic (this is a global retrieval, not file-scoped — no resolver needed)
2. Run the shared `enrichChunks` pipeline from `context/resolvers.ts` for health scores, stale flags, and feature overlays
3. Use the canonical `scoreChunk` from `context/utils.ts` (after 2a this is the only copy)
4. If `forPath` is provided, use `resolveForFiles` for the file-relevance boost (replacing the current inline call to `getContextForFile`)
5. Use `budgetChunks` from `context/utils.ts` for token budgeting
6. Use the formatter from `context/formatter.ts` for markdown output

The `context-export/` directory becomes a thin route file delegating to `context/`.

**Files**: `packages/api/src/context-export/service.ts`, `packages/api/src/context-export/routes.ts`

### 2d. Unify CLAUDE.md Generation

**Problem**: `generateClaudeMd` in `context-export/claude-md.ts` is isolated from the context pipeline — it doesn't use health scores, stale warnings, or the shared formatter.

**Fix**: Move `generateClaudeMd` into the `context/` module (e.g., `context/claude-md.ts`). Have it use the shared enrichment pipeline so CLAUDE.md output includes health signals and stale warnings per chunk. The route stays at `GET /api/chunks/export/claude-md` — only the implementation location changes.

**Files**: `packages/api/src/context-export/claude-md.ts` (move to `packages/api/src/context/claude-md.ts`), `packages/api/src/context-export/routes.ts`

### 2e. Deprecate Old Module Structure

After 2b–2d, the resulting structure:
- `context-for-file/service.ts` — retained as the low-level three-strategy retrieval engine (called by resolvers)
- `context-for-file/glob-match.ts` — retained (used by the service)
- `context-for-file/routes.ts` — thin wrapper calling resolvers + formatter
- `context-export/routes.ts` — thin wrapper calling context/ pipeline
- `context/` — owns all scoring, enrichment, formatting, snapshots, CLAUDE.md generation

No code is deleted in this step — it's a verification checkpoint that all routing goes through the unified pipeline.

---

## Phase 3: Enhance — Retrieval Quality & Missing Capabilities

### 3a. Hybrid Retrieval: Semantic Strategy in context-for-file

**Problem**: The three retrieval strategies (file-ref, applies-to, dependency) use no semantic signals. A chunk about "authentication middleware" won't be found when looking at `src/auth/middleware.ts` unless it has an explicit file-ref or matching glob pattern.

**Fix**: Add Strategy 4 — Semantic Similarity:
1. Construct a search text from the file path: split on `/` and `.`, drop common segments (`src`, `lib`, `index`, file extensions), join remainder with spaces (e.g., `src/auth/middleware.ts` → `"auth middleware"`, `packages/api/src/context/resolvers.ts` → `"api context resolvers"`)
2. Generate an embedding via Ollama
3. Query pgvector for the top 10 nearest chunks (cosine distance)
4. Add results with `matchReason: "semantic"`
5. Lowest priority — runs after Strategies 1–3, only adds chunks not already found

**Ollama dependency**: Gate behind Ollama availability. If Ollama is down or times out, skip silently and log a warning. Same pattern used in `resolveForConcept`.

**Files**: `packages/api/src/context-for-file/service.ts`, `packages/api/src/ollama/client.ts` (may need a helper to embed short text)

### 3b. Score-Based Ranking in context-for-file

**Problem**: Results from `getContextForFile` are returned in Map insertion order with no relevance ranking. A thin, stale chunk found via file-ref outranks a well-maintained, highly-connected chunk found via applies-to.

**Fix**: After all strategies run, score each chunk using `scoreChunk` from `context/utils.ts` plus a strategy bonus:

| Strategy | Bonus |
|----------|-------|
| `file-ref` | +20 |
| `applies-to` | +10 |
| `semantic` | similarity × 10 (0–10 range) |
| `dependency` | +3 |

Sort results descending by total score. This preserves the intuition that direct file references are most relevant while allowing high-quality chunks from other strategies to surface.

**Files**: `packages/api/src/context-for-file/service.ts`

### 3c. Feature Overlay Support in Context Retrieval

**Problem**: None of the context retrieval paths apply active feature deltas. An AI agent using the context system always sees base chunk content, even when the user has active features modifying those chunks.

**Fix**: Add a `resolveFeatureOverlays(chunks, userId)` step in the shared enrichment pipeline in `context/resolvers.ts`:
1. Fetch `user_active_feature` rows for the user
2. If any features are active, batch-fetch `chunk_feature_delta` rows for the matched chunk IDs + active feature IDs
3. Apply `Object.assign(baseChunk, ...deltasAscByPriority)` — same merge logic already used in the chunk detail service
4. Mark overlaid chunks with `featureApplied: true` so the formatter can annotate them (e.g., `[feature: my-feature]`)

Since this goes in the shared enrichment pipeline, all context paths get it automatically after Phase 2 consolidation.

**Files**: `packages/api/src/context/resolvers.ts`, `packages/db/src/repository/feature.ts` (may need a batch query for deltas by chunk IDs)

### 3d. Token Budget Guard for CLAUDE.md

**Problem**: `generateClaudeMd` has no token limit. A heavily-tagged knowledge base produces an unbounded document that may exceed LLM context windows.

**Fix**: Add an optional `maxTokens` query parameter (default: 32000). Use the shared `budgetChunks` function from `context/utils.ts`. When the budget is exceeded, stop adding chunks and append a footer comment:

```
<!-- Truncated: N chunks omitted due to token budget. Increase maxTokens or narrow the tag filter. -->
```

**Files**: `packages/api/src/context/claude-md.ts` (after 2d move), `packages/api/src/context-export/routes.ts`

### 3e. Automatic Staleness Scanning

**Problem**: Age-based staleness detection must be triggered manually via `POST /api/chunks/stale/scan-age`. There is no cron, webhook, or startup trigger.

**Fix**: Add a lightweight scan on server startup and on a configurable interval:
1. In the server bootstrap (`packages/api/src/index.ts` or a dedicated `startup.ts`), call `scanAgeBasedStaleness` after the server is listening
2. Set up a `setInterval` for recurring scans
3. Configurable via `STALENESS_SCAN_INTERVAL_HOURS` env var (default: 24, set to 0 to disable)
4. Log scan results via winston (chunk count flagged, scan duration)
5. Non-blocking — runs in a fire-and-forget Effect, doesn't delay server startup or request handling

**Files**: `packages/api/src/index.ts` (or new `packages/api/src/startup.ts`), `packages/env/src/index.ts` (add env var)

### 3f. Path Normalization for Glob Matching

**Problem**: `globMatch` requires a full path match but there's no normalization. `src/auth/service.ts` and `./src/auth/service.ts` are treated as different paths. Absolute vs relative paths cause silent mismatches.

**Fix**: Add a `normalizePath(path: string)` utility:
1. Strip leading `./`
2. Strip leading `/`
3. Collapse consecutive `/` to single `/`
4. Strip trailing `/`

Apply `normalizePath` to both the input file path and stored patterns before matching in `globMatch`. Also apply when storing new `appliesTo` patterns (write-time normalization).

**Files**: `packages/api/src/context-for-file/glob-match.ts`, `packages/api/src/chunks/service.ts` (or wherever appliesTo patterns are saved)

---

## Phase 4: Organize — Query Semantics, Schema & Consistency

### 4a. Tag AND Semantics

**Problem**: `listChunks` tag filtering uses `IN` (OR semantics). Querying for chunks tagged both `auth` AND `convention` returns chunks with either tag, not both.

**Fix**: Add a `tagMode` query parameter:
- `tagMode=any` (default, current behavior): chunk has at least one of the specified tags
- `tagMode=all`: chunk has all specified tags

SQL for `tagMode=all`:
```sql
SELECT chunk_id FROM chunk_tag
WHERE tag_id IN (:tagIds)
GROUP BY chunk_id
HAVING COUNT(DISTINCT tag_id) = :tagCount
```

Propagate `tagMode` through:
- `listChunks` repository function
- `GET /api/chunks` route (query param)
- `GET /api/chunks/export/context` route
- `GET /api/chunks/export/claude-md` route
- CLI `--tags` flag (add `--tag-mode any|all`)

**Files**: `packages/db/src/repository/chunk.ts`, `packages/api/src/chunks/routes.ts`, `packages/api/src/context-export/routes.ts`, `apps/cli/src/commands/` (relevant commands)

### 4b. Scope Schema Registry

**Problem**: Chunk `scope` is free-form JSONB with no enforcement. Users can set arbitrary keys with no discoverability, autocomplete, or validation.

**Fix**: Add a `scope_key` table:

```
scope_key:
  id: uuid (PK)
  userId: uuid (FK)
  key: text (unique per user)
  description: text
  valueType: enum('string', 'number', 'boolean', 'enum')
  allowedValues: jsonb (nullable — array of allowed values for enum type)
  createdAt: timestamp
```

Behavior:
- **Write-time validation**: When creating/updating a chunk with scope, validate each key against the registry. Unknown keys produce a warning in the response (not a rejection) unless a `strictScope` user setting is enabled.
- **API**: `GET /api/scope-keys` — list registered keys (for UI autocomplete). `POST /api/scope-keys` — register a key. `DELETE /api/scope-keys/:id` — remove.
- **Migration**: No data migration needed — existing scope data is valid. The registry is opt-in.

**Files**: `packages/db/src/schema/scope-key.ts` (new), `packages/db/src/repository/scope-key.ts` (new), `packages/api/src/scope-keys/` (new route + service), `packages/api/src/chunks/service.ts` (add validation on write)

### 4c. Codebase Scoping Consistency in context-for-file

**Problem**: Strategy 1 (`lookupChunksByFilePath`) ignores the `codebaseId` parameter — it returns file-ref matches from any codebase. Strategy 2 scopes by codebase. Strategy 3 scopes by matched codebases. The inconsistency means cross-codebase chunks leak into scoped queries.

**Fix**: Add an optional `codebaseId` parameter to `lookupChunksByFilePath`:
- When provided, join through `chunk_codebase` to filter by codebase (including global chunks with no codebase, matching existing `listChunks` behavior)
- When omitted, current behavior (all codebases) preserved

In the scoring step (3b), file-ref matches from other codebases still appear but with a reduced strategy bonus (+10 instead of the standard +20 from 3b's table) — they're relevant but less targeted. This is a codebase-aware override of the base bonus.

**Files**: `packages/db/src/repository/file-ref.ts`, `packages/api/src/context-for-file/service.ts`

### 4d. Better Token Estimation

**Problem**: `estimateTokens` uses `chars / 4`, which is 15–30% off for technical content with code blocks, unusual characters, and markdown formatting.

**Fix**: Replace with `js-tiktoken` using the `cl100k_base` encoding (used by GPT-4, Claude tokenizers are similar enough for budgeting purposes):
1. `pnpm add js-tiktoken` in `packages/api`
2. Lazy-load the encoder on first call (encoder init is ~50ms)
3. Expose `countTokens(text: string): number` from `context/utils.ts`
4. Fall back to `Math.ceil(text.length / 4)` if encoder load fails
5. Update `budgetChunks` to use `countTokens`

**Files**: `packages/api/src/context/utils.ts`, `packages/api/package.json`

### 4e. Connection-Aware Retrieval

**Problem**: When `context-for-file` finds chunks, their connections are ignored during retrieval. If a chunk about "auth service" depends on a chunk about "session model," only the auth service chunk is returned — the session model context is lost.

**Fix**: Add a connection expansion step after all strategies run and before scoring:
1. Batch-fetch one-hop connections for all matched chunk IDs (single query: `SELECT * FROM connection WHERE sourceId IN (:ids) OR targetId IN (:ids)`)
2. For each connected chunk not already in the result set, add it with `matchReason: "connected"`
3. Cap expansion at 5 new chunks total (pick by connection relation priority: `part_of` > `depends_on` > `extends` > others)
4. Apply a modest strategy bonus of +2

This prevents context explosion while surfacing tightly-coupled knowledge.

**Files**: `packages/api/src/context-for-file/service.ts`, `packages/db/src/repository/connection.ts` (may need a batch query)

### 4f. Update CLAUDE.md Documentation

After all phases, update `/CLAUDE.md` to:
1. Document the unified context pipeline (remove references to two separate systems)
2. Add the `/context/for-plan`, `/context/about`, `/context/for-files`, `/context/snapshot` endpoints to the API reference
3. Document new query parameters: `tagMode`, `maxTokens` on CLAUDE.md endpoint, `STALENESS_SCAN_INTERVAL_HOURS`
4. Document the scoring formula and strategy bonuses
5. Note feature overlay behavior in context retrieval
6. Document the scope key registry

**Files**: `CLAUDE.md`

---

## Dependency Graph

```
Phase 1 (all items independent of each other):
  1a, 1b, 1c, 1d, 1e — can be done in parallel

Phase 2 (sequential within, depends on Phase 1):
  2a → 2b, 2c, 2d (can be parallel after 2a) → 2e

Phase 3 (depends on Phase 2 consolidation):
  3a, 3b — can be parallel (both modify context-for-file/service.ts, but different sections)
  3c — depends on 2e (needs unified enrichment pipeline)
  3d — depends on 2d (needs CLAUDE.md in context/ module)
  3e, 3f — independent, can be done anytime after Phase 1

Phase 4 (mostly independent, depends on Phase 2 for scoring/pipeline work):
  4a — independent
  4b — independent (new table + service)
  4c — depends on 3b (scoring changes)
  4d — depends on 2a (single scoring function)
  4e — depends on 3b (scoring in context-for-file)
  4f — depends on all other phases
```

## Non-Goals

- **Replacing pgvector with a dedicated vector DB**: pgvector is sufficient at the current scale
- **Multi-tenant isolation**: fubbik is single-user or small-team; the auth fix (1b) is about correctness, not enterprise multi-tenancy
- **Real-time context streaming**: context requests are batch, not streamed
- **Rewriting glob matching from scratch**: normalization (3f) addresses the practical issues; the hand-rolled regex converter works correctly for supported patterns
