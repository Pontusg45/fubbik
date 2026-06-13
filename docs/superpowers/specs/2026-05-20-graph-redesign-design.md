# Graph Redesign: Cosmic Archipelago

**Date:** 2026-05-20 **Status:** Draft **Problem:** The current graph becomes overwhelming at 500+ chunks. It renders all nodes at once and
relies on clustering as a band-aid. The primary use case — "where does this chunk fit?" — is poorly served by a dump-everything-then-filter
model. **Solution:** Replace the current graph with a three-level semantic zoom model that combines island-based macro views (Archipelago)
with progressive neighborhood disclosure (Cosmic Map).

---

## Goals

1. **Scale** — handle 500-2000 chunks without performance degradation or visual overload.
2. **Discoverability** — make it easy to orient ("where does this chunk fit?") by showing neighborhood context by default.
3. **Readable edges** — encode relation types visually so the graph structure is legible without hovering every edge.
4. **Health visibility** — surface chunk health at every zoom level so stale/thin knowledge is always apparent.

## Non-Goals

- Collaborative real-time editing of the graph.
- 3D visualization or WebGL rendering.
- Replacing the chunk list page — the graph is a complementary navigation tool.
- Graph-based editing (creating/deleting chunks directly in the graph).

---

## Core Model: Three Semantic Zoom Levels

The graph has three zoom levels that the user moves between fluidly via scroll/pinch or click interactions. These are not discrete modes —
the transition is animated and continuous.

### Level 1 — Overview (Archipelago)

The entire knowledge base rendered as **islands**. Each island represents a knowledge domain (derived from tag types). Islands show:

- Icon + domain name
- Chunk count
- Health dot cluster (one colored dot per chunk: green/yellow/red)
- Size proportional to chunk count

**Bridges** connect islands with cross-domain relationships. Each bridge shows the connection count and is styled by the dominant relation
type. Islands are positioned via force layout on 5-20 nodes — instant computation, no web worker needed.

### Level 2 — Neighborhood (Cosmic Map)

Activated by clicking an island or a chunk. When entering from an island click, the most-connected chunk within that island becomes the
initial focus. The focus chunk sits at the center of the viewport. Around it:

- **Direct connections (1-hop):** Rendered as cards with title, one-line summary, health dot, and tags. Edge color and line style encode the
  relation type.
- **2-hop neighbors:** Rendered as small colored dots with labels on hover. Positioned further from center.
- **Other islands:** Visible as collapsed periphery at the edges of the viewport, maintaining global context.

The focus chunk's card is larger with a highlighted border and glow. Clicking any neighbor chunk re-centers the neighborhood around it
(animated transition).

### Level 3 — Detail

Activated by clicking a chunk that is already in the neighborhood view (or double-clicking any chunk). The neighborhood dims and a detail
panel slides in from the right showing:

- Full chunk content preview
- Health bar with score breakdown (freshness, completeness, richness, connectivity)
- Tags
- Connection list grouped by relation type
- Action buttons: Open (navigate to chunk page), Edit, Center (re-focus graph)

---

## Visual Language

### Node Styles by Zoom Level

| Level        | Element        | Style                                                                                                |
| ------------ | -------------- | ---------------------------------------------------------------------------------------------------- |
| Overview     | Island         | Rounded container with translucent fill, colored border, icon, name, chunk count, health dot cluster |
| Overview     | Bridge         | Curved dashed line between islands with connection count label                                       |
| Neighborhood | Focus chunk    | Card with 2px colored border, glow shadow, title, summary, tags, health dot                          |
| Neighborhood | Neighbor chunk | Smaller card with 1px border, title, one-line summary, health dot                                    |
| Neighborhood | 2-hop chunk    | 7px colored dot with label on hover                                                                  |
| Detail       | Panel          | Right-side panel (260px) with full chunk info, health bar, connections, actions                      |

### Edge Encoding

Each relation type has a distinct visual treatment so the graph is readable at a glance:

| Relation         | Color            | Line Style        | Marker           | Description             |
| ---------------- | ---------------- | ----------------- | ---------------- | ----------------------- |
| `depends_on`     | Blue (#3b82f6)   | Solid, 2px        | Filled arrowhead | "A needs B"             |
| `part_of`        | Green (#22c55e)  | Solid, 2.5px      | Containment dot  | "A belongs inside B"    |
| `extends`        | Purple (#a78bfa) | Solid, 1.5px      | Open arrowhead   | "A builds on B"         |
| `references`     | Gray (#94a3b8)   | Dashed (6,4)      | None             | Weak/informational link |
| `related_to`     | Gray (#94a3b8)   | Dashed (6,4)      | None             | Weak/informational link |
| `contradicts`    | Red (#ef4444)    | Dashed (4,4), 2px | Slash mark       | "A conflicts with B"    |
| `alternative_to` | Amber (#f59e0b)  | Solid, 1.5px      | Fork             | "A or B, pick one"      |
| `supports`       | Cyan (#06b6d4)   | Long-dash (8,3)   | None             | "A reinforces B"        |

### Health Encoding

Health is always visible, never hidden behind a toggle. The representation adapts to zoom level:

| Score  | Color                 | Glow              |
| ------ | --------------------- | ----------------- |
| 80-100 | Green (#22c55e)       | Subtle green glow |
| 60-79  | Light green (#4ade80) | None              |
| 40-59  | Amber (#f59e0b)       | Subtle amber glow |
| 0-39   | Red (#ef4444)         | Subtle red glow   |

**Overview level:** Each island contains a row of small dots, one per chunk, colored by health score. This gives an instant read on domain
health distribution.

**Neighborhood level:** Each chunk card has a health dot (colored circle) next to the title.

**Detail level:** Full health bar with numeric score and breakdown into the four sub-scores.

**Heatmap toggle:** Optional mode that switches all node border colors from type-based to health-based coloring, making the entire graph a
health heatmap.

### Aesthetic

- Dark cosmic theme: deep navy background (#0f172a), subtle radial gradients
- Translucent island containers with backdrop blur
- Glowing accents on focus elements
- Smooth animations on zoom transitions, re-centering, island expansion
- The knowledge base feels like a living constellation

---

## Island Formation (Grouping)

Islands are derived from **tag types** — the existing grouping taxonomy. The user selects which tag type drives island formation (default:
the tag type with the best chunk coverage).

### Rules

1. Each chunk belongs to one primary island based on its tag under the selected tag type.
2. **Multi-tagged chunks** (multiple tags under the grouping tag type): placed in the first matching island. A ghost dot indicator appears
   in other relevant islands.
3. **Untagged chunks**: collected into an "Ungrouped" island with muted styling. Serves as a triage signal.
4. **Single-chunk islands**: rendered as standalone nodes without an island border until they accumulate 2+ chunks.
5. Users can switch the grouping tag type at any time. Island formation recalculates instantly (client-side).

### Island Layout

Islands are positioned using a lightweight force simulation:

- Each island is a single node in the simulation (5-20 nodes total)
- Repulsion prevents overlap
- Bridge connections act as springs (shorter distance = more connections)
- Computation is instant — no web worker needed

---

## Interactions

### Navigation

| Action                           | Result                                                                    |
| -------------------------------- | ------------------------------------------------------------------------- |
| Scroll/pinch                     | Zoom between levels fluidly                                               |
| Click island (overview)          | Zoom into island, center on its most-connected chunk, expand neighborhood |
| Click chunk (neighborhood)       | Re-center neighborhood around clicked chunk                               |
| Click chunk again / double-click | Open detail panel                                                         |
| Breadcrumb                       | Navigate back: Detail → Neighborhood → Overview                           |
| Esc                              | Go up one level                                                           |

### Search

Same as current: case-insensitive substring match on chunk titles, debounced 150ms. At overview level, matching islands glow and show match
count. At neighborhood level, matching chunks are highlighted with non-matches dimmed. Navigating to a search result zooms to its
neighborhood.

### Path Finding

Kept from current graph. Alt+click sets start/end. Path is highlighted across zoom levels — if start and end are in different islands, both
islands expand and the path bridges are highlighted.

### Deep Links

URL encodes: zoom level, focus chunk ID, active grouping tag type, expanded islands. Shareable and bookmarkable.

---

## Entry Points

| Source                            | Opens At      | Behavior                                           |
| --------------------------------- | ------------- | -------------------------------------------------- |
| `/graph`                          | Overview      | All islands visible, no focus                      |
| Chunk detail page "View in graph" | Neighborhood  | Centered on that chunk, its island expanded        |
| Search result                     | Neighborhood  | Centered on matched chunk, search highlight active |
| Deep link                         | Encoded state | Restores exact zoom level, focus, expanded islands |

---

## Performance

### Why This Is Faster

The current graph renders 500-2000 React Flow nodes simultaneously. The redesign renders **~5-30 elements** at any zoom level:

- **Overview:** 5-20 island nodes + bridge edges
- **Neighborhood:** 1 focus card + 4-10 neighbor cards + 5-15 dots
- **Detail:** Same as neighborhood + one panel

### Strategy

| Concern     | Approach                                                                                                                            |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Layout      | Force simulation on islands only (5-20 nodes). Neighborhood uses radial placement (no simulation).                                  |
| Rendering   | Only visible elements + one screen buffer. React components for cards, lightweight SVG/canvas for dots.                             |
| Data        | One API call loads all chunk metadata + connections (same `/api/graph` endpoint). Island membership computed client-side from tags. |
| Transitions | CSS/FLIP animations between zoom levels. No re-layout on zoom.                                                                      |
| Caching     | Island positions cached. Neighborhood layout cached per focus chunk.                                                                |

### No New Backend Work

The existing `/api/graph` endpoint returns chunks, connections, chunkTags, tagTypes, and chunkCodebases — everything needed. Island
formation is pure client-side grouping.

---

## What Changes from Current Graph

### Kept

- React Flow as the rendering foundation
- Tag-based grouping logic (repurposed for island formation)
- Search functionality (enhanced with zoom-aware highlighting)
- Detail panel (restyled)
- Saved views / filter presets
- Path finding
- Graph API endpoint
- Keyboard shortcuts (Esc, search)

### Reworked

- **Force layout** → only used for island-level positioning
- **Node components** → three zoom-level variants (island, card, dot)
- **Edge rendering** → typed visual encoding (color + line style + marker)
- **Focus mode** → replaced by neighborhood-first as the default model
- **Cluster strategy** → replaced by island formation from tags
- **Styling hooks** → zoom-level aware (useGraphStyling rewritten)
- **Graph state** → simplified (no layout algorithm toggle, no explore mode toggle)

### Removed

- **Hierarchical layout** — replaced by the zoom model
- **Radial layout** (as a top-level option) — radial placement used internally for neighborhood, but not user-selectable
- **Explore mode** — the entire graph is now neighborhood-first; explore mode was the prototype of this idea
- **Cluster aggregation** — replaced by islands, which are semantically meaningful (not arbitrary threshold)
- **Web worker for layout** — unnecessary when simulating 5-20 nodes
- **Edge bundling toggle** — edges are sparse enough per zoom level that bundling is unnecessary
- **Timeline cutoff filter** — low usage; health heatmap serves the "what's stale" use case better
- **Compound two-level grouping** — islands provide one clear grouping level; secondary grouping adds complexity without proportional value
- **useMainThread toggle** — no longer needed without heavy layout computation

---

## File Impact

### New Files

- `apps/web/src/features/graph/island-layout.ts` — island formation + force layout
- `apps/web/src/features/graph/neighborhood-layout.ts` — radial placement around focus chunk
- `apps/web/src/features/graph/graph-island-node.tsx` — island node component
- `apps/web/src/features/graph/graph-chunk-card.tsx` — neighborhood chunk card component
- `apps/web/src/features/graph/graph-chunk-dot.tsx` — 2-hop dot component
- `apps/web/src/features/graph/typed-edge.tsx` — relation-type-aware edge component
- `apps/web/src/features/graph/use-graph-zoom.ts` — zoom level state + transitions
- `apps/web/src/features/graph/use-graph-islands.ts` — island formation from tags

### Modified Files

- `apps/web/src/features/graph/graph-view.tsx` — major rewrite as orchestrator for zoom levels
- `apps/web/src/features/graph/use-graph-data.ts` — add island computation to data pipeline
- `apps/web/src/features/graph/use-graph-nodes.ts` — rewrite for zoom-aware node/edge building
- `apps/web/src/features/graph/use-graph-styling.ts` — rewrite for zoom-level-aware styling
- `apps/web/src/features/graph/use-graph-state.ts` — simplify state (remove layout algorithm, explore mode, cluster state)
- `apps/web/src/features/graph/use-graph-interactions.ts` — add zoom navigation, simplify

### Deleted Files

- `apps/web/src/features/graph/force-layout.ts` — replaced by island-layout.ts
- `apps/web/src/features/graph/quadtree.ts` — no longer needed for 5-20 node simulation
- `apps/web/src/features/graph/layout.worker.ts` — web worker no longer needed
- `apps/web/src/features/graph/cluster-strategy.ts` — replaced by island formation
- `apps/web/src/features/graph/graph-node.tsx` — replaced by zoom-level-specific components
- `apps/web/src/features/graph/graph-group-node.tsx` — replaced by island node
- `apps/web/src/features/graph/graph-cluster-node.tsx` — replaced by island node
- `apps/web/src/features/graph/floating-edge.tsx` — replaced by typed-edge.tsx
