# Graph Expansions Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add graph diff, embeddable graph snippets, and clustering suggestions to the knowledge graph.

**Architecture:** Graph diff compares two saved graphs' chunkIds sets. Graph embedding generates static SVG from React Flow. Clustering uses a simple connected-components algorithm on the graph data.

**Tech Stack:** React, @xyflow/react, Elysia, Effect

---

## Task 1: Graph Diff (#1)

Compare two saved graphs or snapshots.

**Files:**
- Create: `apps/web/src/features/graph/graph-diff.tsx`
- Modify: `apps/web/src/routes/graph_.$graphId.tsx` — add "Compare with..." button

- [ ] **Step 1:** Read `saved-graph-view.tsx` and the saved graph API. Understand the `chunkIds` and `positions` shapes.

- [ ] **Step 2:** Create `graph-diff.tsx` — a component that takes two `SavedGraph` objects and shows: added nodes (in graph B but not A), removed nodes (in A but not B), unchanged nodes. Use color coding: green for added, red for removed, gray for unchanged.

- [ ] **Step 3:** On the saved graph detail page, add a "Compare with..." dropdown that lists other saved graphs. When selected, fetch both and render `<GraphDiff>`.

- [ ] **Step 4:** Commit.

---

## Task 2: Graph Embed Snippet (#2)

Generate an HTML snippet for embedding a graph preview externally.

**Files:**
- Create: `packages/api/src/saved-graphs/embed.ts` — generate SVG from graph data
- Modify: `packages/api/src/saved-graphs/routes.ts` — add `GET /saved-graphs/:id/embed`
- Modify: `apps/web/src/features/graph/saved-graph-view.tsx` — add "Copy embed code" button

- [ ] **Step 1:** Create a server-side SVG generator that takes chunkIds + positions and renders circles with labels + lines for connections. No React needed — pure string SVG.

- [ ] **Step 2:** Add `GET /saved-graphs/:id/embed?format=svg|html` endpoint. SVG returns the raw image. HTML returns a full snippet with the SVG + a link to the live graph.

- [ ] **Step 3:** On the saved graph page, add a "Copy embed" button that copies `<iframe>` or `<img>` tag to clipboard.

- [ ] **Step 4:** Commit.

---

## Task 3: Graph Clustering Suggestions (#3)

Analyze the graph for natural clusters and suggest tags.

**Files:**
- Create: `packages/api/src/graph/clustering.ts` — clustering algorithm
- Modify: `packages/api/src/graph/routes.ts` — add `GET /graph/clusters`
- Create: `apps/web/src/features/graph/cluster-suggestions.tsx` — UI component

- [ ] **Step 1:** Implement a simple connected-components clustering. Take all chunks + connections, find groups of chunks that are more connected to each other than to outsiders. Use modularity-based community detection (or simpler: find connected components, then split large components by removing weakest edges).

Simple approach — just find connected components:
```ts
function findClusters(chunks: string[], connections: Array<{sourceId: string, targetId: string}>): string[][] {
    const parent = new Map<string, string>();
    const find = (x: string): string => { ... }; // union-find
    const union = (a: string, b: string) => { ... };
    // ... standard union-find to group connected chunks
}
```

- [ ] **Step 2:** Add `GET /graph/clusters?codebaseId=` that returns clusters with suggested tag names (derived from common words in cluster chunk titles).

- [ ] **Step 3:** Create UI component showing cluster suggestions with "Apply as tag group" button.

- [ ] **Step 4:** Commit.
