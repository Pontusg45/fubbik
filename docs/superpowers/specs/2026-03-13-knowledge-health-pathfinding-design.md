# Knowledge Health + Path Finding UX — Design Spec

## Overview

Two features:

1. **Knowledge Health page** — surfaces chunks that need attention: orphans (no connections), stale (outdated relative to neighbors), and thin (minimal content).
2. **Path Finding UX** — makes the existing shortest-path feature discoverable via a graph panel and accessible from chunk detail pages.

No changes to search — pg_trgm with GIN indexes and similarity ranking already provides good full-text search.

## Knowledge Health API

### New endpoint: `GET /api/health/knowledge`

**Query params:**
- `codebaseId` (optional) — scope to a codebase + global chunks, consistent with other endpoints

**Response shape:**

```typescript
{
  orphans: {
    chunks: Array<{ id: string; title: string; type: string; createdAt: string }>;
    count: number;
  };
  stale: {
    chunks: Array<{ id: string; title: string; type: string; updatedAt: string; newestNeighborUpdate: string }>;
    count: number;
  };
  thin: {
    chunks: Array<{ id: string; title: string; type: string; contentLength: number }>;
    count: number;
  };
}
```

Each category is capped at **50 results** (`LIMIT 50`). The `count` field returns the total count regardless of the limit, so the UI can show "12 orphans" or "127 orphans (showing 50)".

### Definitions

- **Orphans:** Chunks with zero rows in `chunk_connection` as either source or target. Use two `NOT IN` subqueries for index efficiency:
  ```sql
  WHERE chunk.id NOT IN (SELECT source_id FROM chunk_connection)
    AND chunk.id NOT IN (SELECT target_id FROM chunk_connection)
  ```
  This uses the existing `connection_sourceId_idx` and `connection_targetId_idx` indexes.

- **Stale:** Chunks where `updatedAt < NOW() - 30 days` AND at least one connected chunk (via `chunk_connection` as source or target) has `updatedAt > NOW() - 7 days`. The neighbor was refreshed recently but this chunk wasn't. The `newestNeighborUpdate` field is the max `updatedAt` across all connected chunks, computed in SQL via a correlated subquery.

- **Thin:** Chunks where `LENGTH(content) < 100`. Content length is computed in SQL (`LENGTH(content) AS content_length`) — the raw `content` field is **not** selected in this query to avoid transferring large text payloads.

All queries are user-scoped and optionally codebase-scoped.

### Codebase scoping

All three queries must follow the same codebase-scoping pattern used in `getAllChunksMeta` in `packages/db/src/repository/graph.ts`: when `codebaseId` is set, include chunks that are in the requested codebase OR chunks that are not in any codebase (global). This uses the `chunk_codebase` join table with the same `IN / NOT IN` subquery pattern from the multi-codebase feature.

### Backend pattern

- New repository functions in `packages/db/src/repository/knowledge-health.ts`: `getOrphanChunks(userId, codebaseId?)`, `getStaleChunks(userId, codebaseId?)`, `getThinChunks(userId, codebaseId?)`.
- New service in `packages/api/src/knowledge-health/service.ts`: composes the three queries with `Effect.all({ concurrency: "unbounded" })`.
- New route in `packages/api/src/knowledge-health/routes.ts`: `GET /health/knowledge`.
- Registered in `packages/api/src/index.ts` via `.use(knowledgeHealthRoutes)`.

**Route registration:** The existing `healthRoutes` (system health) is registered before the `.onError` handler at the top of the Elysia chain. The new `knowledgeHealthRoutes` is registered after `.resolve()` alongside the other feature routes, since it requires authentication. The paths do not conflict: existing is `GET /health`, new is `GET /health/knowledge`.

## Knowledge Health Page

### Route: `/knowledge-health`

Using `/knowledge-health` instead of `/health` to avoid confusion with system health semantics.

Three card sections, each with a count badge and expandable chunk list:

**Orphan Chunks (N)**
- Description: "These chunks have no connections. Link them to other chunks or delete them if no longer needed."
- Each row: chunk title (linked to `/chunks/:id`), type badge, created date
- Quick actions: "Link" (navigates to chunk detail where linking is available), "Delete"

**Stale Chunks (N)**
- Description: "These chunks haven't been updated recently but are connected to chunks that have. They may need a refresh."
- Each row: chunk title (linked), type badge, last updated date, "neighbor updated X days ago"
- Quick action: "Edit" (navigates to `/chunks/:id/edit`)

**Thin Chunks (N)**
- Description: "These chunks have very little content. Consider expanding them or merging with related chunks."
- Each row: chunk title (linked), type badge, content length indicator
- Quick action: "Edit"

**Codebase-aware:** Reads active codebase from `useActiveCodebase()` hook and passes `codebaseId` to the API.

**Nav:** Add "Health" link to the navigation bar (desktop + mobile).

## Path Finding UX

### Entry point 1: Graph view panel

Add a visible "Find Path" button to the graph toolbar (alongside existing filter/layout controls).

**When clicked, opens a panel with:**
- Two searchable chunk selector dropdowns (populated from the chunks already loaded in the graph)
- "Find Path" button
- Result display: a **relation chain** strip showing the path with direction indicators
- "Clear" button to dismiss

**Relation chain direction:** The existing `findShortestPath` BFS treats edges as undirected (adds both directions to the adjacency list). This is correct — you want to find connections regardless of edge direction. However, the chain display must indicate the actual edge direction. For each consecutive pair `(A, B)` in the path, look up the edge:
- If an edge exists with `source=A, target=B`: display as `A —relation→ B`
- If an edge exists with `source=B, target=A`: display as `A ←relation— B`
- If multiple edges exist between the same pair (different relations), show all relation labels separated by `/`

**Graph behavior when path is found:**
- Highlight path nodes and edges (increased opacity/color)
- Dim all non-path nodes and edges
- The existing `findShortestPath` BFS function is reused — it already works on the loaded edge data

**When no path exists:** Show "No path found between these chunks" message.

**Alt+Click shortcut:** The current Alt+Click path-finding behavior remains as a shortcut. Both the panel and Alt+Click write to the same shared state (`pathStartId`, `pathEndId`). Last write wins — clicking a new start via either method always clears the previous end and result. The panel reflects the current state regardless of how it was set.

### Entry point 2: Chunk detail page

On `/chunks/:id`, add a "Find path to..." action in the actions area.

**When clicked:**
- Opens a searchable chunk selector dropdown ("Select target chunk")
- On selection, navigates to `/graph?pathFrom=<currentId>&pathTo=<selectedId>`

**Graph page behavior with pathFrom/pathTo params:**
- Auto-triggers path finding on load
- Populates the path panel with the pre-selected chunks
- Highlights the result immediately
- If either chunk is not present in the currently loaded graph (e.g., different codebase scope), show an error: "One or both chunks are not in the current graph view. Try switching to 'All' codebases." The graph does not auto-switch codebase scope.

This means the graph view owns all path visualization — the chunk detail page just links into it with query params.

### Implementation notes

- The graph view already has `findShortestPath` (BFS on adjacency list from edges). No new algorithm needed.
- Path panel state: `pathStartId`, `pathEndId`, `pathResult` (array of node IDs or null). Shared between panel and Alt+Click.
- The relation chain display reads edge data to find the relation label between each consecutive pair of nodes in the path, respecting edge direction.
- Graph route needs new optional search params: `pathFrom`, `pathTo`.
