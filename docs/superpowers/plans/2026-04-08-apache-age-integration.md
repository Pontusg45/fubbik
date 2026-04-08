# Apache AGE Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Apache AGE as a graph query layer alongside PostgreSQL, with write-through sync from relational tables and Cypher queries replacing client-side graph traversals.

**Architecture:** AGE graph `knowledge` stores chunk/requirement vertices and connects/depends_on/covers edges. Relational tables remain source of truth. A thin sync layer in `packages/db/src/age/` mirrors writes. A query layer provides Cypher-backed functions that replace client-side BFS and recursive CTEs.

**Tech Stack:** Apache AGE (PostgreSQL extension), Cypher query language, Drizzle ORM (raw SQL via `db.execute`), Effect

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `packages/db/src/age/client.ts` | Create | AGE connection setup, `cypher()` helper |
| `packages/db/src/age/sync.ts` | Create | Write-through sync functions |
| `packages/db/src/age/query.ts` | Create | Cypher query functions |
| `packages/db/src/age/setup.ts` | Create | Extension + graph creation script |
| `packages/db/src/age/backfill.ts` | Create | Backfill existing data into AGE |
| `packages/db/src/repository/connection.ts` | Modify | Add sync calls to create/delete |
| `packages/db/src/repository/chunk.ts` | Modify | Add sync calls to create/delete |
| `packages/db/src/repository/requirement.ts` | Modify | Add sync calls to create/delete |
| `packages/db/src/repository/requirement-dependency.ts` | Modify | Add sync calls + replace CTEs with Cypher |
| `packages/db/src/repository/knowledge-health.ts` | Modify | Replace orphan detection with Cypher |
| `packages/db/package.json` | Modify | Add `age:setup` script |

---

### Task 1: AGE Client and Cypher Helper

**Files:**
- Create: `packages/db/src/age/client.ts`

**Context:** Apache AGE requires `LOAD 'age'` and `SET search_path` on each new connection. We need a helper that wraps Cypher queries into the SQL format AGE expects: `SELECT * FROM cypher('knowledge', $$ CYPHER_QUERY $$) AS (result agtype)`.

- [ ] **Step 1: Create the AGE client module**

```typescript
// packages/db/src/age/client.ts
import { sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";

/**
 * Initialize AGE on the current connection.
 * Must be called once per connection before any Cypher queries.
 */
export function initAge() {
    return Effect.tryPromise({
        try: async () => {
            await db.execute(sql`LOAD 'age'`);
            await db.execute(sql`SET search_path = ag_catalog, "$user", public`);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

/**
 * Execute a Cypher query against the 'knowledge' graph.
 * Returns raw rows from AGE.
 *
 * @param query - Cypher query string (will be embedded in $$ delimiters)
 * @param returnType - SQL column definitions for the RETURNS clause, e.g. "v agtype"
 */
export function cypher(query: string, returnType = "v agtype") {
    return Effect.tryPromise({
        try: async () => {
            const result = await db.execute(
                sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (${returnType})`)
            );
            return result.rows;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

/**
 * Execute a Cypher query that returns no results (CREATE, MERGE, DELETE).
 */
export function cypherVoid(query: string) {
    return Effect.tryPromise({
        try: async () => {
            await db.execute(
                sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (v agtype)`)
            );
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | grep "age/client"`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/age/client.ts
git commit -m "feat(age): add AGE client with cypher query helper"
```

---

### Task 2: AGE Setup and Backfill Scripts

**Files:**
- Create: `packages/db/src/age/setup.ts`
- Create: `packages/db/src/age/backfill.ts`
- Modify: `packages/db/package.json`

**Context:** Setup creates the AGE extension and graph. Backfill reads existing relational data and creates vertices/edges. Both are idempotent and run via `pnpm age:setup`.

- [ ] **Step 1: Create the setup script**

```typescript
// packages/db/src/age/setup.ts
import { resolve } from "path";

import { config } from "dotenv";
import { sql } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

config({ path: resolve(import.meta.dirname, "../../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

console.log("Setting up Apache AGE...");

// 1. Create extension
await db.execute(sql`CREATE EXTENSION IF NOT EXISTS age`);
console.log("  ✓ Extension created");

// 2. Load and configure
await db.execute(sql`LOAD 'age'`);
await db.execute(sql.raw(`SET search_path = ag_catalog, "$user", public`));
console.log("  ✓ AGE loaded");

// 3. Create graph (idempotent)
try {
    await db.execute(sql.raw(`SELECT create_graph('knowledge')`));
    console.log("  ✓ Graph 'knowledge' created");
} catch (e: any) {
    if (e?.message?.includes("already exists")) {
        console.log("  ✓ Graph 'knowledge' already exists");
    } else {
        throw e;
    }
}

console.log("\n✅ AGE setup complete");

// Now run backfill
await import("./backfill.js");
```

- [ ] **Step 2: Create the backfill script**

```typescript
// packages/db/src/age/backfill.ts
import { resolve } from "path";

import { config } from "dotenv";
import { sql } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

import { chunk, chunkConnection } from "../schema/chunk";
import { requirement, requirementChunk } from "../schema/requirement";
import { requirementDependency } from "../schema/requirement-dependency";

config({ path: resolve(import.meta.dirname, "../../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

// Ensure AGE is loaded on this connection
await db.execute(sql`LOAD 'age'`);
await db.execute(sql.raw(`SET search_path = ag_catalog, "$user", public`));

function cypherExec(query: string) {
    return db.execute(sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (v agtype)`));
}

console.log("\nBackfilling AGE graph...");

// 1. Chunk vertices
const allChunks = await db.select({ id: chunk.id }).from(chunk);
for (const c of allChunks) {
    await cypherExec(`MERGE (:chunk {id: '${c.id}'})`);
}
console.log(`  ✓ ${allChunks.length} chunk vertices`);

// 2. Requirement vertices
const allRequirements = await db.select({ id: requirement.id }).from(requirement);
for (const r of allRequirements) {
    await cypherExec(`MERGE (:requirement {id: '${r.id}'})`);
}
console.log(`  ✓ ${allRequirements.length} requirement vertices`);

// 3. connects edges (chunk → chunk)
const allConnections = await db
    .select({
        id: chunkConnection.id,
        sourceId: chunkConnection.sourceId,
        targetId: chunkConnection.targetId,
        relation: chunkConnection.relation
    })
    .from(chunkConnection);
for (const conn of allConnections) {
    await cypherExec(
        `MATCH (a:chunk {id: '${conn.sourceId}'}), (b:chunk {id: '${conn.targetId}'})
         MERGE (a)-[:connects {id: '${conn.id}', relation: '${conn.relation}'}]->(b)`
    );
}
console.log(`  ✓ ${allConnections.length} connects edges`);

// 4. depends_on edges (requirement → requirement)
const allDeps = await db
    .select({
        requirementId: requirementDependency.requirementId,
        dependsOnId: requirementDependency.dependsOnId
    })
    .from(requirementDependency);
for (const dep of allDeps) {
    await cypherExec(
        `MATCH (a:requirement {id: '${dep.requirementId}'}), (b:requirement {id: '${dep.dependsOnId}'})
         MERGE (a)-[:depends_on]->(b)`
    );
}
console.log(`  ✓ ${allDeps.length} depends_on edges`);

// 5. covers edges (requirement → chunk)
const allCovers = await db
    .select({
        requirementId: requirementChunk.requirementId,
        chunkId: requirementChunk.chunkId
    })
    .from(requirementChunk);
for (const rc of allCovers) {
    await cypherExec(
        `MATCH (r:requirement {id: '${rc.requirementId}'}), (c:chunk {id: '${rc.chunkId}'})
         MERGE (r)-[:covers]->(c)`
    );
}
console.log(`  ✓ ${allCovers.length} covers edges`);

console.log("\n✅ Backfill complete");
process.exit(0);
```

- [ ] **Step 3: Add the `age:setup` script to package.json**

In `packages/db/package.json`, add to the `scripts` section:

```json
"age:setup": "bun run src/age/setup.ts"
```

- [ ] **Step 4: Test locally**

First install the AGE extension on your local PostgreSQL (this varies by system — on macOS with Homebrew you may need to build from source similar to pgvector).

Then run:
```bash
pnpm --filter @fubbik/db run age:setup
```

Expected output:
```
Setting up Apache AGE...
  ✓ Extension created
  ✓ AGE loaded
  ✓ Graph 'knowledge' created

Backfilling AGE graph...
  ✓ 27 chunk vertices
  ✓ 7 requirement vertices
  ✓ 37 connects edges
  ✓ N depends_on edges
  ✓ N covers edges

✅ Backfill complete
```

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/setup.ts packages/db/src/age/backfill.ts packages/db/package.json
git commit -m "feat(age): add setup and backfill scripts for AGE graph"
```

---

### Task 3: Write-Through Sync Layer

**Files:**
- Create: `packages/db/src/age/sync.ts`

**Context:** Thin sync functions called from existing repository functions. Each function executes a single Cypher MERGE/DELETE via the `cypherVoid` helper from Task 1.

- [ ] **Step 1: Create the sync module**

```typescript
// packages/db/src/age/sync.ts
import { Effect } from "effect";

import { cypherVoid } from "./client";

/** Idempotent — creates vertex if it doesn't exist. */
export function ensureVertex(label: string, id: string) {
    return cypherVoid(`MERGE (:${label} {id: '${id}'})`);
}

/** Remove a vertex and all its edges. */
export function deleteVertex(label: string, id: string) {
    return cypherVoid(`MATCH (v:${label} {id: '${id}'}) DETACH DELETE v`);
}

/** Create a directed edge between two vertices. */
export function createEdge(
    edgeLabel: string,
    fromLabel: string,
    fromId: string,
    toLabel: string,
    toId: string,
    props: Record<string, string> = {}
) {
    const propsStr = Object.entries(props)
        .map(([k, v]) => `${k}: '${v}'`)
        .join(", ");
    const propsClause = propsStr ? ` {${propsStr}}` : "";
    return cypherVoid(
        `MATCH (a:${fromLabel} {id: '${fromId}'}), (b:${toLabel} {id: '${toId}'})
         CREATE (a)-[:${edgeLabel}${propsClause}]->(b)`
    );
}

/** Delete an edge by matching its properties. */
export function deleteEdge(edgeLabel: string, props: Record<string, string>) {
    const conditions = Object.entries(props)
        .map(([k, v]) => `e.${k} = '${v}'`)
        .join(" AND ");
    return cypherVoid(
        `MATCH ()-[e:${edgeLabel}]-() WHERE ${conditions} DELETE e`
    );
}

/** Delete all edges of a type from a vertex. */
export function deleteEdgesFrom(edgeLabel: string, fromLabel: string, fromId: string) {
    return cypherVoid(
        `MATCH (a:${fromLabel} {id: '${fromId}'})-[e:${edgeLabel}]->() DELETE e`
    );
}
```

- [ ] **Step 2: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | grep "age/sync"`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/age/sync.ts
git commit -m "feat(age): add write-through sync layer for AGE graph"
```

---

### Task 4: Integrate Sync into Repository Functions

**Files:**
- Modify: `packages/db/src/repository/connection.ts`
- Modify: `packages/db/src/repository/chunk.ts`
- Modify: `packages/db/src/repository/requirement.ts`
- Modify: `packages/db/src/repository/requirement-dependency.ts`

**Context:** Each repository write function gets a sync call appended. The sync is fire-and-forget wrapped in `Effect.catchAll` so AGE failures don't break relational writes during the transition period. Once AGE is stable, the catch can be removed.

- [ ] **Step 1: Add sync to connection.ts**

At the top, add import:
```typescript
import { ensureVertex, createEdge, deleteEdge } from "../age/sync";
```

In `createConnection`, after the `db.insert` returns, add:
```typescript
// Sync to AGE graph
await Effect.runPromise(
    ensureVertex("chunk", params.sourceId).pipe(
        Effect.flatMap(() => ensureVertex("chunk", params.targetId)),
        Effect.flatMap(() => createEdge("connects", "chunk", params.sourceId, "chunk", params.targetId, { id: params.id, relation: params.relation })),
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

In `deleteConnection`, after `db.delete` returns and we have the deleted row, add:
```typescript
if (deleted) {
    await Effect.runPromise(
        deleteEdge("connects", { id: connectionId }).pipe(
            Effect.catchAll(() => Effect.succeed(undefined))
        )
    );
}
```

- [ ] **Step 2: Add sync to chunk.ts**

At the top, add import:
```typescript
import { ensureVertex, deleteVertex } from "../age/sync";
```

In `createChunk`, after `db.insert` returns, add:
```typescript
await Effect.runPromise(
    ensureVertex("chunk", created.id).pipe(
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

In `deleteChunk`, after `db.delete` returns and we have the deleted row, add:
```typescript
if (deleted) {
    await Effect.runPromise(
        deleteVertex("chunk", chunkId).pipe(
            Effect.catchAll(() => Effect.succeed(undefined))
        )
    );
}
```

In `deleteMany`, after `db.delete` returns, add:
```typescript
for (const row of result) {
    await Effect.runPromise(
        deleteVertex("chunk", row.id).pipe(
            Effect.catchAll(() => Effect.succeed(undefined))
        )
    );
}
```

- [ ] **Step 3: Add sync to requirement.ts**

At the top, add import:
```typescript
import { ensureVertex, deleteVertex, createEdge, deleteEdgesFrom } from "../age/sync";
```

In `createRequirement`, after the insert, add:
```typescript
await Effect.runPromise(
    ensureVertex("requirement", created.id).pipe(
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

In `deleteRequirement`, after the delete, add:
```typescript
await Effect.runPromise(
    deleteVertex("requirement", id).pipe(
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

In `setRequirementChunks`, after deleting old and inserting new chunk links, add:
```typescript
// Sync covers edges to AGE
await Effect.runPromise(
    deleteEdgesFrom("covers", "requirement", requirementId).pipe(
        Effect.flatMap(() =>
            Effect.all(
                chunkIds.map(chunkId =>
                    createEdge("covers", "requirement", requirementId, "chunk", chunkId)
                ),
                { concurrency: 1 }
            )
        ),
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

- [ ] **Step 4: Add sync to requirement-dependency.ts**

At the top, add import:
```typescript
import { ensureVertex, createEdge, deleteEdge } from "../age/sync";
```

In `addDependency`, after `db.insert`, add:
```typescript
await Effect.runPromise(
    ensureVertex("requirement", requirementId).pipe(
        Effect.flatMap(() => ensureVertex("requirement", dependsOnId)),
        Effect.flatMap(() => createEdge("depends_on", "requirement", requirementId, "requirement", dependsOnId)),
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

In `removeDependency`, after `db.delete`, add:
```typescript
await Effect.runPromise(
    deleteEdge("depends_on", { }).pipe(
        Effect.catchAll(() => Effect.succeed(undefined))
    )
);
```

Note: For `removeDependency`, the edge has no unique `id` property. Use a Cypher match on the endpoint vertices instead:
```typescript
import { cypherVoid } from "../age/client";

// In removeDependency, after db.delete:
await Effect.runPromise(
    cypherVoid(
        `MATCH (a:requirement {id: '${requirementId}'})-[e:depends_on]->(b:requirement {id: '${dependsOnId}'}) DELETE e`
    ).pipe(Effect.catchAll(() => Effect.succeed(undefined)))
);
```

- [ ] **Step 5: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | head -20`

Expected: No new errors in the modified repository files.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/connection.ts packages/db/src/repository/chunk.ts packages/db/src/repository/requirement.ts packages/db/src/repository/requirement-dependency.ts
git commit -m "feat(age): integrate write-through sync into repository functions"
```

---

### Task 5: Cypher Query Layer

**Files:**
- Create: `packages/db/src/age/query.ts`

**Context:** These functions replace client-side BFS and recursive CTEs with Cypher queries. They return plain ID arrays that callers join with relational data.

- [ ] **Step 1: Create the query module**

```typescript
// packages/db/src/age/query.ts
import { Effect } from "effect";

import { cypher } from "./client";

/**
 * Find shortest path between two chunks (undirected).
 * Returns ordered array of chunk IDs, or null if no path exists.
 */
export function findShortestPath(chunkIdA: string, chunkIdB: string) {
    return cypher(
        `MATCH p = shortestPath((a:chunk {id: '${chunkIdA}'})-[*]-(b:chunk {id: '${chunkIdB}'}))
         RETURN [n IN nodes(p) | n.id] AS path`,
        "path agtype"
    ).pipe(
        Effect.map(rows => {
            if (rows.length === 0) return null;
            const raw = (rows[0] as any)?.path;
            if (!raw) return null;
            // AGE returns agtype — parse the array
            return (typeof raw === "string" ? JSON.parse(raw) : raw) as string[];
        })
    );
}

/**
 * Get all chunk IDs within N hops of a given chunk (undirected).
 */
export function getNeighborhood(chunkId: string, maxHops: number) {
    return cypher(
        `MATCH (c:chunk {id: '${chunkId}'})-[*1..${maxHops}]-(neighbor:chunk)
         RETURN DISTINCT neighbor.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => {
            const val = r.id;
            return (typeof val === "string" ? val.replace(/^"|"$/g, "") : val) as string;
        }))
    );
}

/**
 * Get all transitive dependencies of a requirement (directed).
 * Returns ancestor IDs, descendant IDs, and edge pairs.
 */
export function getTransitiveDeps(requirementId: string) {
    return Effect.gen(function* () {
        const ancestorRows = yield* cypher(
            `MATCH (r:requirement {id: '${requirementId}'})-[:depends_on*]->(dep:requirement)
             RETURN dep.id AS id`,
            "id agtype"
        );
        const ancestors = ancestorRows.map((r: any) => {
            const val = r.id;
            return (typeof val === "string" ? val.replace(/^"|"$/g, "") : val) as string;
        });

        const descendantRows = yield* cypher(
            `MATCH (dep:requirement)-[:depends_on*]->(r:requirement {id: '${requirementId}'})
             RETURN dep.id AS id`,
            "id agtype"
        );
        const descendants = descendantRows.map((r: any) => {
            const val = r.id;
            return (typeof val === "string" ? val.replace(/^"|"$/g, "") : val) as string;
        });

        const allIds = [requirementId, ...ancestors, ...descendants];
        const edgeRows = yield* cypher(
            `MATCH (a:requirement)-[e:depends_on]->(b:requirement)
             WHERE a.id IN [${allIds.map(id => `'${id}'`).join(",")}]
                OR b.id IN [${allIds.map(id => `'${id}'`).join(",")}]
             RETURN a.id AS source, b.id AS target`,
            "source agtype, target agtype"
        );
        const edges = edgeRows.map((r: any) => ({
            source: (typeof r.source === "string" ? r.source.replace(/^"|"$/g, "") : r.source) as string,
            target: (typeof r.target === "string" ? r.target.replace(/^"|"$/g, "") : r.target) as string
        }));

        return { ancestors, descendants, edges };
    });
}

/**
 * Check if adding a dependency would create a cycle.
 */
export function checkCircular(requirementId: string, dependsOnId: string) {
    return cypher(
        `MATCH (start:requirement {id: '${dependsOnId}'})-[:depends_on*]->(end:requirement {id: '${requirementId}'})
         RETURN 1 AS found LIMIT 1`,
        "found agtype"
    ).pipe(
        Effect.map(rows => rows.length > 0)
    );
}

/**
 * Find all chunks affected by a requirement (traverses covers + connects edges).
 */
export function getChunksAffectedByRequirement(requirementId: string, hops: number) {
    return cypher(
        `MATCH (r:requirement {id: '${requirementId}'})-[:covers]->(c:chunk)-[:connects*0..${hops}]-(related:chunk)
         RETURN DISTINCT related.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => {
            const val = r.id;
            return (typeof val === "string" ? val.replace(/^"|"$/g, "") : val) as string;
        }))
    );
}

/**
 * Get subgraph — edges between a given set of chunk IDs.
 */
export function getSubgraph(chunkIds: string[]) {
    const idList = chunkIds.map(id => `'${id}'`).join(",");
    return cypher(
        `MATCH (a:chunk)-[e:connects]->(b:chunk)
         WHERE a.id IN [${idList}] AND b.id IN [${idList}]
         RETURN a.id AS source, e.relation AS relation, b.id AS target`,
        "source agtype, relation agtype, target agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => ({
            source: (typeof r.source === "string" ? r.source.replace(/^"|"$/g, "") : r.source) as string,
            relation: (typeof r.relation === "string" ? r.relation.replace(/^"|"$/g, "") : r.relation) as string,
            target: (typeof r.target === "string" ? r.target.replace(/^"|"$/g, "") : r.target) as string
        })))
    );
}

/**
 * Find orphan chunks (no edges at all).
 */
export function getOrphanChunkIds() {
    return cypher(
        `MATCH (c:chunk) WHERE NOT (c)-[]-() RETURN c.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => {
            const val = r.id;
            return (typeof val === "string" ? val.replace(/^"|"$/g, "") : val) as string;
        }))
    );
}
```

- [ ] **Step 2: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | grep "age/query"`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/age/query.ts
git commit -m "feat(age): add Cypher query layer for graph traversals"
```

---

### Task 6: Replace Client-Side Graph Traversals and Recursive CTEs

**Files:**
- Modify: `packages/db/src/repository/requirement-dependency.ts`
- Modify: `packages/db/src/repository/knowledge-health.ts`
- Modify: `packages/db/src/repository/index.ts` (re-export AGE queries)

**Context:** Now that the Cypher query layer exists, replace the recursive CTEs in requirement-dependency.ts with AGE queries, and the orphan detection in knowledge-health.ts. Also export the new query functions so the API layer can use them.

- [ ] **Step 1: Replace `getTransitiveDependencies` in requirement-dependency.ts**

Replace the body of `getTransitiveDependencies` (the function with two recursive CTEs) with a delegation to the AGE query:

```typescript
import { getTransitiveDeps as ageGetTransitiveDeps } from "../age/query";

export function getTransitiveDependencies(requirementId: string) {
    return ageGetTransitiveDeps(requirementId).pipe(
        Effect.map(({ ancestors, descendants, edges }) => ({
            ancestors: ancestors.map(id => ({ id })),
            descendants: descendants.map(id => ({ id })),
            edges: edges.map(e => ({ source: e.source, target: e.target }))
        }))
    );
}
```

Note: The return shape changes slightly — callers that expect `title`/`status`/`priority` on ancestors/descendants will need to join those from relational data. Check callers and adjust if needed. If callers need full requirement data, keep the existing CTE as a fallback and add the AGE version as an alternative.

- [ ] **Step 2: Replace `checkCircularDependency`**

```typescript
import { checkCircular as ageCheckCircular } from "../age/query";

export function checkCircularDependency(requirementId: string, dependsOnId: string) {
    return ageCheckCircular(requirementId, dependsOnId);
}
```

- [ ] **Step 3: Add AGE-based orphan detection option to knowledge-health.ts**

Add at the bottom of `packages/db/src/repository/knowledge-health.ts`:

```typescript
import { getOrphanChunkIds } from "../age/query";

/**
 * Get orphan chunks using AGE graph query (faster for large graphs).
 * Returns chunk IDs only — caller joins with relational data.
 */
export function getOrphanChunkIdsViaAge() {
    return getOrphanChunkIds();
}
```

This doesn't replace the existing `getOrphanChunks` (which returns full chunk data + count) but provides an efficient alternative that callers can opt into.

- [ ] **Step 4: Export AGE queries from the repository index**

In `packages/db/src/repository/index.ts`, add:

```typescript
export * from "../age/query";
```

- [ ] **Step 5: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | head -20`

Expected: No new errors.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/requirement-dependency.ts packages/db/src/repository/knowledge-health.ts packages/db/src/repository/index.ts
git commit -m "feat(age): replace recursive CTEs and add AGE-based orphan detection"
```
