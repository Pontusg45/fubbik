# PostgreSQL Graph Extensions Phase 2 — Advanced Utilization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build seven advanced graph-powered features on top of the Phase 1 infrastructure (centrality, communities, hybrid retrieval) to improve context budgeting, knowledge gap detection, tag propagation, path explanation, edge weighting, upstream impact propagation, and redundancy scoring.

**Architecture:** Each feature adds a focused capability via new AGE query functions and/or service-layer logic. Features 1 and 3 improve the context/retrieval pipeline. Features 2 and 7 power knowledge health insights. Features 4 and 5 enhance graph visualization. Feature 6 extends the staleness system. All features are independent except Feature 7 depends on Feature 2.

**Tech Stack:** PostgreSQL (AGE, pgvector), Drizzle ORM, Effect, vitest

---

## File Structure Overview

```
packages/db/src/
├── age/
│   ├── query.ts              # Modify: add path-with-intermediates, bridge detection, redundancy queries
│   └── query.test.ts         # Modify: add tests for new queries
├── schema/
│   └── chunk.ts              # Modify: add weight column to chunkConnection
├── repository/
│   └── connection.ts         # Modify: weight updates on co-access

packages/api/src/
├── context/
│   └── utils.ts              # Modify: community-aware budgetChunks
├── graph/
│   ├── service.ts            # Modify: add bridges + redundancy to graph response
│   └── routes.ts             # Modify: add bridges endpoint
├── chunks/
│   └── service.ts            # Modify: tag suggestions from graph neighbors
├── staleness/
│   └── detect-impact.ts      # Modify: add upstream propagation
├── search/
│   └── service.ts            # Modify: full path explanation in search results
```

---

## Feature 1: Graph-Aware Context Budgeting

The current `budgetChunks` greedily picks highest-score chunks. If the top results are from the same graph community, the token budget is wasted on redundant knowledge. A coverage-maximizing approach penalizes chunks from already-represented communities.

### Task 1.1: Add community-aware budgeting

**Files:**
- Modify: `packages/api/src/context/utils.ts`

- [ ] **Step 1: Add `ScoredChunkWithCommunity` interface and `budgetChunksWithCoverage` function**

In `packages/api/src/context/utils.ts`, add after the existing `budgetChunks` function:

```typescript
export interface ScoredChunkWithCommunity extends ScoredChunk {
    communityId?: string;
}

export function budgetChunksWithCoverage<T extends ScoredChunkWithCommunity>(
    chunks: T[],
    maxTokens: number
): T[] {
    const sorted = [...chunks].sort((a, b) => b.score - a.score);
    const selected: T[] = [];
    let usedTokens = estimateTokens("# Project Context\n\n");
    const communityCounts = new Map<string, number>();

    for (const chunk of sorted) {
        const chunkText = formatChunkText(chunk);
        const tokens = estimateTokens(chunkText);
        if (usedTokens + tokens > maxTokens) continue;

        // Penalize chunks from already-represented communities
        const cid = chunk.communityId;
        if (cid) {
            const count = communityCounts.get(cid) ?? 0;
            // After 2 chunks from the same community, apply diminishing returns
            if (count >= 2) {
                const penalty = count * 3;
                if (chunk.score - penalty <= 0) continue;
            }
            communityCounts.set(cid, count + 1);
        }

        selected.push(chunk);
        usedTokens += tokens;
    }
    return selected;
}
```

- [ ] **Step 2: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS — the new function is additive.

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/context/utils.ts
git commit -m "feat: community-aware context budgeting with diminishing returns"
```

### Task 1.2: Wire community-aware budgeting into resolvers

**Files:**
- Modify: `packages/api/src/context/resolvers.ts`

- [ ] **Step 1: Import `detectCommunities` and `budgetChunksWithCoverage`**

In `packages/api/src/context/resolvers.ts`, add to the imports:

```typescript
import { detectCommunities } from "@fubbik/db/repository";
import { budgetChunksWithCoverage, type ScoredChunkWithCommunity } from "./utils";
```

- [ ] **Step 2: Find the caller of `budgetChunks` in resolvers.ts**

Search for `budgetChunks` usage in the file. It's likely in the context export functions. Replace those calls with `budgetChunksWithCoverage` after annotating chunks with their community ID.

Before the `budgetChunks` call, add community detection:

```typescript
// Annotate chunks with community IDs for coverage-aware budgeting
const chunkIds = enrichedChunks.map(c => c.id);
const communities = await Effect.runPromise(
    detectCommunities(chunkIds, 1).pipe(
        Effect.catchAll(() => Effect.succeed([]))
    )
);
const chunkToCommunity = new Map<string, string>();
for (const community of communities) {
    for (const memberId of community.members) {
        chunkToCommunity.set(memberId, community.id);
    }
}
const chunksWithCommunity = enrichedChunks.map(c => ({
    ...c,
    communityId: chunkToCommunity.get(c.id)
}));
```

Then replace `budgetChunks(enrichedChunks, maxTokens)` with `budgetChunksWithCoverage(chunksWithCommunity, maxTokens)`.

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context/resolvers.ts
git commit -m "feat: wire community-aware budgeting into context resolvers"
```

---

## Feature 2: Bridge Detection (Knowledge Bottlenecks)

Find articulation points — chunks whose removal would split a community into disconnected parts. These are knowledge bottlenecks that should be flagged in the health dashboard.

### Task 2.1: Add `findBridgeChunks` AGE query

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    // ... existing imports ...
    findBridgeChunks,
} from "./query";

describe("findBridgeChunks", () => {
    it("identifies B as a bridge in the A→B→C chain", async () => {
        if (!ageReady) return;
        const bridges = await Effect.runPromise(
            findBridgeChunks([uid("A"), uid("B"), uid("C")])
        );
        // B connects A and C — removing it disconnects the chain
        expect(bridges).toContain(uid("B"));
        // A and C are leaf nodes, not bridges
        expect(bridges).not.toContain(uid("A"));
        expect(bridges).not.toContain(uid("C"));
    });

    it("identifies center as a bridge in star topology", async () => {
        if (!ageReady) return;
        const bridges = await Effect.runPromise(
            findBridgeChunks([uid("center"), uid("spoke1"), uid("spoke2"), uid("spoke3")])
        );
        expect(bridges).toContain(uid("center"));
    });

    it("returns empty for empty input", async () => {
        if (!ageReady) return;
        const bridges = await Effect.runPromise(findBridgeChunks([]));
        expect(bridges).toHaveLength(0);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "findBridgeChunks"`

Expected: FAIL — not exported.

- [ ] **Step 3: Implement `findBridgeChunks`**

AGE doesn't have built-in articulation point detection. We implement it in application code using the adjacency data from a Cypher query, applying the classic DFS-based algorithm.

Add to `packages/db/src/age/query.ts`:

```typescript
export function findBridgeChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([] as string[]);

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

            // Find articulation points via DFS
            const visited = new Set<string>();
            const disc = new Map<string, number>();
            const low = new Map<string, number>();
            const parent = new Map<string, string | null>();
            const bridges: string[] = [];
            let timer = 0;

            function dfs(u: string) {
                visited.add(u);
                disc.set(u, timer);
                low.set(u, timer);
                timer++;
                let children = 0;
                let isArticulation = false;

                for (const v of adj.get(u) ?? []) {
                    if (!visited.has(v)) {
                        children++;
                        parent.set(v, u);
                        dfs(v);
                        low.set(u, Math.min(low.get(u)!, low.get(v)!));
                        // u is an articulation point if:
                        // 1) u is root and has 2+ children
                        // 2) u is not root and low[v] >= disc[u]
                        if (parent.get(u) === null && children > 1) isArticulation = true;
                        if (parent.get(u) !== null && low.get(v)! >= disc.get(u)!) isArticulation = true;
                    } else if (v !== parent.get(u)) {
                        low.set(u, Math.min(low.get(u)!, disc.get(v)!));
                    }
                }
                if (isArticulation) bridges.push(u);
            }

            for (const id of chunkIds) {
                if (!visited.has(id) && (adj.get(id)?.size ?? 0) > 0) {
                    parent.set(id, null);
                    dfs(id);
                }
            }

            return bridges;
        })
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "findBridgeChunks"`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: articulation point detection for knowledge bottleneck identification"
```

### Task 2.2: Expose bridges in graph API

**Files:**
- Modify: `packages/api/src/graph/service.ts`
- Modify: `packages/api/src/graph/routes.ts`

- [ ] **Step 1: Add bridges to graph service**

In `packages/api/src/graph/service.ts`, update the import:

```typescript
import { detectCommunities, findBridgeChunks, getAllChunksMeta, getAllConnectionsForUser, getAllTagsWithTypes, getChunkCodebaseMappings, getTagTypesForGraph } from "@fubbik/db/repository";
```

Update the `getUserGraph` function to also compute bridges:

```typescript
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
            return Effect.all({
                communities: detectCommunities(chunkIds, 1).pipe(
                    Effect.catchAll(() => Effect.succeed([]))
                ),
                bridges: findBridgeChunks(chunkIds).pipe(
                    Effect.catchAll(() => Effect.succeed([] as string[]))
                )
            }).pipe(
                Effect.map(({ communities, bridges }) => ({
                    ...result,
                    communities,
                    bridges
                }))
            );
        })
    );
}
```

- [ ] **Step 2: Add bridges endpoint to routes**

In `packages/api/src/graph/routes.ts`, add after the communities endpoint:

```typescript
.get(
    "/graph/bridges",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)
                ),
                Effect.map(result => result.bridges)
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
git commit -m "feat: expose knowledge bottleneck (bridge) chunks via graph API"
```

---

## Feature 3: Graph-Based Tag Propagation

When a chunk is connected to several chunks that share a tag, suggest that tag for the chunk. Reduces manual tagging effort and improves consistency.

### Task 3.1: Add `getNeighborTagFrequencies` service function

**Files:**
- Create: `packages/api/src/chunks/tag-suggestions.ts`

- [ ] **Step 1: Implement tag frequency analysis from graph neighbors**

Create `packages/api/src/chunks/tag-suggestions.ts`:

```typescript
import { getNeighborhood, getTagsForChunks, getTagsForChunk } from "@fubbik/db/repository";
import { Effect } from "effect";

export interface TagSuggestion {
    tagId: string;
    tagName: string;
    frequency: number;
    neighborCount: number;
}

export function suggestTagsFromGraph(chunkId: string, maxHops = 1, minFrequency = 0.5) {
    return Effect.gen(function* () {
        const neighborIds = yield* getNeighborhood(chunkId, maxHops).pipe(
            Effect.catchAll(() => Effect.succeed([] as string[]))
        );
        if (neighborIds.length === 0) return [] as TagSuggestion[];

        const existingTags = yield* getTagsForChunk(chunkId).pipe(
            Effect.catchAll(() => Effect.succeed([]))
        );
        const existingTagIds = new Set(existingTags.map(t => t.id));

        const neighborTags = yield* getTagsForChunks(neighborIds).pipe(
            Effect.catchAll(() => Effect.succeed([]))
        );

        // Count tag frequency across neighbors
        const tagCounts = new Map<string, { tagName: string; count: number }>();
        for (const nt of neighborTags) {
            if (existingTagIds.has(nt.tagId)) continue;
            const existing = tagCounts.get(nt.tagId);
            if (existing) {
                existing.count++;
            } else {
                tagCounts.set(nt.tagId, { tagName: nt.tagName, count: 1 });
            }
        }

        const suggestions: TagSuggestion[] = [];
        for (const [tagId, { tagName, count }] of tagCounts) {
            const frequency = count / neighborIds.length;
            if (frequency >= minFrequency) {
                suggestions.push({ tagId, tagName, frequency, neighborCount: neighborIds.length });
            }
        }

        return suggestions.sort((a, b) => b.frequency - a.frequency);
    });
}
```

- [ ] **Step 2: Add route for tag suggestions**

Find the chunk routes file (`packages/api/src/chunks/routes.ts`) and add a new endpoint:

```typescript
import { suggestTagsFromGraph } from "./tag-suggestions";

// Add in the Elysia chain:
.get(
    "/chunks/:id/tag-suggestions",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => suggestTagsFromGraph(ctx.params.id))
            )
        ),
    { params: t.Object({ id: t.String() }) }
)
```

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/chunks/tag-suggestions.ts packages/api/src/chunks/routes.ts
git commit -m "feat: graph-based tag suggestions from neighbor frequency analysis"
```

---

## Feature 4: Weighted Edges from Co-Access Patterns

Add a `weight` column to chunk connections. Increment weight when two chunks are accessed in the same context request. Heavier edges mean stronger relationships.

### Task 4.1: Add weight column to chunkConnection schema

**Files:**
- Modify: `packages/db/src/schema/chunk.ts`
- Create: `packages/db/src/migrations/0003_connection_weight.sql`

- [ ] **Step 1: Add weight column to schema**

In `packages/db/src/schema/chunk.ts`, add the `weight` column to the `chunkConnection` table definition, after the `reviewedAt` field:

```typescript
        weight: integer("weight").notNull().default(1)
```

- [ ] **Step 2: Create migration**

Create `packages/db/src/migrations/0003_connection_weight.sql`:

```sql
-- Add weight column to chunk_connection for co-access pattern tracking
ALTER TABLE chunk_connection ADD COLUMN IF NOT EXISTS weight integer NOT NULL DEFAULT 1;
```

- [ ] **Step 3: Update the Drizzle migration journal**

In `packages/db/src/migrations/meta/_journal.json`, add:

```json
{
  "idx": 3,
  "version": "7",
  "when": 1715212800000,
  "tag": "0003_connection_weight",
  "breakpoints": true
}
```

- [ ] **Step 4: Run migration**

```bash
psql $DATABASE_URL -f packages/db/src/migrations/0003_connection_weight.sql
```

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/schema/chunk.ts packages/db/src/migrations/0003_connection_weight.sql packages/db/src/migrations/meta/_journal.json
git commit -m "feat: add weight column to chunk_connection for co-access tracking"
```

### Task 4.2: Add weight increment function

**Files:**
- Modify: `packages/db/src/repository/connection.ts`

- [ ] **Step 1: Add `incrementConnectionWeights` function**

In `packages/db/src/repository/connection.ts`, add:

```typescript
import { eq, inArray, or, sql } from "drizzle-orm";

export function incrementConnectionWeights(chunkIds: string[]) {
    if (chunkIds.length < 2) return dbEffect(() => Promise.resolve(0));

    return dbEffect(async () => {
        const result = await db
            .update(chunkConnection)
            .set({ weight: sql`${chunkConnection.weight} + 1` })
            .where(
                and(
                    inArray(chunkConnection.sourceId, chunkIds),
                    inArray(chunkConnection.targetId, chunkIds)
                )
            );
        return result.rowCount ?? 0;
    });
}
```

Make sure `and` is imported from `drizzle-orm` (it likely already is — check the existing imports).

- [ ] **Step 2: Run tests**

Run: `cd packages/db && pnpm test`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/connection.ts
git commit -m "feat: add connection weight increment for co-access tracking"
```

### Task 4.3: Wire weight increment into context retrieval

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`

- [ ] **Step 1: Add fire-and-forget weight increment after context retrieval**

In `packages/api/src/context-for-file/service.ts`, add the import:

```typescript
import { getAppliesToForChunks, getChunkById, getConnectionDegrees, getConnectionsForChunks, getGraphProximityBoost, getRequirementsForChunks, incrementConnectionWeights, listChunks, listCodebases, lookupChunksByFilePath, semanticSearch as semanticSearchRepo } from "@fubbik/db/repository";
```

At the end of the `getContextForFile` function, just before `return { chunks: matchedChunks, requirements }`, add:

```typescript
        // Fire-and-forget: increment edge weights for co-accessed chunks
        if (matchedChunks.length >= 2) {
            const coAccessedIds = matchedChunks.slice(0, 10).map(c => c.id);
            Effect.runPromise(
                incrementConnectionWeights(coAccessedIds).pipe(
                    Effect.catchAll(() => Effect.succeed(0))
                )
            ).catch(() => {});
        }
```

- [ ] **Step 2: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/context-for-file/service.ts
git commit -m "feat: increment connection weights on co-access in context retrieval"
```

---

## Feature 5: Full Path Explanation

Replace the current `findShortestPath` (which only confirms reachability) with a version that returns intermediate nodes and relation types. Powers "why are these related?" in the UI and search.

### Task 5.1: Add `findShortestPathWithDetails` AGE query

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    // ... existing imports ...
    findShortestPathWithDetails,
} from "./query";

describe("findShortestPathWithDetails", () => {
    it("returns intermediate nodes in A→B→C path", async () => {
        if (!ageReady) return;
        const path = await Effect.runPromise(
            findShortestPathWithDetails(uid("A"), uid("C"))
        );
        expect(path).not.toBeNull();
        expect(path!.nodes).toHaveLength(3);
        expect(path!.nodes[0]).toBe(uid("A"));
        expect(path!.nodes[1]).toBe(uid("B"));
        expect(path!.nodes[2]).toBe(uid("C"));
        expect(path!.edges).toHaveLength(2);
    });

    it("returns direct path for adjacent nodes", async () => {
        if (!ageReady) return;
        const path = await Effect.runPromise(
            findShortestPathWithDetails(uid("A"), uid("B"))
        );
        expect(path).not.toBeNull();
        expect(path!.nodes).toHaveLength(2);
        expect(path!.edges).toHaveLength(1);
        expect(path!.edges[0]!.relation).toBe("related_to");
    });

    it("returns null for unreachable nodes", async () => {
        if (!ageReady) return;
        const path = await Effect.runPromise(
            findShortestPathWithDetails(uid("A"), uid("orphan"))
        );
        expect(path).toBeNull();
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "findShortestPathWithDetails"`

Expected: FAIL — not exported.

- [ ] **Step 3: Implement `findShortestPathWithDetails`**

Add to `packages/db/src/age/query.ts`:

```typescript
export interface PathEdge {
    source: string;
    target: string;
    relation: string;
}

export interface DetailedPath {
    nodes: string[];
    edges: PathEdge[];
    hops: number;
}

export function findShortestPathWithDetails(chunkIdA: string, chunkIdB: string) {
    // Use BFS over the graph to find the actual shortest path with intermediates.
    // AGE 1.x doesn't support path extraction, so we:
    // 1. Get the neighborhood of A up to 10 hops
    // 2. Get all edges in that neighborhood
    // 3. BFS in application code to find the actual path
    const escapedA = escCypher(chunkIdA);
    const escapedB = escCypher(chunkIdB);

    return cypher(
        `MATCH (a:chunk {id: '${escapedA}'})-[*1..10]-(b:chunk {id: '${escapedB}'})
         RETURN b.id AS found LIMIT 1`,
        "found agtype"
    ).pipe(
        Effect.flatMap(rows => {
            if (rows.length === 0) return Effect.succeed(null as DetailedPath | null);

            // Path exists — now get all edges to reconstruct it
            return cypher(
                `MATCH (a:chunk {id: '${escapedA}'})-[*1..10]-(reachable:chunk)
                 WITH collect(DISTINCT reachable.id) AS reachable_ids
                 MATCH (x:chunk)-[e:connects]->(y:chunk)
                 WHERE x.id IN reachable_ids AND y.id IN reachable_ids
                 RETURN x.id AS source, y.id AS target, e.relation AS relation`,
                "source agtype, target agtype, relation agtype"
            ).pipe(
                Effect.map(edgeRows => {
                    // Build adjacency for BFS
                    const adj = new Map<string, Array<{ neighbor: string; relation: string }>>();
                    const addEdge = (from: string, to: string, rel: string) => {
                        if (!adj.has(from)) adj.set(from, []);
                        adj.get(from)!.push({ neighbor: to, relation: rel });
                    };
                    for (const row of edgeRows) {
                        const s = parseAgtypeId((row as any).source);
                        const t = parseAgtypeId((row as any).target);
                        const r = parseAgtypeId((row as any).relation);
                        addEdge(s, t, r);
                        addEdge(t, s, r);
                    }

                    // BFS from A to B
                    const visited = new Set<string>();
                    const parentMap = new Map<string, { from: string; relation: string } | null>();
                    const queue = [chunkIdA];
                    visited.add(chunkIdA);
                    parentMap.set(chunkIdA, null);

                    while (queue.length > 0) {
                        const current = queue.shift()!;
                        if (current === chunkIdB) break;
                        for (const { neighbor, relation } of adj.get(current) ?? []) {
                            if (!visited.has(neighbor)) {
                                visited.add(neighbor);
                                parentMap.set(neighbor, { from: current, relation });
                                queue.push(neighbor);
                            }
                        }
                    }

                    if (!parentMap.has(chunkIdB)) return null;

                    // Reconstruct path
                    const nodes: string[] = [];
                    const edges: PathEdge[] = [];
                    let current: string | null = chunkIdB;
                    while (current !== null) {
                        nodes.unshift(current);
                        const p = parentMap.get(current);
                        if (p) {
                            edges.unshift({ source: p.from, target: current, relation: p.relation });
                            current = p.from;
                        } else {
                            current = null;
                        }
                    }

                    return { nodes, edges, hops: edges.length } as DetailedPath;
                })
            );
        }),
        Effect.catchAll(() => Effect.succeed(null as DetailedPath | null))
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "findShortestPathWithDetails"`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: full shortest-path with intermediate nodes and relation types"
```

### Task 5.2: Wire detailed path into search service

**Files:**
- Modify: `packages/api/src/search/service.ts`

- [ ] **Step 1: Import and use `findShortestPathWithDetails`**

In `packages/api/src/search/service.ts`, add `findShortestPathWithDetails` to the import from `@fubbik/db/repository`.

Find the `"path"` clause handler (around line 101). Replace the `findShortestPath` call:

```typescript
else if (clause.field === "path") {
    const [chunkA, chunkB] = clause.value.split(",").map(s => s.trim());
    if (chunkA && chunkB) {
        const detailedPath = yield* findShortestPathWithDetails(chunkA, chunkB).pipe(
            Effect.orElse(() => Effect.succeed(null))
        );
        const ids = detailedPath?.nodes ?? [];
        graphIds = graphIds ? graphIds.filter(id => ids.includes(id)) : ids;
        graphMeta = {
            type: "path",
            pathChunks: ids,
            pathEdges: detailedPath?.edges ?? [],
            hops: detailedPath?.hops ?? 0
        };
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/search/service.ts
git commit -m "feat: return full path with intermediates and relations in search results"
```

---

## Feature 6: Upstream Impact Propagation

Extend the existing downstream staleness detection to also propagate upstream — if a chunk `depends_on` a stale chunk, it is itself at risk.

### Task 6.1: Add `getUpstreamChunks` AGE query

**Files:**
- Modify: `packages/db/src/age/query.ts`
- Modify: `packages/db/src/age/query.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `packages/db/src/age/query.test.ts`:

```typescript
import {
    // ... existing imports ...
    getUpstreamChunks,
} from "./query";

describe("getUpstreamChunks", () => {
    it("finds chunks that depend on the given chunk (reverse traversal)", async () => {
        if (!ageReady) return;
        // In test graph: A→B→C (directed :connects edges)
        // Upstream of C (chunks that connect TO C) = B
        const upstream = await Effect.runPromise(
            getUpstreamChunks(uid("C"), 3)
        );
        expect(upstream).toContain(uid("B"));
        expect(upstream).toContain(uid("A"));
    });

    it("returns empty for root chunks with no incoming edges", async () => {
        if (!ageReady) return;
        const upstream = await Effect.runPromise(
            getUpstreamChunks(uid("A"), 3)
        );
        expect(upstream.length).toBe(0);
    });

    it("returns empty for orphan chunks", async () => {
        if (!ageReady) return;
        const upstream = await Effect.runPromise(
            getUpstreamChunks(uid("orphan"), 3)
        );
        expect(upstream.length).toBe(0);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/db && pnpm test -- --grep "getUpstreamChunks"`

Expected: FAIL — not exported.

- [ ] **Step 3: Implement `getUpstreamChunks`**

Add to `packages/db/src/age/query.ts`:

```typescript
export function getUpstreamChunks(chunkId: string, maxHops: number) {
    return cypher(
        `MATCH (upstream:chunk)-[:connects*1..${maxHops}]->(target:chunk {id: '${escCypher(chunkId)}'})
         RETURN DISTINCT upstream.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id))),
        Effect.catchAll(() => Effect.succeed([] as string[]))
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/db && pnpm test -- --grep "getUpstreamChunks"`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/age/query.ts packages/db/src/age/query.test.ts
git commit -m "feat: add getUpstreamChunks AGE query for reverse impact traversal"
```

### Task 6.2: Extend impact detector with upstream propagation

**Files:**
- Modify: `packages/api/src/staleness/detect-impact.ts`

- [ ] **Step 1: Add `flagUpstreamStale` function**

In `packages/api/src/staleness/detect-impact.ts`, add the import and function:

```typescript
import { createStaleFlag, getDownstreamChunks, getUpstreamChunks, getStaleFlags } from "@fubbik/db/repository";
import { Effect } from "effect";

// ... existing flagDownstreamStale function ...

export function flagUpstreamStale(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.gen(function* () {
        const upstreamIds = yield* getUpstreamChunks(updatedChunkId, 3);
        if (upstreamIds.length === 0) return { flagged: 0 };

        const existingFlags = yield* getStaleFlags(userId, { reason: "downstream_changed" });
        const alreadyFlagged = new Set(existingFlags.map(f => f.chunkId));

        let flagged = 0;
        for (const chunkId of upstreamIds) {
            if (alreadyFlagged.has(chunkId)) continue;
            yield* createStaleFlag({
                id: crypto.randomUUID(),
                chunkId,
                reason: "downstream_changed",
                detail: `Downstream chunk "${updatedChunkTitle}" (${updatedChunkId}) was updated`,
                relatedChunkId: updatedChunkId
            });
            flagged++;
        }

        return { flagged };
    });
}

export function flagBidirectionalImpact(updatedChunkId: string, updatedChunkTitle: string, userId: string) {
    return Effect.all({
        downstream: flagDownstreamStale(updatedChunkId, updatedChunkTitle, userId),
        upstream: flagUpstreamStale(updatedChunkId, updatedChunkTitle, userId)
    }).pipe(
        Effect.map(({ downstream, upstream }) => ({
            flagged: downstream.flagged + upstream.flagged
        }))
    );
}
```

- [ ] **Step 2: Update the chunk mutation to use bidirectional impact**

In `packages/api/src/chunks/chunk-mutations.ts` (or wherever `flagDownstreamStale` is called), replace it with `flagBidirectionalImpact`:

```typescript
import { flagBidirectionalImpact } from "../staleness/detect-impact";

// Replace:
//   flagDownstreamStale(chunkId, updated.title, userId)
// With:
//   flagBidirectionalImpact(chunkId, updated.title, userId)
```

- [ ] **Step 3: Re-export from staleness service**

In `packages/api/src/staleness/service.ts`, add:

```typescript
export { flagDownstreamStale, flagUpstreamStale, flagBidirectionalImpact } from "./detect-impact";
```

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm test`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/staleness/detect-impact.ts packages/api/src/staleness/service.ts packages/api/src/chunks/chunk-mutations.ts
git commit -m "feat: bidirectional impact propagation flags both upstream and downstream chunks"
```

---

## Feature 7: Redundancy Scoring per Community

For each community, compute how much its members overlap semantically. High-redundancy communities suggest chunks that should be merged. Low-redundancy with many members suggests well-structured knowledge.

### Task 7.1: Add `computeCommunityRedundancy` function

**Files:**
- Create: `packages/api/src/graph/community-analysis.ts`

- [ ] **Step 1: Implement redundancy scoring**

Create `packages/api/src/graph/community-analysis.ts`:

```typescript
import { findDuplicatePairs, type Community } from "@fubbik/db/repository";
import { Effect } from "effect";

export interface CommunityRedundancy {
    communityId: string;
    memberCount: number;
    avgPairwiseSimilarity: number;
    maxPairwiseSimilarity: number;
    redundancyLevel: "low" | "medium" | "high";
    mergeCandidates: Array<{ idA: string; idB: string; similarity: number }>;
}

export function computeCommunityRedundancy(communities: Community[]) {
    return Effect.gen(function* () {
        const results: CommunityRedundancy[] = [];

        for (const community of communities) {
            if (community.members.length < 2) continue;

            const pairs = yield* findDuplicatePairs({
                chunkIds: community.members,
                threshold: 0.5,
                limit: 50
            }).pipe(
                Effect.catchAll(() => Effect.succeed([]))
            );

            if (pairs.length === 0) {
                results.push({
                    communityId: community.id,
                    memberCount: community.members.length,
                    avgPairwiseSimilarity: 0,
                    maxPairwiseSimilarity: 0,
                    redundancyLevel: "low",
                    mergeCandidates: []
                });
                continue;
            }

            const avgSimilarity = pairs.reduce((sum, p) => sum + p.similarity, 0) / pairs.length;
            const maxSimilarity = Math.max(...pairs.map(p => p.similarity));
            const redundancyLevel = avgSimilarity > 0.8 ? "high" : avgSimilarity > 0.6 ? "medium" : "low";
            const mergeCandidates = pairs
                .filter(p => p.similarity > 0.8)
                .map(p => ({ idA: p.idA, idB: p.idB, similarity: p.similarity }));

            results.push({
                communityId: community.id,
                memberCount: community.members.length,
                avgPairwiseSimilarity: avgSimilarity,
                maxPairwiseSimilarity: maxSimilarity,
                redundancyLevel,
                mergeCandidates
            });
        }

        return results.sort((a, b) => b.avgPairwiseSimilarity - a.avgPairwiseSimilarity);
    });
}
```

- [ ] **Step 2: Add route for redundancy analysis**

In `packages/api/src/graph/routes.ts`, add:

```typescript
import { computeCommunityRedundancy } from "./community-analysis";

// Add to the Elysia chain:
.get(
    "/graph/redundancy",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    graphService.getUserGraph(session.user.id, ctx.query.codebaseId, ctx.query.workspaceId)
                ),
                Effect.flatMap(result => computeCommunityRedundancy(result.communities))
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
git add packages/api/src/graph/community-analysis.ts packages/api/src/graph/routes.ts
git commit -m "feat: community redundancy scoring with merge candidate detection"
```

---

## Dependency Map

```
Feature 1 (Graph-Aware Budgeting)     ← Uses detectCommunities (Phase 1)
Feature 2 (Bridge Detection)          ← Independent
Feature 3 (Tag Propagation)           ← Uses getNeighborhood (Phase 1)
Feature 4 (Weighted Edges)            ← Independent (schema change)
Feature 5 (Path Explanation)          ← Independent
Feature 6 (Upstream Impact)           ← Extends detect-impact.ts (Phase 1)
Feature 7 (Redundancy Scoring)        ← Uses detectCommunities + findDuplicatePairs (Phase 1)
```

**Recommended execution order:** 2 → 5 → 6 → 1 → 3 → 4 → 7

Features 2, 5, 6 can run in parallel (different files). Features 1, 3, 4 can run in parallel after those. Feature 7 last (depends on community data from the graph service).
