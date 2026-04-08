# Apache AGE Integration Design

## Problem

Fubbik is a knowledge graph, but its graph topology lives in relational tables. Chunk connections, requirement dependencies, and requirement-chunk links are modeled as join tables queried with SQL. Multi-hop traversals happen client-side via JavaScript BFS. Recursive CTEs exist only for requirement dependencies. There's no DB-side path finding, reachability, or pattern matching for the core chunk graph.

This is an architectural mismatch: the data is a graph, but the query model isn't.

## Goal

Add Apache AGE as a graph query layer alongside PostgreSQL. AGE owns the topology (edges and traversals); Drizzle owns entity data (chunk content, tags, embeddings, auth). Relational tables remain source of truth with write-through sync to AGE.

---

## 1. AGE Graph Schema

One graph called `knowledge` with:

### Vertices

- **`chunk`** — properties: `{id}` (matches relational `chunk.id`). No other properties; all chunk data lives in the relational table. The vertex is a topology anchor.
- **`requirement`** — properties: `{id}` (matches relational `requirement.id`). Same principle.

### Edges

- **`connects`** — chunk → chunk. Properties: `{relation, id}`. `relation` is the connection type (related_to, depends_on, extends, etc.). `id` matches `chunkConnection.id`.
- **`depends_on`** — requirement → requirement. Properties: `{id}` for tracking, value derived from the composite key.
- **`covers`** — requirement → chunk. Properties: `{id}` for tracking.

### Why Both Entity Types as Vertices

Requirements link to chunks (`covers`) and to each other (`depends_on`). Making both types vertices enables cross-entity traversals: "find all chunks within 2 hops of a failing requirement" or "trace the dependency chain from a requirement through chunks to related requirements."

---

## 2. Write-Through Sync Layer

**Module:** `packages/db/src/age/sync.ts`

A thin sync module called from existing repository functions within the same database transaction.

### Functions

- `ensureVertex(type, id)` — idempotent vertex creation via `MERGE`
- `createEdge(label, fromType, fromId, toType, toId, props)` — creates a Cypher edge
- `deleteEdge(label, props)` — removes edge matching properties
- `deleteVertex(type, id)` — removes vertex and all connected edges

### Integration Points

Existing repository functions that get a sync call added:

| Repository Function | Sync Action |
|---|---|
| `createConnection()` | `ensureVertex` for both chunks + `createEdge('connects', ...)` |
| `deleteConnection()` | `deleteEdge('connects', {id})` |
| `addDependency()` | `ensureVertex` for both requirements + `createEdge('depends_on', ...)` |
| `removeDependency()` | `deleteEdge('depends_on', ...)` |
| `setRequirementChunks()` | Delete old `covers` edges for requirement, insert new ones |
| `createChunk()` | `ensureVertex('chunk', id)` |
| `deleteChunk()` | `deleteVertex('chunk', id)` (cascades edges) |
| `createRequirement()` | `ensureVertex('requirement', id)` |
| `deleteRequirement()` | `deleteVertex('requirement', id)` (cascades edges) |

### SQL Execution

AGE queries execute via `db.execute(sql\`SELECT * FROM cypher('knowledge', $$ ... $$) AS (v agtype)\`)` — raw SQL through Drizzle's `execute`. AGE requires per-connection setup: `LOAD 'age'; SET search_path = ag_catalog, "$user", public;`.

### Transaction Safety

Sync calls run in the same `db.transaction()` as the relational write. If the AGE write fails, the whole transaction rolls back.

---

## 3. Cypher Query Layer

**Module:** `packages/db/src/age/query.ts`

Replaces client-side graph operations and expensive SQL queries with Cypher.

### Queries

**`findShortestPath(chunkIdA, chunkIdB)`** — replaces client-side BFS in `graph-utils.ts`
```cypher
MATCH p = shortestPath((a:chunk {id: $A})-[*]-(b:chunk {id: $B}))
RETURN [n IN nodes(p) | n.id] AS path
```

**`getNeighborhood(chunkId, maxHops)`** — replaces client-side `getNodesWithinHops`
```cypher
MATCH (c:chunk {id: $id})-[*1..N]-(neighbor:chunk)
RETURN DISTINCT neighbor.id AS id
```

**`getTransitiveDeps(requirementId)`** — replaces recursive CTE in `requirement-dependency.ts`
```cypher
MATCH (r:requirement {id: $id})-[:depends_on*]->(dep:requirement)
RETURN dep.id AS id
```

**`getChunksAffectedByRequirement(requirementId, hops)`** — new capability
```cypher
MATCH (r:requirement {id: $id})-[:covers]->(c:chunk)-[:connects*0..N]-(related:chunk)
RETURN DISTINCT related.id AS id
```

**`getSubgraph(chunkIds)`** — efficient subgraph extraction for graph UI
```cypher
MATCH (a:chunk)-[e:connects]->(b:chunk)
WHERE a.id IN $ids AND b.id IN $ids
RETURN a.id, e.relation, b.id
```

**`getOrphanChunks()`** — replaces expensive NOT IN subqueries in `knowledge-health.ts`
```cypher
MATCH (c:chunk) WHERE NOT (c)-[]-() RETURN c.id
```

### Consumers That Change

| Current Location | Current Approach | New Approach |
|---|---|---|
| `graph-utils.ts` (frontend) | Client-side BFS for path finding, N-hop | API calls to Cypher-backed endpoints |
| `requirement-dependency.ts` | Recursive CTEs | Cypher variable-length paths |
| `knowledge-health.ts` | NOT IN subqueries for orphans | Cypher pattern match |
| `/api/graph` endpoint | Fetch all, filter client-side | Optional `getSubgraph` for filtered views |

### Return Types

All query functions return `Effect<T, DatabaseError>` matching the existing pattern. They return plain ID arrays or simple objects — calling code joins with relational data as needed.

---

## 4. Migration and Setup

### PostgreSQL Extension

Apache AGE must be installed as a PostgreSQL extension. This is a deployment requirement.

### Setup Script: `packages/db/src/age/setup.ts`

1. `CREATE EXTENSION IF NOT EXISTS age`
2. `LOAD 'age'`
3. `SET search_path = ag_catalog, "$user", public`
4. `SELECT create_graph('knowledge')` (idempotent — skip if graph exists)

### Backfill Script: `packages/db/src/age/backfill.ts`

Reads existing relational data and populates the AGE graph:
1. All chunks → `chunk` vertices
2. All requirements → `requirement` vertices
3. All `chunkConnection` rows → `connects` edges
4. All `requirementDependency` rows → `depends_on` edges
5. All `requirementChunk` rows → `covers` edges

Idempotent via `MERGE` — safe to re-run. Logs progress.

### Connection Initialization

AGE requires `LOAD 'age'` and `SET search_path` on each new database connection. A setup hook is added to the Drizzle db pool initialization in `packages/db/src/index.ts` that runs on each new connection from the pool.

### CLI Command

`pnpm age:setup` — runs setup + backfill. Added to `package.json` in `packages/db`.

### Deployment Constraint

Railway's managed PostgreSQL may not support the AGE extension. Options if it doesn't:
- Custom Docker image with AGE pre-installed
- Switch to a provider that supports custom extensions
- Self-hosted PostgreSQL

This constraint should be validated early before implementation begins.

---

## 5. What Doesn't Change

- **Drizzle ORM** — stays as primary data access for all entity CRUD
- **Relational tables** — `chunk`, `chunkConnection`, `requirement`, `requirementChunk`, `requirementDependency` all stay as source of truth
- **Frontend graph rendering** — React Flow and `/api/graph` endpoint shape unchanged; backend sources data differently for traversal queries
- **Tags, codebases, workspaces, plans, sessions** — purely relational, no AGE involvement
- **Embeddings and semantic search** — pgvector stays; AGE and pgvector coexist as PostgreSQL extensions
- **Effect error handling** — AGE queries use same `Effect.tryPromise` + `DatabaseError` pattern

### Rollback Path

Since relational tables remain source of truth and AGE is a synced projection, removing AGE is straightforward: delete sync calls, remove extension, revert to current SQL/client-side queries. No data loss.

---

## Out of Scope

- Moving entity data (chunk content, tags, embeddings) into AGE vertex properties
- Graph-based recommendation engine or ML features
- Replacing Drizzle with a graph ORM
- Tags, codebases, plan/session relationships as AGE edges
- AGE-based access control or multi-tenancy
