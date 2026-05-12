# PostgreSQL Graph Extensions Deep Utilization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deeply leverage the existing PostgreSQL graph extensions (Apache AGE, pgvector, pg_trgm) to improve retrieval relevance, scoring accuracy, staleness detection, search performance, and knowledge clustering.

**Architecture:** Six independent features that build on the existing extension stack. Features 1-3 enhance the scoring/retrieval pipeline. Feature 4 adds missing performance indexes. Feature 5 extends staleness detection with graph traversal. Feature 6 adds community detection for knowledge clustering. Each feature produces working, testable software independently.

**Tech Stack:** PostgreSQL (AGE, pgvector, pg_trgm), Drizzle ORM, Effect, vitest

---

## File Structure Overview

```
packages/db/src/
├── age/
│   ├── query.ts              # Modify: add centrality, community, impact queries
│   └── query.test.ts         # Modify: add tests for new queries
├── repository/
│   ├── semantic.ts           # Modify: add hybrid search function
│   └── semantic.test.ts      # Create: tests for hybrid search
├── migrations/
│   └── 0002_graph_indexes.sql  # Create: GIN + HNSW indexes

packages/api/src/
├── context-for-file/
│   └── service.ts            # Modify: integrate hybrid scoring
├── context/
│   └── utils.ts              # Modify: add centrality to scoreChunk
├── chunks/
│   └── health-score.ts       # Modify: add centrality dimension
├── staleness/
│   ├── detect-impact.ts      # Create: graph-based impact propagation
│   ├── routes.ts             # Modify: add impact scan endpoint
│   └── service.ts            # Modify: re-export impact functions
├── graph/
│   ├── service.ts            # Modify: add community data to graph response
│   └── routes.ts             # Modify: add communities endpoint
```

---

## Feature 1: GIN Indexes for pg_trgm (Performance Foundation)

This is the simplest feature and should go first — it makes every downstream text search faster.

### Task 1.1: Add GIN trigram and HNSW vector indexes

**Files:**
- Create: `packages/db/src/migrations/0002_graph_indexes.sql`

- [ ] **Step 1: Create the migration file**

```sql
-- 0002_graph_indexes.sql
-- GIN trigram indexes for fast fuzzy text search via pg_trgm
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_title_trgm_idx
  ON chunk USING GIN (title gin_trgm_ops);

CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_content_trgm_idx
  ON chunk USING GIN (content gin_trgm_ops);

-- HNSW index for fast approximate nearest-neighbor vector search via pgvector
-- m=16 (connections per node), ef_construction=64 (build-time accuracy)
-- Tuned for 768-dim nomic-embed-text embeddings
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_embedding_hnsw_idx
  ON chunk USING hnsw (embedding vector_cosine_ops)
  WITH (m = 16, ef_construction = 64);
```

- [ ] **Step 2: Update the Drizzle migration journal**

The migration journal lives at `packages/db/src/migrations/meta/_journal.json`. Add an entry for the new migration:

```json
{
  "idx": 2,
  "version": "7",
  "when": 1715126400000,
  "tag": "0002_graph_indexes",
  "breakpoints": true
}
```

Append this to the existing `entries` array in the journal.

- [ ] **Step 3: Run the migration against your local database**

Run: `cd packages/db && pnpm db:migrate`

Expected: Migration applies successfully. If `db:migrate` doesn't pick up raw SQL, apply directly:

```bash
psql $DATABASE_URL -f packages/db/src/migrations/0002_graph_indexes.sql
```

- [ ] **Step 4: Verify indexes exist**

Run:
```bash
psql $DATABASE_URL -c "\di chunk_title_trgm_idx; \di chunk_content_trgm_idx; \di chunk_embedding_hnsw_idx"
```

Expected: Three indexes listed with correct types (GIN, GIN, HNSW).

- [ ] **Step 5: Verify query plan uses the new indexes**

Run:
```bash
psql $DATABASE_URL -c "EXPLAIN ANALYZE SELECT id, similarity(title, 'authentication') AS sim FROM chunk WHERE similarity(title, 'authentication') > 0.15 ORDER BY sim DESC LIMIT 10;"
```

Expected: Plan shows `Bitmap Index Scan on chunk_title_trgm_idx`.

Run:
```bash
psql $DATABASE_URL -c "EXPLAIN ANALYZE SELECT id FROM chunk ORDER BY embedding <=> '[0.1,0.2,...]'::vector LIMIT 10;"
```

(Use a real 768-dim vector or a short test.) Expected: Plan shows `Index Scan using chunk_embedding_hnsw_idx`.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/migrations/0002_graph_indexes.sql packages/db/src/migrations/meta/_journal.json
git commit -m "perf: add GIN trigram and HNSW vector indexes for chunk search"
```

---

## Feature 2: Centrality Scoring via AGE

Add a graph-centrality metric to the health score. Chunks that sit on many shortest paths between other chunks are "load-bearing knowledge."

### Task 2.1: Add centrality query to AGE

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test for `getConnectionDegrees`**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    checkCircular,
    findShortestPath,
    getConnectionDegrees,
    getNeighborhood,
    getOrphanChunkIds
} from "./query";

// ... existing tests ...

describe("getConnectionDegrees", () => {
    it("returns correct degree counts for known graph", async () => {
        if (!ageReady) return;
        const degrees = await Effect.runPromise(
            getConnectionDegrees([uid("A"), uid("B"), uid("C"), uid("center"), uid("orphan")])
        );
        // B has 2 edges (A→B, B→C), center has 3 edges (3 spokes)
        expect(degrees.get(uid("B"))).toBe(2);
        expect(degrees.get(uid("center"))).toBe(3);
        // A and C have 1 edge each
        expect(degrees.get(uid("A"))).toBe(1);
        expect(degrees.get(uid("C"))).toBe(1);
        // Orphan has 0
        expect(degrees.get(uid("orphan"))).toBeUndefined();
    });

    it("returns empty map for empty input", async () => {
        if (!ageReady) return;
        const degrees = await Effect.runPromise(getConnectionDegrees([]));
        expect(degrees.size).toBe(0);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "getConnectionDegrees"`

Expected: FAIL — `getConnectionDegrees` is not exported from `./query`.

- [ ] **Step 3: Implement `getConnectionDegrees` in `query.ts`**

Add to `packages/db/src/age/query.ts`, after the existing `getOrphanChunkIds` function:

```typescript
export function getConnectionDegrees(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed(new Map<string, number>());

    const idList = chunkIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (c:chunk)-[e]-()
         WHERE c.id IN [${idList}]
         RETURN c.id AS id, count(e) AS degree`,
        "id agtype, degree agtype"
    ).pipe(
        Effect.map(rows => {
            const map = new Map<string, number>();
            for (const row of rows) {
                const id = parseAgtypeId((row as any).id);
                const degree = Number((row as any).degree);
                map.set(id, degree);
            }
            return map;
        })
    );
}
```

Add the `escCypher` import at the top of query.ts if not already there:

```typescript
import { cypher, escCypher } from "./client";
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "getConnectionDegrees"`

Expected: PASS (or skip if AGE not available locally).

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: add getConnectionDegrees AGE query for centrality scoring"
```

### Task 2.2: Integrate centrality into health score

**Files:**
- Modify: `packages/api/src/chunks/health-score.ts`
- Modify: `packages/api/src/context/utils.ts`

- [ ] **Step 1: Add `centralityDegree` to `ChunkHealthInput`**

In `packages/api/src/chunks/health-score.ts`, update the interface:

```typescript
export interface ChunkHealthInput {
    content: string;
    updatedAt: Date;
    summary: string | null;
    rationale: string | null;
    alternatives: string[] | null;
    consequences: string | null;
    connectionCount: number;
    centralityDegree: number;
    hasEmbedding: boolean;
    requirementCount: number;
    allRequirementsPassing: boolean;
    referencedInSession: boolean;
}
```

- [ ] **Step 2: Replace the flat connectivity scoring with a centrality-aware version**

In `packages/api/src/chunks/health-score.ts`, replace the connectivity block (lines 65-74):

Old:
```typescript
    // Connectivity (0-20): 20 for 3+, 12 for 1-2, 0 for orphans
    let connectivity: number;
    if (input.connectionCount >= 3) {
        connectivity = 20;
    } else if (input.connectionCount >= 1) {
        connectivity = 12;
    } else {
        connectivity = 0;
        issues.push("Orphan chunk with no connections");
    }
```

New:
```typescript
    // Connectivity (0-20): base from connection count + bonus from centrality degree
    let connectivity: number;
    if (input.connectionCount === 0) {
        connectivity = 0;
        issues.push("Orphan chunk with no connections");
    } else {
        // Base: 8 for 1-2 connections, 12 for 3+
        const base = input.connectionCount >= 3 ? 12 : 8;
        // Centrality bonus: up to 8 points for high-degree nodes
        const centralityBonus = Math.min(Math.floor(input.centralityDegree / 2), 8);
        connectivity = Math.min(base + centralityBonus, 20);
    }
```

- [ ] **Step 3: Update all callers to pass `centralityDegree`**

In `packages/api/src/context/utils.ts`, update the `scoreChunk` call (line 41-53):

```typescript
export function scoreChunk(c: ChunkRow, connectionCount: number, centralityDegree = 0): number {
    const health = computeHealthScore({
        content: c.content,
        updatedAt: c.updatedAt,
        summary: c.summary,
        rationale: c.rationale,
        alternatives: c.alternatives,
        consequences: c.consequences,
        connectionCount,
        centralityDegree,
        hasEmbedding: c.embedding != null,
        requirementCount: 0,
        allRequirementsPassing: false,
        referencedInSession: false
    });
```

- [ ] **Step 4: Find and update all other callers of `computeHealthScore`**

Search for `computeHealthScore(` in the codebase and add `centralityDegree: 0` to every call site that doesn't have it yet. Key locations:

- `packages/api/src/chunks/service.ts` (chunk detail) — use `centralityDegree: 0` for now; will be wired in Task 2.3.
- `packages/api/src/search/service.ts` (search results) — use `centralityDegree: 0`.

- [ ] **Step 5: Run the full test suite**

Run: `cd packages/api && pnpm test`

Expected: PASS — all existing tests pass with the new optional parameter defaulting to 0.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/chunks/health-score.ts packages/api/src/context/utils.ts
git add -u  # any other files touched for caller updates
git commit -m "feat: integrate centrality degree into health score connectivity dimension"
```

### Task 2.3: Wire centrality into the context-for-file pipeline

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`

- [ ] **Step 1: Import `getConnectionDegrees` and use it in scoring**

In `packages/api/src/context-for-file/service.ts`, add the import:

```typescript
import { getAppliesToForChunks, getChunkById, getConnectionDegrees, getConnectionsForChunks, getRequirementsForChunks, listChunks, listCodebases, lookupChunksByFilePath, semanticSearch as semanticSearchRepo } from "@fubbik/db/repository";
```

Then in the scoring section (around line 237), after building `connCountMap`, add centrality fetch:

```typescript
        // Fetch centrality degrees from AGE graph
        const degreeMap = chunkIdsForScoring.length > 0
            ? yield* getConnectionDegrees(chunkIdsForScoring).pipe(
                  Effect.catchAll(() => Effect.succeed(new Map<string, number>())),
              )
            : new Map<string, number>();
```

Update the scoring loop (around line 259) to pass centrality:

```typescript
        for (const chunk of matchedChunks) {
            const rawRow = chunkRows.get(chunk.id);
            const connectionCount = connCountMap.get(chunk.id) ?? 0;
            const centralityDegree = degreeMap.get(chunk.id) ?? 0;
            const baseScore = rawRow ? scoreChunk(rawRow, connectionCount, centralityDegree) : 0;
            chunk.score = baseScore + (STRATEGY_BONUS[chunk.matchReason] ?? 0);
        }
```

- [ ] **Step 2: Update `scoreChunk` import in service.ts if needed**

The function signature already accepts the optional third parameter from Task 2.2. No import change needed.

- [ ] **Step 3: Run existing context-for-file tests**

Run: `cd packages/api && pnpm test -- --grep "getContextForFile"`

Expected: PASS — `getConnectionDegrees` is mocked via `@fubbik/db/repository` mock.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context-for-file/service.ts
git commit -m "feat: wire centrality degrees into context-for-file scoring pipeline"
```

---

## Feature 3: Hybrid Graph + Vector Retrieval

Combine semantic similarity (pgvector) with graph proximity (AGE) so chunks that are both semantically relevant AND graph-connected get a higher score than either signal alone.

### Task 3.1: Add `getGraphProximityBoost` to AGE queries

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    checkCircular,
    findShortestPath,
    getConnectionDegrees,
    getGraphProximityBoost,
    getNeighborhood,
    getOrphanChunkIds
} from "./query";

// ... existing tests ...

describe("getGraphProximityBoost", () => {
    it("returns high boost for directly connected chunks", async () => {
        if (!ageReady) return;
        const boosts = await Effect.runPromise(
            getGraphProximityBoost(uid("A"), [uid("B"), uid("C"), uid("orphan")], 3)
        );
        // B is 1 hop from A → high boost
        expect(boosts.get(uid("B"))).toBeGreaterThan(boosts.get(uid("C"))!);
        // C is 2 hops from A → medium boost
        expect(boosts.get(uid("C"))).toBeGreaterThan(0);
        // orphan is unreachable → no entry
        expect(boosts.has(uid("orphan"))).toBe(false);
    });

    it("returns empty map when anchor has no connections", async () => {
        if (!ageReady) return;
        const boosts = await Effect.runPromise(
            getGraphProximityBoost(uid("orphan"), [uid("A"), uid("B")], 3)
        );
        expect(boosts.size).toBe(0);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "getGraphProximityBoost"`

Expected: FAIL — not exported.

- [ ] **Step 3: Implement `getGraphProximityBoost`**

Add to `packages/db/src/age/query.ts`:

```typescript
export function getGraphProximityBoost(
    anchorId: string,
    candidateIds: string[],
    maxHops: number
) {
    if (candidateIds.length === 0) return Effect.succeed(new Map<string, number>());

    const idList = candidateIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (anchor:chunk {id: '${escCypher(anchorId)}'}), (target:chunk)
         WHERE target.id IN [${idList}]
         MATCH p = shortestPath((anchor)-[*1..${maxHops}]-(target))
         RETURN target.id AS id, length(p) AS hops`,
        "id agtype, hops agtype"
    ).pipe(
        Effect.map(rows => {
            const map = new Map<string, number>();
            for (const row of rows) {
                const id = parseAgtypeId((row as any).id);
                const hops = Number((row as any).hops);
                // Inverse relationship: closer = higher boost (1/hops)
                // 1 hop → 1.0, 2 hops → 0.5, 3 hops → 0.33
                map.set(id, 1 / hops);
            }
            return map;
        }),
        Effect.catchAll(() => Effect.succeed(new Map<string, number>()))
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "getGraphProximityBoost"`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: add getGraphProximityBoost AGE query for hybrid retrieval"
```

### Task 3.2: Integrate hybrid scoring into context-for-file

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`

The idea: after collecting all candidate chunks and before final scoring, use the first file-ref or applies-to match as an "anchor" and boost semantic results that are also graph-close to the anchor.

- [ ] **Step 1: Add import for `getGraphProximityBoost`**

In `packages/api/src/context-for-file/service.ts`:

```typescript
import { getAppliesToForChunks, getChunkById, getConnectionDegrees, getConnectionsForChunks, getGraphProximityBoost, getRequirementsForChunks, listChunks, listCodebases, lookupChunksByFilePath, semanticSearch as semanticSearchRepo } from "@fubbik/db/repository";
```

- [ ] **Step 2: Add a `graph-boosted` bonus in the STRATEGY_BONUS map**

Update the scoring section. After the existing `STRATEGY_BONUS` declaration and the `degreeMap` fetch (from Task 2.3), add:

```typescript
        // Hybrid boost: if we have high-confidence anchors (file-ref or applies-to),
        // boost semantic matches that are also graph-connected to them
        const anchorIds = matchedChunks
            .filter(c => c.matchReason === "file-ref" || c.matchReason === "applies-to")
            .map(c => c.id);
        const semanticIds = matchedChunks
            .filter(c => c.matchReason === "semantic")
            .map(c => c.id);

        let graphBoosts = new Map<string, number>();
        if (anchorIds.length > 0 && semanticIds.length > 0) {
            // Use first anchor as reference point for proximity
            graphBoosts = yield* getGraphProximityBoost(anchorIds[0], semanticIds, 3).pipe(
                Effect.catchAll(() => Effect.succeed(new Map<string, number>())),
            );
        }
```

Then update the scoring loop:

```typescript
        const GRAPH_PROXIMITY_WEIGHT = 5;

        for (const chunk of matchedChunks) {
            const rawRow = chunkRows.get(chunk.id);
            const connectionCount = connCountMap.get(chunk.id) ?? 0;
            const centralityDegree = degreeMap.get(chunk.id) ?? 0;
            const baseScore = rawRow ? scoreChunk(rawRow, connectionCount, centralityDegree) : 0;
            const strategyBonus = STRATEGY_BONUS[chunk.matchReason] ?? 0;
            const proximityBonus = (graphBoosts.get(chunk.id) ?? 0) * GRAPH_PROXIMITY_WEIGHT;
            chunk.score = baseScore + strategyBonus + proximityBonus;
        }
```

- [ ] **Step 3: Run existing tests**

Run: `cd packages/api && pnpm test -- --grep "getContextForFile"`

Expected: PASS — `getGraphProximityBoost` returns empty map in mock.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context-for-file/service.ts
git commit -m "feat: hybrid graph+vector scoring in context-for-file pipeline"
```

### Task 3.3: Apply hybrid boost to duplicate detection

**Files:**
- Modify: `packages/db/src/repository/similarity.ts`

- [ ] **Step 1: Read the current file**

Read `packages/db/src/repository/similarity.ts` to confirm the current `findDuplicatePairs` signature.

- [ ] **Step 2: Add `findDuplicatePairsWithGraphSignal` function**

Add to `packages/db/src/repository/similarity.ts`:

```typescript
import { getSubgraph } from "../age/query";

export function findDuplicatePairsWithGraphSignal(params: {
    chunkIds: string[];
    embeddingThreshold?: number;
    limit?: number;
}) {
    const embeddingThreshold = params.embeddingThreshold ?? 0.85;
    const limit = params.limit ?? 10;

    return findDuplicatePairs({
        chunkIds: params.chunkIds,
        threshold: embeddingThreshold,
        limit: limit * 2
    }).pipe(
        Effect.flatMap(pairs => {
            if (pairs.length === 0) return Effect.succeed([]);
            const allIds = [...new Set(pairs.flatMap(p => [p.idA, p.idB]))];
            return getSubgraph(allIds).pipe(
                Effect.map(edges => {
                    const edgeSet = new Set(
                        edges.map(e => [e.source, e.target].sort().join(":"))
                    );
                    return pairs.map(p => {
                        const pairKey = [p.idA, p.idB].sort().join(":");
                        const graphConnected = edgeSet.has(pairKey);
                        return {
                            ...p,
                            graphConnected,
                            combinedScore: graphConnected
                                ? p.similarity * 1.15
                                : p.similarity
                        };
                    })
                    .sort((a, b) => b.combinedScore - a.combinedScore)
                    .slice(0, limit);
                }),
                Effect.catchAll(() =>
                    Effect.succeed(
                        pairs.slice(0, limit).map(p => ({
                            ...p,
                            graphConnected: false,
                            combinedScore: p.similarity
                        }))
                    )
                )
            );
        })
    );
}
```

- [ ] **Step 3: Export from repository index**

The file already re-exports via `export * from "./similarity"` in `packages/db/src/repository/index.ts`, so no change needed.

- [ ] **Step 4: Run tests**

Run: `cd packages/db && pnpm test`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/repository/similarity.ts
git commit -m "feat: graph-enhanced duplicate detection combining embedding + topology signals"
```

---

## Feature 4: Graph-Based Impact Analysis for Staleness

When a chunk is updated, automatically flag its downstream dependents (chunks connected via `depends_on`, `extends`, `part_of`) as potentially stale.

### Task 4.1: Add `getDownstreamChunks` AGE query

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    checkCircular,
    findShortestPath,
    getConnectionDegrees,
    getDownstreamChunks,
    getGraphProximityBoost,
    getNeighborhood,
    getOrphanChunkIds
} from "./query";

describe("getDownstreamChunks", () => {
    it("finds transitive downstream chunks via directed edges", async () => {
        if (!ageReady) return;
        // In our test graph: A→B→C (all via :connects)
        // Downstream of A should include B and C
        const downstream = await Effect.runPromise(
            getDownstreamChunks(uid("A"), 3)
        );
        expect(downstream).toContain(uid("B"));
        expect(downstream).toContain(uid("C"));
    });

    it("returns empty for leaf chunks with no outgoing edges", async () => {
        if (!ageReady) return;
        // C has no outgoing :connects edges
        const downstream = await Effect.runPromise(
            getDownstreamChunks(uid("C"), 3)
        );
        expect(downstream.length).toBe(0);
    });

    it("returns empty for orphan chunks", async () => {
        if (!ageReady) return;
        const downstream = await Effect.runPromise(
            getDownstreamChunks(uid("orphan"), 3)
        );
        expect(downstream.length).toBe(0);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "getDownstreamChunks"`

Expected: FAIL — not exported.

- [ ] **Step 3: Implement `getDownstreamChunks`**

Add to `packages/db/src/age/query.ts`:

```typescript
export function getDownstreamChunks(chunkId: string, maxHops: number) {
    return cypher(
        `MATCH (source:chunk {id: '${escCypher(chunkId)}'})-[:connects*1..${maxHops}]->(downstream:chunk)
         RETURN DISTINCT downstream.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id))),
        Effect.catchAll(() => Effect.succeed([] as string[]))
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "getDownstreamChunks"`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: add getDownstreamChunks AGE query for impact analysis"
```

### Task 4.2: Create impact-based staleness detector

**Files:**
- Create: `packages/api/src/staleness/detect-impact.ts`

- [ ] **Step 1: Implement the impact detector**

Create `packages/api/src/staleness/detect-impact.ts`:

```typescript
import { createStaleFlag, getDownstreamChunks, getStaleFlags } from "@fubbik/db/repository";
import { Effect } from "effect";

export function flagDownstreamStale(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.gen(function* () {
        const downstreamIds = yield* getDownstreamChunks(updatedChunkId, 3);
        if (downstreamIds.length === 0) return { flagged: 0 };

        const existingFlags = yield* getStaleFlags(userId, { reason: "upstream_changed" });
        const alreadyFlagged = new Set(existingFlags.map(f => f.chunkId));

        let flagged = 0;
        for (const chunkId of downstreamIds) {
            if (alreadyFlagged.has(chunkId)) continue;
            yield* createStaleFlag({
                id: crypto.randomUUID(),
                chunkId,
                reason: "upstream_changed",
                detail: `Upstream chunk "${updatedChunkTitle}" (${updatedChunkId}) was updated`,
                relatedChunkId: updatedChunkId
            });
            flagged++;
        }

        return { flagged };
    });
}
```

- [ ] **Step 2: Re-export from staleness service**

In `packages/api/src/staleness/service.ts`, add:

```typescript
export { flagDownstreamStale } from "./detect-impact";
```

- [ ] **Step 3: Wire into chunk update flow**

In the chunk service (`packages/api/src/chunks/service.ts`), find the `updateChunk` function. After the chunk is successfully updated, add a fire-and-forget impact scan:

```typescript
import { flagDownstreamStale } from "../staleness/detect-impact";

// Inside updateChunk, after the db update succeeds:
// Fire-and-forget: flag downstream chunks as potentially stale
Effect.runPromise(
    flagDownstreamStale(chunkId, updated.title, userId)
).catch(() => {});
```

This should be placed after the existing title/content change detection logic (where embeddings are re-generated).

- [ ] **Step 4: Add route for manual impact scan**

In `packages/api/src/staleness/routes.ts`, add the import at the top:

```typescript
import { flagDownstreamStale } from "./detect-impact";
```

Then add a new endpoint after the existing `scan-age` route:

```typescript
    .post(
        "/chunks/:id/scan-impact",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        flagDownstreamStale(
                            ctx.params.id,
                            ctx.body.title ?? "Unknown",
                            session.user.id
                        )
                    )
                )
            ),
        {
            params: t.Object({ id: t.String() }),
            body: t.Object({ title: t.Optional(t.String()) })
        }
    )
```

- [ ] **Step 5: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/staleness/detect-impact.ts packages/api/src/staleness/service.ts packages/api/src/staleness/routes.ts packages/api/src/chunks/service.ts
git commit -m "feat: graph-based impact analysis flags downstream chunks on update"
```

---

## Feature 5: Community Detection for Auto-Grouping

Use AGE multi-hop queries to find clusters of densely connected chunks. Each cluster is a natural "knowledge community" that could inform tag suggestions, workspace recommendations, or a knowledge-domains view.

### Task 5.1: Add `detectCommunities` AGE query

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    checkCircular,
    detectCommunities,
    findShortestPath,
    getConnectionDegrees,
    getDownstreamChunks,
    getGraphProximityBoost,
    getNeighborhood,
    getOrphanChunkIds
} from "./query";

describe("detectCommunities", () => {
    it("identifies separate clusters in the test graph", async () => {
        if (!ageReady) return;
        const allTestIds = [
            uid("A"), uid("B"), uid("C"),
            uid("center"), uid("spoke1"), uid("spoke2"), uid("spoke3"),
            uid("orphan")
        ];
        const communities = await Effect.runPromise(detectCommunities(allTestIds, 1));
        // Should have at least 2 communities: the chain (A-B-C) and the star (center+spokes)
        // Orphan forms its own singleton community
        expect(communities.length).toBeGreaterThanOrEqual(2);
        // The star cluster should have 4 members
        const starCluster = communities.find(c => c.members.includes(uid("center")));
        expect(starCluster).toBeDefined();
        expect(starCluster!.members).toContain(uid("spoke1"));
    });

    it("returns empty for empty input", async () => {
        if (!ageReady) return;
        const communities = await Effect.runPromise(detectCommunities([], 1));
        expect(communities.length).toBe(0);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "detectCommunities"`

Expected: FAIL — not exported.

- [ ] **Step 3: Implement `detectCommunities`**

AGE doesn't have built-in community detection algorithms, so we use a label-propagation approach in application code, seeded by AGE neighborhood queries.

Add to `packages/db/src/age/query.ts`:

```typescript
export interface Community {
    id: string;
    members: string[];
}

export function detectCommunities(chunkIds: string[], maxHops: number) {
    if (chunkIds.length === 0) return Effect.succeed([] as Community[]);

    const idList = chunkIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (a:chunk)-[e:connects]-(b:chunk)
         WHERE a.id IN [${idList}] AND b.id IN [${idList}]
         RETURN DISTINCT a.id AS source, b.id AS target`,
        "source agtype, target agtype"
    ).pipe(
        Effect.map(rows => {
            // Build adjacency list
            const adj = new Map<string, Set<string>>();
            for (const id of chunkIds) adj.set(id, new Set());
            for (const row of rows) {
                const source = parseAgtypeId((row as any).source);
                const target = parseAgtypeId((row as any).target);
                adj.get(source)?.add(target);
                adj.get(target)?.add(source);
            }

            // Connected-components via BFS
            const visited = new Set<string>();
            const communities: Community[] = [];

            for (const id of chunkIds) {
                if (visited.has(id)) continue;
                const members: string[] = [];
                const queue = [id];
                while (queue.length > 0) {
                    const current = queue.shift()!;
                    if (visited.has(current)) continue;
                    visited.add(current);
                    members.push(current);
                    for (const neighbor of adj.get(current) ?? []) {
                        if (!visited.has(neighbor)) queue.push(neighbor);
                    }
                }
                if (members.length > 1) {
                    communities.push({ id: members[0], members });
                }
            }

            return communities.sort((a, b) => b.members.length - a.members.length);
        })
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "detectCommunities"`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: add community detection via connected-component analysis on AGE graph"
```

### Task 5.2: Expose communities in the graph API

**Files:**
- Modify: `packages/api/src/graph/service.ts`
- Modify: `packages/api/src/graph/routes.ts`

- [ ] **Step 1: Add communities to graph service**

In `packages/api/src/graph/service.ts`:

```typescript
import { detectCommunities, getAllChunksMeta, getAllConnectionsForUser, getAllTagsWithTypes, getChunkCodebaseMappings, getTagTypesForGraph } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserGraph(userId?: string, codebaseId?: string, workspaceId?: string) {
    return Effect.all(
        {
            chunks: getAllChunksMeta(userId, codebaseId, workspaceId),
            connections: getAllConnectionsForUser(userId),
            chunkTags: getAllTagsWithTypes(userId),
            tagTypes: getTagTypesForGraph(userId),
            chunkCodebases: workspaceId ? getChunkCodebaseMappings(userId) : Effect.succeed([] as { chunkId: string; codebaseId: string; codebaseName: string }[])
        },
        { concurrency: "unbounded" }
    ).pipe(
        Effect.flatMap(result => {
            const chunkIds = result.chunks.map(c => c.id);
            return detectCommunities(chunkIds, 1).pipe(
                Effect.catchAll(() => Effect.succeed([])),
                Effect.map(communities => ({
                    ...result,
                    communities
                }))
            );
        })
    );
}
```

- [ ] **Step 2: Add standalone communities endpoint**

In `packages/api/src/graph/routes.ts`, add:

```typescript
import { detectCommunities } from "@fubbik/db/repository";

// After the existing /graph route:
    .get(
        "/graph/communities",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)
                    ),
                    Effect.map(result => result.communities)
                )
            ),
        { query: t.Object({ codebaseId: t.Optional(t.String()), workspaceId: t.Optional(t.String()) }) }
    )
```

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/graph/service.ts packages/api/src/graph/routes.ts
git commit -m "feat: expose knowledge communities via graph API"
```

---

## Feature 6: Graph-Weighted Similarity for Related Chunk Suggestions

Improve the "related chunks" suggestions on chunk detail pages by combining embedding similarity with graph distance — two chunks that are both semantically similar AND graph-connected are much more likely to be genuinely related.

### Task 6.1: Add hybrid similarity function

**Files:**
- Modify: `packages/db/src/repository/semantic.ts`

- [ ] **Step 1: Add `findRelatedChunksHybrid` function**

Add to `packages/db/src/repository/semantic.ts`:

```typescript
import { Effect } from "effect";
import { getNeighborhood } from "../age/query";

export function findRelatedChunksHybrid(
    chunkId: string,
    userId: string,
    k: number,
    graphHops = 2
) {
    return Effect.gen(function* () {
        // Get embedding-based neighbors
        const embeddingNeighbors = yield* findNeighborsByChunkId(chunkId, userId, k * 2);

        // Get graph neighbors
        const graphNeighborIds = yield* getNeighborhood(chunkId, graphHops).pipe(
            Effect.catchAll(() => Effect.succeed([] as string[]))
        );
        const graphSet = new Set(graphNeighborIds);

        // Score: embedding similarity + graph proximity bonus
        const scored = embeddingNeighbors.map(n => {
            const embeddingSimilarity = 1 - n.distance;
            const graphBonus = graphSet.has(n.id) ? 0.15 : 0;
            return {
                ...n,
                embeddingSimilarity,
                graphConnected: graphSet.has(n.id),
                combinedScore: embeddingSimilarity + graphBonus
            };
        });

        return scored
            .sort((a, b) => b.combinedScore - a.combinedScore)
            .slice(0, k);
    });
}
```

- [ ] **Step 2: Run tests**

Run: `cd packages/db && pnpm test`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/semantic.ts
git commit -m "feat: hybrid similarity function combining embeddings with graph proximity"
```

### Task 6.2: Wire hybrid suggestions into chunk detail

**Files:**
- Modify: `packages/api/src/chunks/service.ts` (or wherever related chunk suggestions are served)

- [ ] **Step 1: Find where related chunks are currently fetched**

Search for `findNeighborsByChunkId` usage in the API layer. If it's used in the chunk detail or suggestion endpoints, replace it with `findRelatedChunksHybrid`.

- [ ] **Step 2: Update the import and call**

Replace:
```typescript
import { findNeighborsByChunkId } from "@fubbik/db/repository";
```
With:
```typescript
import { findRelatedChunksHybrid } from "@fubbik/db/repository";
```

And update the call from:
```typescript
findNeighborsByChunkId(chunkId, userId, 5)
```
To:
```typescript
findRelatedChunksHybrid(chunkId, userId, 5)
```

The return type includes additional fields (`embeddingSimilarity`, `graphConnected`, `combinedScore`) which can be used by the frontend to show indicators.

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "feat: use hybrid graph+embedding similarity for related chunk suggestions"
```

---

## Dependency Map

```
Feature 1 (GIN Indexes)     ← No dependencies, do first for performance
Feature 2 (Centrality)      ← Independent
Feature 3 (Hybrid Retrieval) ← Depends on Feature 2 (uses centralityDegree in scoreChunk)
Feature 4 (Impact Analysis)  ← Independent
Feature 5 (Communities)      ← Independent
Feature 6 (Hybrid Similarity) ← Independent
```

**Recommended execution order:** 1 → 2 → 3 → 4 → 5 → 6

Features 4, 5, and 6 can run in parallel after Feature 1 is done.
