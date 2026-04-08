# Improvements Roadmap

Prioritized backlog of improvements organized into phases. Each phase builds on the previous. Items within a phase can be parallelized.

---

## Phase 1: Polish Current Work (foundations)

These fix rough edges in what we just built. Do these first — they make everything else better.

### 1.1 AGE Connection Initialization
**Why:** `LOAD 'age'` runs per-query, adding overhead. Fix once, benefit everywhere.
**Scope:** Modify `packages/db/src/index.ts` to add a pool-level `afterConnect` hook that runs `LOAD 'age'; SET search_path = ag_catalog, "$user", public;` on each new connection. Remove per-query init from `age/client.ts`.
**Files:** `packages/db/src/index.ts`, `packages/db/src/age/client.ts`
**Effort:** Small

### 1.2 Search Filter Inline Popovers
**Why:** `window.prompt` for filter values is unusable. Replace with inline autocomplete popovers.
**Scope:** Rewrite `add-filter-dropdown.tsx` — when a field is selected, show a small popover with a text input that calls `GET /api/search/autocomplete` as you type. For type/origin/review, show preset buttons instead.
**Files:** `apps/web/src/features/search/add-filter-dropdown.tsx`
**Effort:** Medium

### 1.3 Graph Hop Distance Annotation
**Why:** Neighborhood search returns IDs but doesn't tell you how far each result is from the reference chunk.
**Scope:** In `packages/api/src/search/service.ts`, after getting neighborhood IDs, run a BFS-like Cypher query: `MATCH p = shortestPath((a:chunk {id: $ref})-[*]-(b:chunk {id: $id})) RETURN length(p) AS hops` for each result (batched). Populate `graphContext.hopDistance`.
**Files:** `packages/api/src/search/service.ts`, `packages/db/src/age/query.ts` (add `getHopDistances` function)
**Effort:** Medium

### 1.4 Health Score on Search Results
**Why:** Search results don't show health scores. Users can't quickly spot unhealthy chunks.
**Scope:** Extend the search service to compute or fetch health scores for result chunks. Add a small colored badge (green/amber/red) to `search-results.tsx`.
**Files:** `packages/api/src/search/service.ts`, `apps/web/src/features/search/search-results.tsx`
**Effort:** Small

### 1.5 AGE Integration Tests
**Why:** The sync and query layers have no tests. Regressions will be silent.
**Scope:** Add vitest tests for `age/sync.ts` (create/delete vertex/edge) and `age/query.ts` (findShortestPath, getNeighborhood, etc.) running against the local PG+AGE instance. Requires AGE extension in test DB.
**Files:** Create `packages/db/src/age/sync.test.ts`, `packages/db/src/age/query.test.ts`
**Effort:** Medium

---

## Phase 2: Search Power Features

Build on the query builder to make search genuinely powerful.

### 2.1 Semantic Search Clause
**Why:** The query builder has text and graph search but not embedding-based similarity.
**Scope:** Add `similar-to:"some text"` clause. In the search service, generate an embedding via Ollama, query pgvector for top-N similar chunks, return as an ID set (same pattern as graph clauses). Combine with other clauses: "chunks similar to X within 2 hops of Y".
**Files:** `packages/api/src/search/service.ts` (add semantic clause handler), `packages/api/src/search/parser.ts` (add `similar-to` syntax)
**Effort:** Medium
**Depends on:** Ollama running

### 2.2 Search from Nav
**Why:** Users must navigate to `/search` to search. A compact input in the nav would make search accessible from anywhere.
**Scope:** Add a search input to the nav bar (in `apps/web/src/routes/__root.tsx` or the nav component). On focus, show recent queries. On Enter, navigate to `/search?q=...`. Keyboard shortcut: `/` to focus from any page.
**Files:** Nav component (find in `apps/web/src/`), `apps/web/src/routes/__root.tsx`
**Effort:** Medium

### 2.3 Query Builder on Chunks Page
**Why:** The chunks page has a separate filter popover that's less powerful than the query builder. Unify the filter experience.
**Scope:** Replace the filter popover on `/chunks` with the FilterPills + AddFilterDropdown components. Reuse `useQueryBuilder`. The query builder drives the existing `GET /api/chunks` endpoint by converting clauses to query params (same `buildStandardQuery` logic).
**Files:** `apps/web/src/routes/chunks.index.tsx`, potentially extract shared query-to-params logic
**Effort:** Large (the chunks page is 1000+ lines)

### 2.4 Onboarding for Search
**Why:** Empty query builder is intimidating for new users.
**Scope:** When no clauses are active, show example queries as clickable suggestions: "type:reference tag:api", "near:'Architecture Overview' hops:2", "origin:ai review:draft". Clicking loads the clauses.
**Files:** `apps/web/src/routes/search.tsx`
**Effort:** Small

### 2.5 Bulk Operations from Search Results
**Why:** Finding chunks via search is great, but acting on them requires opening each one.
**Scope:** Add checkbox selection to search results. When items are selected, show a bulk action bar: "Add tag", "Link to requirement", "Create connection between selected". Reuse the existing `chunk-bulk-action-bar.tsx` pattern from the chunks page.
**Files:** `apps/web/src/features/search/search-results.tsx`, potentially reuse `apps/web/src/features/chunks/chunk-bulk-action-bar.tsx`
**Effort:** Medium

---

## Phase 3: Graph Visualization in Search

Make graph queries visual.

### 3.1 Graph Minimap on Search Results
**Why:** When graph queries are active, users want to see the topology, not just a list.
**Scope:** Add a toggle to search results: "Show graph". When active, render a small React Flow visualization using `getSubgraph(resultChunkIds)` from AGE. Nodes are the result chunks, edges are their connections. Clicking a node navigates to the chunk.
**Files:** Create `apps/web/src/features/search/search-graph.tsx`, modify `apps/web/src/routes/search.tsx`
**Effort:** Medium
**Depends on:** AGE `getSubgraph` query (already exists)

### 3.2 Path Visualization
**Why:** `path:"A"->"B"` returns a list, but the path is inherently visual.
**Scope:** When a path query is active, render the path as a horizontal chain of nodes with labeled edges between them. Use a simplified React Flow layout (dagre horizontal).
**Files:** Create `apps/web/src/features/search/path-view.tsx`, modify search page
**Effort:** Medium

### 3.3 Duplicate Detection Hints
**Why:** Search results may include near-duplicate chunks that should be merged.
**Scope:** After fetching results, compute pairwise embedding similarity for the top N results. If any pair exceeds 0.9 similarity, show a "Possible duplicates" hint with a link to compare them.
**Files:** `packages/api/src/search/service.ts`, `apps/web/src/features/search/search-results.tsx`
**Effort:** Medium
**Depends on:** Embeddings populated (Ollama)

---

## Phase 4: Infrastructure & Deployment

### 4.1 AGE on Railway
**Why:** The deployed database doesn't have AGE. Graph queries fail in production.
**Scope:** Options:
  - **A)** Custom Dockerfile extending Railway's PG image with AGE compiled in
  - **B)** Switch to a PG provider that supports custom extensions (Supabase, Neon, self-hosted)
  - **C)** Feature-flag AGE queries — fall back to SQL/client-side when AGE is unavailable
**Recommendation:** C first (makes the app resilient), then A or B when ready.
**Files:** `packages/db/src/age/client.ts` (add availability check), service files (add fallback)
**Effort:** Medium (for feature flag), Large (for custom Docker)

### 4.2 AGE Fallback Mode
**Why:** Not all environments will have AGE installed. The app should degrade gracefully.
**Scope:** Add an `isAgeAvailable()` check (try `LOAD 'age'`, cache result). Query functions in `age/query.ts` fall back to the old SQL/CTE implementations when AGE is unavailable. Sync functions become no-ops.
**Files:** `packages/db/src/age/client.ts`, `packages/db/src/age/query.ts`, `packages/db/src/age/sync.ts`
**Effort:** Medium

### 4.3 Keyboard Shortcuts
**Why:** Power users want fast navigation.
**Scope:** `/` focuses search from any page. `Ctrl+K` opens a command palette (search + quick actions). `Escape` closes. Use a global keyboard listener in the root layout.
**Files:** Create `apps/web/src/features/nav/command-palette.tsx`, modify `apps/web/src/routes/__root.tsx`
**Effort:** Medium

---

## Priority Order

| # | Item | Phase | Effort | Impact |
|---|------|-------|--------|--------|
| 1 | AGE Connection Init | 1 | Small | High (perf) |
| 2 | Search Filter Popovers | 1 | Medium | High (UX) |
| 3 | AGE Fallback Mode | 4 | Medium | High (reliability) |
| 4 | Hop Distance Annotation | 1 | Medium | Medium |
| 5 | Health Score on Results | 1 | Small | Medium |
| 6 | Search from Nav | 2 | Medium | High (discoverability) |
| 7 | Onboarding for Search | 2 | Small | Medium |
| 8 | Semantic Search Clause | 2 | Medium | High |
| 9 | AGE Integration Tests | 1 | Medium | High (safety) |
| 10 | Graph Minimap | 3 | Medium | Medium |
| 11 | Bulk Operations | 2 | Medium | Medium |
| 12 | Query Builder on Chunks | 2 | Large | Medium |
| 13 | Path Visualization | 3 | Medium | Low |
| 14 | Duplicate Detection | 3 | Medium | Low |
| 15 | AGE on Railway | 4 | Large | High (deployment) |
| 16 | Keyboard Shortcuts | 4 | Medium | Medium |

---

## How to Execute

Each item is a standalone spec → plan → implementation cycle. Pick items from the top of the priority list. Items within the same phase can be parallelized if they don't share files.

Recommended batches:
- **Batch 1:** Items 1, 2, 3 (fix foundations + make resilient)
- **Batch 2:** Items 4, 5, 7, 9 (small wins)
- **Batch 3:** Items 6, 8 (search power)
- **Batch 4:** Items 10, 11 (graph viz + bulk ops)
- **Batch 5:** Items 12, 15, 16 (larger efforts)
