# Query Builder & Graph-Aware Search Design

## Problem

The current `/search` page is text-only with no filters. The chunk list page has filters but no advanced query logic (everything is AND, no graph awareness). Multi-hop traversals, path finding, and requirement reach queries exist in the AGE layer but aren't exposed to users. There's no way to compose complex queries or save them for reuse.

## Goal

Replace `/search` with a hybrid query builder that combines structured text input with visual filter blocks. Include graph-aware query types powered by Apache AGE (neighborhood, path finding, requirement reach, subgraph). Results render as a standard chunk list with graph context metadata.

---

## 1. Query Builder UI

Replaces the current `/search` page. Layout top to bottom:

### Text Input Bar

Monospace input at the top supporting structured syntax: `type:reference tag:api near:"Auth Flow" hops:2`. Autocomplete suggests field names, tag values, and chunk titles as the user types. Pressing Enter or clicking Search executes the query.

Syntax reference displayed below the input: `Supports: type: tag: connections: near: hops: path: affected-by: NOT`

### Filter Pill Row

Each active filter renders as a color-coded pill:
- **Indigo** — type, origin, review status (entity metadata)
- **Teal** — tag filters
- **Amber** — graph queries (near, path, affected-by)
- **Slate** — text search, connections, date

AND/OR toggle between pills (AND is default). Each pill has a × button to remove it. A "+ Add filter" button opens a categorized dropdown with all available filter types.

### Sync Behavior

Text input and pills stay in sync bidirectionally. Typing `tag:api` in the input creates a teal pill. Clicking to add a tag pill updates the text input. Both produce the same query object.

### Graph Query Indicator

When graph operations are active, a subtle bar below the pills describes what's happening: "Graph query active — showing chunks within 2 hops of 'Auth Flow'".

### Results

Standard chunk list. When graph queries are active, each result shows contextual metadata:
- **Neighborhood queries:** hop distance from reference chunk
- **Path queries:** position in the path
- **Requirement reach:** which requirement triggered the match

### Actions

- **Save query** — persist the current query for reuse
- **Clear all** — reset all filters

---

## 2. Query Model

A query is a list of filter clauses joined by AND (default) or OR.

### Clause Structure

```typescript
interface QueryClause {
    field: string;
    operator: string;
    value: string;
    params?: Record<string, string>;
    negate?: boolean;
}

interface SearchQuery {
    clauses: QueryClause[];
    join: "and" | "or";
    sort?: "relevance" | "newest" | "oldest" | "updated";
    limit?: number;
    offset?: number;
    codebaseId?: string;
}
```

### Standard Filter Clauses

| Field | Operators | Value | Text Syntax |
|-------|-----------|-------|-------------|
| `type` | is, is_not | chunk type | `type:reference` |
| `tag` | is, is_not, any_of | tag name(s) | `tag:api,auth` |
| `text` | contains | search string | `"auth flow"` |
| `connections` | gte, lte, eq | number | `connections:3+` |
| `updated` | within | days | `updated:30d` |
| `origin` | is | human, ai | `origin:ai` |
| `review` | is | draft, approved | `review:draft` |
| `codebase` | is | codebase name | `codebase:fubbik` |

Prefix any clause with `NOT` to negate: `NOT tag:deprecated`

### Graph Filter Clauses

| Field | Parameters | AGE Function | Text Syntax |
|-------|-----------|-------------|-------------|
| `near` | chunk title/id, hops (1-5) | `getNeighborhood` | `near:"Auth Flow" hops:2` |
| `path` | from chunk, to chunk | `findShortestPath` | `path:"Auth Flow"->"DB Schema"` |
| `affected-by` | requirement title/id, hops (1-5) | `getChunksAffectedByRequirement` | `affected-by:"CRUD operations"` |

### Execution Order

1. Graph clauses execute first — return chunk ID sets from AGE
2. Standard clauses filter within those IDs (or all chunks if no graph clause)
3. Results sorted by relevance (text match score) or recency

### URL Serialization

Queries serialize to URL search params for sharing/bookmarking: `/search?q=type:reference+tag:api+near:"Auth Flow"+hops:2`

---

## 3. API Endpoint

### `POST /api/search/query`

**Request body:** `SearchQuery` (see clause structure above)

**Response:**
```typescript
{
    chunks: Array<{
        id: string;
        title: string;
        type: string;
        summary: string | null;
        tags: string[];
        connectionCount: number;
        updatedAt: string;
        graphContext?: {
            hopDistance?: number;
            pathPosition?: number;
            matchedRequirement?: string;
        };
    }>;
    total: number;
    graphMeta?: {
        type: "neighborhood" | "path" | "requirement-reach";
        referenceChunk?: string;
        pathChunks?: string[];
    };
}
```

### `GET /api/search/autocomplete`

Query params: `field` (tag, chunk, requirement), `prefix` (search string)

Returns: `{ suggestions: Array<{ value: string; label: string }> }`

Queries tags, chunk titles, or requirement titles depending on the field.

### Service Layer

New module: `packages/api/src/search/service.ts`

Responsibilities:
- Parse query clauses
- Dispatch graph clauses to AGE query functions (`getNeighborhood`, `findShortestPath`, `getChunksAffectedByRequirement`)
- Build Drizzle query for standard filters (reuses patterns from existing `listChunks`)
- Intersect graph results with standard filter results (AND) or union them (OR)
- Compute `graphContext` metadata for each result (hop distance, path position)

---

## 4. Saved Queries

### Schema

New table `saved_query`:
- `id` (text, PK)
- `name` (text, not null) — user-given name like "API docs needing review"
- `query` (jsonb, not null) — serialized `SearchQuery` object
- `userId` (text, FK to user)
- `codebaseId` (text, FK to codebase, nullable)
- `createdAt` (timestamp, default now)

### API

- `GET /api/search/saved` — list user's saved queries
- `POST /api/search/saved` — save new query (body: `{ name, query, codebaseId? }`)
- `DELETE /api/search/saved/:id` — delete

### UI

"Save query" button in the search header. Saved queries appear as a dropdown next to the search bar. Click to load clauses into the builder. No inline editing — delete and re-save.

---

## 5. What Changes and What Doesn't

### Changes

- `apps/web/src/routes/search.tsx` — full rewrite to query builder
- Create `packages/api/src/search/service.ts` — query execution engine
- Create `packages/api/src/search/routes.ts` — POST /api/search/query, GET /api/search/autocomplete, saved query CRUD
- Create `packages/db/src/schema/saved-query.ts` — saved_query table
- Create `packages/db/src/repository/saved-query.ts` — saved query CRUD
- Create `apps/web/src/features/search/` — query builder components (filter pills, add-filter dropdown, autocomplete input, text parser)

### Doesn't Change

- `/chunks` page — keeps existing filter popover and list/kanban views
- `GET /api/chunks` — existing list endpoint unchanged
- `GET /api/chunks/search/semantic` — still used for related suggestions on chunk detail
- `GET /api/chunks/search/federated` — still works independently
- AGE query layer (`packages/db/src/age/query.ts`) — consumed, not modified
- Graph visualization (`/graph`) — unchanged

---

## Out of Scope

- Semantic search in the query builder (could be added later as a `similar-to:` clause)
- Mini-graph visualization of results (list only for now)
- Natural language query parsing
- Replacing the `/chunks` filter popover
- Query sharing between users (queries are per-user)
