# Dynamic Knowledge Graph

> Make fubbik's knowledge model and graph visualization alive — usage-weighted edges, emergent concepts, code-derived nodes, timeline scrubbing, and impact propagation — all powered by deepening the existing Apache AGE integration.

## Architecture: Deepened AGE + PostgreSQL

PostgreSQL remains the system of record for CRUD, auth, and relational data. The existing AGE `knowledge` graph expands from its current chunk/requirement vertices to become the primary representation for topology, temporal state, usage signals, code structure, and traversal queries.

Mutations flow through Drizzle (PostgreSQL) first, then the AGE sync layer projects graph-relevant data into the `knowledge` graph. If AGE queries fail, CRUD operations still work; graph features degrade gracefully.

## Expanded Graph Model

### New Vertex Labels

| Label | Source | Properties |
|---|---|---|
| `code_file` | AST indexer | `path`, `language`, `lastIndexedAt` |
| `code_symbol` | AST indexer | `name`, `kind` (function/class/type/variable), `filePath`, `line`, `exported` |
| `concept` | Emergent from usage | `label`, `strength` (co-reference count), `firstSeenAt` |

### New Edge Types

| Edge | Between | Properties |
|---|---|---|
| `defines` | code_file → code_symbol | — |
| `imports` | code_file → code_file | `symbols[]` |
| `annotates` | chunk → code_file or code_symbol | `via` (file_ref / applies_to) |
| `co_referenced` | chunk → chunk | `count`, `lastSeenAt`, `contexts[]` |
| `embodies` | concept → chunk | `strength` |
| `impacted_by` | chunk → chunk | `degree` (0–1), `sourceChangeAt`, `relation` |

### Temporal Properties on Existing Edges

Every `connects` edge gains `createdAt` and `deletedAt` (null = active). Deletions become soft-deletes in the graph so the timeline scrubber can reconstruct past states.

### State History on Existing Vertices

`chunk` vertices gain a `stateHistory` property — an array of `{at, title, type}` entries appended on each mutation, enabling point-in-time reconstruction without full snapshots.

## Feature 1: Usage Tracking & Emergent Concepts

### Usage Event Capture

A lightweight middleware records when chunks are accessed together. Three event sources:

| Source | What's recorded | Where |
|---|---|---|
| Context API (`/api/context/*`) | Which chunk IDs were returned together for a query | API route middleware |
| Chunk detail view (`/api/chunks/:id`) | Which chunk was viewed, by whom | API route |
| MCP tools (context, plan) | Which chunks surfaced to an AI agent | MCP tool wrapper |

### Storage

Events write to a PostgreSQL `usage_event` table (append-only):

- `id` text PK
- `kind` text — context_query / chunk_view / mcp_resolve
- `chunkIds` JSONB — array of chunk IDs in this event
- `query` text nullable — the search/path query that triggered it
- `userId` text FK
- `createdAt` timestamptz

### Co-reference Aggregation

A periodic job runs on the same interval as staleness scanning (configurable, default 24h). It reads usage events since the last aggregation run and upserts `co_referenced` edges in the AGE graph. Two chunks that repeatedly appear in the same context response get a stronger edge. The `count` property increments, `lastSeenAt` updates. A manual trigger is also available via `POST /api/usage/aggregate`.

### Emergent Concepts

When a cluster of 3+ chunks are co-referenced above a threshold (co-appear in 5+ distinct queries), a `concept` vertex is created with `embodies` edges to each member chunk. The concept's `label` is derived using a priority cascade: (1) if all member chunks share a common tag, use that tag name; (2) otherwise, use the most frequent `query` string from the usage events that produced the co-references; (3) failing both, use the most common noun phrase across member chunk titles (extracted via simple tokenization, no NLP dependency).

Concepts are soft — they strengthen with continued co-reference and fade (get pruned) if usage drops to zero over 90 days.

### Graph Visualization

The `co_referenced.count` feeds directly into edge thickness in the graph view. The existing `weight` field on `chunkConnection` in Drizzle stays for manual weights; AGE edge properties handle the behavioral weights. The graph service merges both when assembling visualization data.

## Feature 2: Code-Derived Nodes

### Indexing Pipeline

A `code-index` service scans the codebase for a given space and extracts symbols using tree-sitter (language-agnostic AST parsing via WASM bindings).

**Extracted per file:**

- Exported functions, classes, interfaces, types, constants
- Import relationships (which files import from which)
- Symbol metadata: name, kind, line number, exported flag

### Indexing Triggers

1. **Manual:** CLI command `fubbik index` or API endpoint `POST /api/code-index/scan`
2. **On staleness scan:** When the staleness scanner detects file changes via git diff, it re-indexes changed files
3. **On chunk file-ref update:** When a chunk's `fileReferences` change, re-index those files

### Graph Population

The indexer writes to AGE:

- `MERGE` a `code_file` vertex per file
- `MERGE` a `code_symbol` vertex per exported symbol
- `CREATE` `defines` edges (file → symbol)
- `CREATE` `imports` edges (file → file, with `symbols[]` property)
- `MERGE` `annotates` edges by matching chunk `file_refs` and `applies_to` globs against file paths

### Annotation as Bridge

Chunks annotate code nodes rather than replacing them. A `code_symbol` vertex like `enrichChunk()` shows which chunks document it, and a chunk shows which symbols it's about. The `annotates` edge is the link between human knowledge and code structure.

### Staleness from Code Changes

When a file is re-indexed and its symbol set has changed (function renamed, removed, signature changed), all chunks with `annotates` edges to affected symbols get an impact flag — feeding into impact propagation (Feature 4).

### Graph Density Management

Symbol-level nodes make the graph denser. Two controls:

- **Default visibility:** Code nodes are hidden by default in the graph view. A toggle reveals them, filtered by the current space's file paths.
- **Collapse mode:** Files with 5+ symbols collapse into a single `code_file` node showing a count badge. Expand on click.

## Feature 3: Timeline Scrubber

### State Transitions, Not Snapshots

Rather than storing periodic snapshots of the full graph, we store transitions — events that changed the graph's topology or properties.

### Graph Event Log

PostgreSQL table `graph_event`:

| Column | Type | Purpose |
|---|---|---|
| `id` | text | PK |
| `vertexLabel` | text | chunk, code_file, code_symbol, concept |
| `vertexId` | text | ID of the affected node |
| `edgeType` | text | nullable — populated for edge events |
| `edgeTargetId` | text | nullable — the other end of the edge |
| `action` | text | created, updated, deleted, property_changed |
| `snapshot` | JSONB | relevant properties at this moment |
| `createdAt` | timestamptz | when it happened |

Every AGE sync operation also appends to this log. The sync layer is already a chokepoint — adding a log write is one line per operation.

### Reconstruction

To render the graph at time T:

1. Fetch all events up to T
2. Replay forward to build a vertex/edge set
3. Return the same shape as `getUserGraph()` filtered to that moment

For performance, the service caches reconstructed graphs at daily granularity. Scrubbing within a day replays only that day's delta against the cached daily snapshot.

### UI

The timeline scrubber sits below the graph canvas:

- Horizontal slider spanning the space's history (first event → now)
- Tick marks at significant moments (event clusters = visible ticks)
- Dragging reconstructs the graph at that point
- Play button animates forward, nodes/edges appearing and disappearing
- Current time = rightmost position (default, same as today's behavior)

**What changes during scrub:**

- Nodes that don't exist yet at time T are absent
- Edges later deleted are still present (they hadn't been deleted yet)
- Concept nodes appear when their co-reference threshold was first crossed
- Code nodes appear when first indexed

## Feature 4: Impact Propagation

### Trigger

When a chunk is updated (content or title change), the existing `chunk.updated` event fires. A new handler computes the impact ripple.

### Ripple Calculation

Uses AGE's variable-length path queries:

```cypher
MATCH (source:chunk {id: $changedId})-[r*1..3]->(downstream:chunk)
RETURN downstream.id, length(r) AS hops, [rel IN r | type(rel)] AS path
```

### Degree Scoring

Each downstream chunk gets an impact degree (0–1):

- **Distance decay:** 1 hop = 0.9, 2 hops = 0.5, 3 hops = 0.2
- **Relation weight:** `depends_on` = 1.0x, `extends` = 0.8x, `part_of` = 0.7x, `references` = 0.3x, `related_to` = 0.2x
- **Final degree** = distance_factor × relation_weight, capped at strongest path if multiple exist

### Flag Creation

For each downstream chunk with degree > 0.1, a staleness flag in the existing `chunk_staleness` table:

- `reason`: `upstream_impact` (new reason type)
- `detail`: "Impacted by change to '{sourceTitle}' (degree: 0.7, via depends_on → extends)"
- Flags with degree < 0.3 auto-dismiss after 14 days if no action taken

### Code Node Propagation

When the code indexer detects a symbol change, it traces `annotates` edges backward to find chunks documenting that symbol, then propagates outward from those chunks using the same ripple logic.

### No Double-Flagging

If a chunk already has an active impact flag from the same source chunk, the degree is updated (max of old and new) rather than creating a duplicate.

### Surfacing

Impact flags appear in the same places staleness flags already appear:

- Dashboard "Attention Needed" widget (sorted by degree)
- Amber banner on chunk detail page: "Impacted by change to X (2 hops via depends_on)"
- Nav badge count includes impact flags
- Graph view: impacted nodes get an amber ring, intensity proportional to degree

## Feature Composition

The five features form a feedback loop:

1. **Usage tracking** feeds **emergent concepts** — co-reference patterns create concept nodes
2. **Code indexing** feeds **impact propagation** — symbol changes ripple through annotated chunks
3. **All mutations** feed the **timeline scrubber** — the event log enables time-travel across every node and edge type
4. **Usage-weighted edges** and **impact ripples** are both visible in the graph, controlled by the timeline position
5. **Concepts** create new graph structure that code indexing can detect as annotation targets, closing the loop

## New API Endpoints

- `POST /api/code-index/scan` — trigger code indexing for a space
- `GET /api/code-index/status` — indexing status and last indexed timestamp
- `GET /api/graph/at?t=<iso-timestamp>` — graph state at a point in time
- `GET /api/graph/events?from=&to=` — raw event log for a time range
- `GET /api/concepts` — list emergent concepts with strength and member chunks
- `GET /api/usage/co-references?chunkId=` — co-reference edges for a chunk

## New CLI Commands

- `fubbik index [path]` — index code symbols for the current space
- `fubbik concepts` — list emergent concepts

## Database Changes

### New PostgreSQL Tables

- `usage_event` — append-only usage tracking
- `graph_event` — append-only graph state transition log

### New AGE Vertex Labels

- `code_file`, `code_symbol`, `concept`

### New AGE Edge Types

- `defines`, `imports`, `annotates`, `co_referenced`, `embodies`, `impacted_by`

### Modified

- `chunk_staleness.reason` — add `upstream_impact` value
- AGE `connects` edges — add `createdAt`, `deletedAt` properties
- AGE `chunk` vertices — add `stateHistory` property

## Dependencies

- `tree-sitter` + language WASM bindings (TypeScript at minimum, extensible)
- No new infrastructure — AGE and PostgreSQL handle everything
