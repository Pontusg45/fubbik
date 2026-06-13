# Graph Redesign: Cosmic Archipelago — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current dump-everything graph with a three-level semantic zoom model (Overview/Archipelago → Neighborhood/Cosmic Map →
Detail) that handles 500-2000 chunks without visual overload.

**Architecture:** Islands form from tag-type grouping (client-side). Overview renders ~5-20 island nodes. Clicking an island zooms to a
neighborhood view centered on the most-connected chunk, showing 1-hop cards and 2-hop dots. Detail panel slides in on chunk click. React
Flow stays as the rendering foundation. No new backend work.

**Tech Stack:** React Flow (@xyflow/react), TanStack Router (URL state), TanStack Query (data fetching), Tailwind CSS, vitest (unit tests)

**Spec:** `docs/superpowers/specs/2026-05-20-graph-redesign-design.md`

---

## File Structure

### New Files

| File                                                      | Responsibility                                    |
| --------------------------------------------------------- | ------------------------------------------------- |
| `apps/web/src/features/graph/island-formation.ts`         | Pure function: chunks + tags → island groups      |
| `apps/web/src/features/graph/island-formation.test.ts`    | Tests for island formation logic                  |
| `apps/web/src/features/graph/island-layout.ts`            | Lightweight force simulation for island positions |
| `apps/web/src/features/graph/island-layout.test.ts`       | Tests for island layout                           |
| `apps/web/src/features/graph/neighborhood-layout.ts`      | Radial placement around focus chunk               |
| `apps/web/src/features/graph/neighborhood-layout.test.ts` | Tests for neighborhood layout                     |
| `apps/web/src/features/graph/use-graph-zoom.ts`           | Zoom level state machine + transitions            |
| `apps/web/src/features/graph/typed-edge.tsx`              | Relation-type-aware edge component                |
| `apps/web/src/features/graph/graph-island-node.tsx`       | Island node component (overview level)            |
| `apps/web/src/features/graph/graph-chunk-card.tsx`        | Chunk card component (neighborhood level)         |
| `apps/web/src/features/graph/graph-chunk-dot.tsx`         | 2-hop dot component (neighborhood level)          |

### Modified Files

| File                                                    | Changes                                                                                |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `apps/web/src/features/graph/use-graph-state.ts`        | Simplify: remove explore mode, layout algorithm toggle, cluster state, timeline cutoff |
| `apps/web/src/features/graph/use-graph-data.ts`         | Add island computation to data pipeline                                                |
| `apps/web/src/features/graph/use-graph-nodes.ts`        | Rewrite: zoom-level-aware node/edge building                                           |
| `apps/web/src/features/graph/use-graph-styling.ts`      | Rewrite: zoom-level-aware opacity/glow                                                 |
| `apps/web/src/features/graph/use-graph-interactions.ts` | Simplify: keep search + path, remove focus mode                                        |
| `apps/web/src/features/graph/graph-view.tsx`            | Major rewrite: zoom orchestration, breadcrumbs                                         |
| `apps/web/src/routes/graph.tsx`                         | Update search params: add `zoomLevel`, `focusChunk`, `island`                          |
| `apps/web/src/features/graph/graph-utils.ts`            | Keep `findShortestPath`, `getNodesWithinHops`, `getMostConnected`                      |

### Deleted Files

| File                                                  | Reason                                                           |
| ----------------------------------------------------- | ---------------------------------------------------------------- |
| `apps/web/src/features/graph/force-layout.ts`         | Replaced by island-layout.ts                                     |
| `apps/web/src/features/graph/quadtree.ts`             | Not needed for 5-20 node simulation                              |
| `apps/web/src/features/graph/layout.worker.ts`        | No heavy layout to offload                                       |
| `apps/web/src/features/graph/layout-cache.ts`         | New layout is fast enough without caching                        |
| `apps/web/src/features/graph/cluster-strategy.ts`     | Replaced by island formation                                     |
| `apps/web/src/features/graph/graph-node.tsx`          | Replaced by graph-chunk-card.tsx                                 |
| `apps/web/src/features/graph/graph-group-node.tsx`    | Replaced by graph-island-node.tsx                                |
| `apps/web/src/features/graph/graph-cluster-node.tsx`  | Replaced by graph-island-node.tsx                                |
| `apps/web/src/features/graph/floating-edge.tsx`       | Replaced by typed-edge.tsx                                       |
| `apps/web/src/features/graph/use-graph-layout.ts`     | Layout logic moves into use-graph-nodes.ts                       |
| `apps/web/src/features/graph/use-graph-grouping.ts`   | Grouping logic replaced by island formation in use-graph-data.ts |
| `apps/web/src/features/graph/layouts.ts`              | Hierarchical/radial top-level layouts removed                    |
| `apps/web/src/features/graph/graph-filter-form.tsx`   | Filter dialog replaced by simpler island/tag-type picker         |
| `apps/web/src/features/graph/graph-filter-dialog.tsx` | Same — replaced                                                  |

---

## Task 1: Island Formation Logic

Pure function that groups chunks into islands based on a selected tag type. No React dependencies — fully testable.

**Files:**

- Create: `apps/web/src/features/graph/island-formation.ts`
- Create: `apps/web/src/features/graph/island-formation.test.ts`

- [ ] **Step 1: Write failing tests for island formation**

```ts
// apps/web/src/features/graph/island-formation.test.ts
import { describe, expect, it } from "vitest";
import { formIslands, type IslandFormationInput } from "./island-formation";

const makeInput = (overrides: Partial<IslandFormationInput> = {}): IslandFormationInput => ({
    chunks: [
        { id: "c1", type: "note" },
        { id: "c2", type: "guide" },
        { id: "c3", type: "note" },
        { id: "c4", type: "reference" },
        { id: "c5", type: "note" }
    ],
    connections: [
        { sourceId: "c1", targetId: "c2", relation: "depends_on" },
        { sourceId: "c2", targetId: "c3", relation: "part_of" },
        { sourceId: "c4", targetId: "c5", relation: "references" }
    ],
    chunkTags: [
        { chunkId: "c1", tagTypeId: "tt1", tagName: "auth" },
        { chunkId: "c2", tagTypeId: "tt1", tagName: "auth" },
        { chunkId: "c3", tagTypeId: "tt1", tagName: "api" },
        { chunkId: "c4", tagTypeId: "tt1", tagName: "api" }
        // c5 has no tag under tt1
    ],
    groupingTagTypeId: "tt1",
    ...overrides
});

describe("formIslands", () => {
    it("groups chunks by tag under the selected tag type", () => {
        const result = formIslands(makeInput());
        expect(result.islands).toHaveLength(3); // auth, api, ungrouped
        const auth = result.islands.find(i => i.name === "auth");
        expect(auth?.chunkIds).toEqual(["c1", "c2"]);
        const api = result.islands.find(i => i.name === "api");
        expect(api?.chunkIds).toEqual(["c3", "c4"]);
    });

    it("puts untagged chunks in an ungrouped island", () => {
        const result = formIslands(makeInput());
        const ungrouped = result.islands.find(i => i.id === "ungrouped");
        expect(ungrouped?.chunkIds).toEqual(["c5"]);
    });

    it("computes bridge connections between islands", () => {
        const result = formIslands(makeInput());
        const bridge = result.bridges.find(
            b => (b.fromIslandId === "auth" && b.toIslandId === "api") || (b.fromIslandId === "api" && b.toIslandId === "auth")
        );
        expect(bridge).toBeDefined();
        expect(bridge!.count).toBe(1); // c2→c3
    });

    it("does not create an island for single-chunk groups when skipSingletons is true", () => {
        const input = makeInput({
            chunks: [{ id: "c1", type: "note" }],
            connections: [],
            chunkTags: [{ chunkId: "c1", tagTypeId: "tt1", tagName: "lonely" }]
        });
        const result = formIslands(input);
        const lonely = result.islands.find(i => i.name === "lonely");
        expect(lonely?.isSingleton).toBe(true);
    });

    it("assigns multi-tagged chunks to the first matching island with ghosts in others", () => {
        const input = makeInput({
            chunkTags: [
                { chunkId: "c1", tagTypeId: "tt1", tagName: "auth" },
                { chunkId: "c1", tagTypeId: "tt1", tagName: "api" },
                { chunkId: "c2", tagTypeId: "tt1", tagName: "auth" }
            ]
        });
        const result = formIslands(input);
        const auth = result.islands.find(i => i.name === "auth");
        const api = result.islands.find(i => i.name === "api");
        expect(auth?.chunkIds).toContain("c1");
        expect(api?.chunkIds).not.toContain("c1");
        expect(api?.ghostChunkIds).toContain("c1");
    });

    it("returns dominant relation type per bridge", () => {
        const input = makeInput({
            connections: [
                { sourceId: "c1", targetId: "c3", relation: "depends_on" },
                { sourceId: "c2", targetId: "c3", relation: "depends_on" },
                { sourceId: "c2", targetId: "c4", relation: "references" }
            ]
        });
        const result = formIslands(input);
        const bridge = result.bridges.find(b => b.fromIslandId === "auth" && b.toIslandId === "api");
        expect(bridge?.dominantRelation).toBe("depends_on");
        expect(bridge?.count).toBe(3);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/web && npx vitest run src/features/graph/island-formation.test.ts` Expected: FAIL — module `./island-formation` not found

- [ ] **Step 3: Implement island formation**

```ts
// apps/web/src/features/graph/island-formation.ts

export interface IslandFormationInput {
    chunks: Array<{ id: string; type: string }>;
    connections: Array<{ sourceId: string; targetId: string; relation: string }>;
    chunkTags: Array<{ chunkId: string; tagTypeId: string | null; tagName: string }>;
    groupingTagTypeId: string;
}

export interface Island {
    id: string;
    name: string;
    chunkIds: string[];
    ghostChunkIds: string[];
    isSingleton: boolean;
}

export interface IslandBridge {
    fromIslandId: string;
    toIslandId: string;
    count: number;
    dominantRelation: string;
}

export interface IslandFormationResult {
    islands: Island[];
    bridges: IslandBridge[];
    chunkToIsland: Map<string, string>;
}

export function formIslands(input: IslandFormationInput): IslandFormationResult {
    const { chunks, connections, chunkTags, groupingTagTypeId } = input;

    // Build chunk → tags mapping (only tags under the selected tag type)
    const chunkTagNames = new Map<string, string[]>();
    for (const ct of chunkTags) {
        if (ct.tagTypeId !== groupingTagTypeId) continue;
        const existing = chunkTagNames.get(ct.chunkId);
        if (existing) existing.push(ct.tagName);
        else chunkTagNames.set(ct.chunkId, [ct.tagName]);
    }

    // Group chunks: first tag wins for primary island, rest get ghost
    const islandChunks = new Map<string, string[]>();
    const islandGhosts = new Map<string, string[]>();
    const chunkToIsland = new Map<string, string>();

    for (const chunk of chunks) {
        const tags = chunkTagNames.get(chunk.id);
        if (!tags || tags.length === 0) {
            const arr = islandChunks.get("ungrouped") ?? [];
            arr.push(chunk.id);
            islandChunks.set("ungrouped", arr);
            chunkToIsland.set(chunk.id, "ungrouped");
            continue;
        }
        // First tag = primary island
        const primary = tags[0]!;
        const arr = islandChunks.get(primary) ?? [];
        arr.push(chunk.id);
        islandChunks.set(primary, arr);
        chunkToIsland.set(chunk.id, primary);

        // Remaining tags = ghost
        for (let i = 1; i < tags.length; i++) {
            const ghostArr = islandGhosts.get(tags[i]!) ?? [];
            ghostArr.push(chunk.id);
            islandGhosts.set(tags[i]!, ghostArr);
        }
    }

    // Build islands
    const islands: Island[] = [];
    for (const [name, chunkIds] of islandChunks) {
        islands.push({
            id: name,
            name,
            chunkIds,
            ghostChunkIds: islandGhosts.get(name) ?? [],
            isSingleton: chunkIds.length === 1 && name !== "ungrouped"
        });
    }
    // Add islands that only have ghosts (no primary chunks) — shouldn't normally happen but defensive
    for (const [name] of islandGhosts) {
        if (!islandChunks.has(name)) {
            islands.push({ id: name, name, chunkIds: [], ghostChunkIds: islandGhosts.get(name)!, isSingleton: false });
        }
    }

    // Compute bridges between islands
    const bridgeKey = (a: string, b: string) => (a < b ? `${a}|${b}` : `${b}|${a}`);
    const bridgeData = new Map<string, { from: string; to: string; relations: string[] }>();

    for (const conn of connections) {
        const fromIsland = chunkToIsland.get(conn.sourceId);
        const toIsland = chunkToIsland.get(conn.targetId);
        if (!fromIsland || !toIsland || fromIsland === toIsland) continue;

        const key = bridgeKey(fromIsland, toIsland);
        const existing = bridgeData.get(key);
        if (existing) {
            existing.relations.push(conn.relation);
        } else {
            bridgeData.set(key, { from: fromIsland, to: toIsland, relations: [conn.relation] });
        }
    }

    const bridges: IslandBridge[] = [];
    for (const data of bridgeData.values()) {
        // Find dominant relation by frequency
        const freq = new Map<string, number>();
        for (const r of data.relations) freq.set(r, (freq.get(r) ?? 0) + 1);
        let dominant = data.relations[0]!;
        let maxCount = 0;
        for (const [r, c] of freq) {
            if (c > maxCount) {
                dominant = r;
                maxCount = c;
            }
        }
        bridges.push({
            fromIslandId: data.from,
            toIslandId: data.to,
            count: data.relations.length,
            dominantRelation: dominant
        });
    }

    return { islands, bridges, chunkToIsland };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/web && npx vitest run src/features/graph/island-formation.test.ts` Expected: All 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/graph/island-formation.ts apps/web/src/features/graph/island-formation.test.ts
git commit -m "feat(graph): add island formation logic with tests"
```

---

## Task 2: Island Layout (Force Simulation)

Lightweight force simulation that positions islands (5-20 nodes). Much simpler than the current 200-iteration simulation — no quadtree, no
web worker.

**Files:**

- Create: `apps/web/src/features/graph/island-layout.ts`
- Create: `apps/web/src/features/graph/island-layout.test.ts`

- [ ] **Step 1: Write failing tests for island layout**

```ts
// apps/web/src/features/graph/island-layout.test.ts
import { describe, expect, it } from "vitest";
import { layoutIslands, type IslandLayoutInput } from "./island-layout";

describe("layoutIslands", () => {
    it("assigns positions to all islands", () => {
        const input: IslandLayoutInput = {
            islands: [
                { id: "auth", chunkCount: 7 },
                { id: "api", chunkCount: 12 },
                { id: "db", chunkCount: 5 }
            ],
            bridges: [
                { fromIslandId: "auth", toIslandId: "api", count: 5 },
                { fromIslandId: "api", toIslandId: "db", count: 8 }
            ]
        };
        const positions = layoutIslands(input);
        expect(Object.keys(positions)).toHaveLength(3);
        expect(positions.auth).toHaveProperty("x");
        expect(positions.auth).toHaveProperty("y");
    });

    it("places connected islands closer than unconnected ones", () => {
        const input: IslandLayoutInput = {
            islands: [
                { id: "a", chunkCount: 5 },
                { id: "b", chunkCount: 5 },
                { id: "c", chunkCount: 5 }
            ],
            bridges: [
                { fromIslandId: "a", toIslandId: "b", count: 10 }
                // no bridge between a↔c or b↔c
            ]
        };
        const positions = layoutIslands(input);
        const distAB = Math.hypot(positions.a!.x - positions.b!.x, positions.a!.y - positions.b!.y);
        const distAC = Math.hypot(positions.a!.x - positions.c!.x, positions.a!.y - positions.c!.y);
        expect(distAB).toBeLessThan(distAC);
    });

    it("returns empty object for empty input", () => {
        const positions = layoutIslands({ islands: [], bridges: [] });
        expect(positions).toEqual({});
    });

    it("handles single island", () => {
        const positions = layoutIslands({
            islands: [{ id: "solo", chunkCount: 3 }],
            bridges: []
        });
        expect(positions.solo).toEqual({ x: 0, y: 0 });
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/web && npx vitest run src/features/graph/island-layout.test.ts` Expected: FAIL

- [ ] **Step 3: Implement island layout**

```ts
// apps/web/src/features/graph/island-layout.ts

export interface IslandLayoutInput {
    islands: Array<{ id: string; chunkCount: number }>;
    bridges: Array<{ fromIslandId: string; toIslandId: string; count: number }>;
}

const REPULSION = 50000;
const SPRING_K = 0.005;
const SPRING_REST = 300;
const CENTER_PULL = 0.01;
const DAMPING = 0.8;
const ITERATIONS = 80;

export function layoutIslands(input: IslandLayoutInput): Record<string, { x: number; y: number }> {
    const { islands, bridges } = input;
    if (islands.length === 0) return {};
    if (islands.length === 1) return { [islands[0]!.id]: { x: 0, y: 0 } };

    // Initialize positions in a circle
    const state = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    const radius = islands.length * 40;
    for (let i = 0; i < islands.length; i++) {
        const angle = (2 * Math.PI * i) / islands.length;
        state.set(islands[i]!.id, {
            x: Math.cos(angle) * radius,
            y: Math.sin(angle) * radius,
            vx: 0,
            vy: 0
        });
    }

    for (let iter = 0; iter < ITERATIONS; iter++) {
        // Repulsion between all pairs
        for (let i = 0; i < islands.length; i++) {
            const a = state.get(islands[i]!.id)!;
            for (let j = i + 1; j < islands.length; j++) {
                const b = state.get(islands[j]!.id)!;
                const dx = a.x - b.x;
                const dy = a.y - b.y;
                const dist = Math.max(Math.hypot(dx, dy), 1);
                const force = REPULSION / (dist * dist);
                const fx = (dx / dist) * force;
                const fy = (dy / dist) * force;
                a.vx += fx;
                a.vy += fy;
                b.vx -= fx;
                b.vy -= fy;
            }
        }

        // Spring attraction for bridges
        for (const bridge of bridges) {
            const a = state.get(bridge.fromIslandId);
            const b = state.get(bridge.toIslandId);
            if (!a || !b) continue;
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            const dist = Math.max(Math.hypot(dx, dy), 1);
            const displacement = dist - SPRING_REST;
            const force = SPRING_K * displacement * Math.min(bridge.count, 10);
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;
            a.vx += fx;
            a.vy += fy;
            b.vx -= fx;
            b.vy -= fy;
        }

        // Center pull
        for (const s of state.values()) {
            s.vx -= s.x * CENTER_PULL;
            s.vy -= s.y * CENTER_PULL;
        }

        // Apply velocity with damping
        for (const s of state.values()) {
            s.x += s.vx;
            s.y += s.vy;
            s.vx *= DAMPING;
            s.vy *= DAMPING;
        }
    }

    const positions: Record<string, { x: number; y: number }> = {};
    for (const [id, s] of state) {
        positions[id] = { x: Math.round(s.x), y: Math.round(s.y) };
    }
    return positions;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/web && npx vitest run src/features/graph/island-layout.test.ts` Expected: All 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/graph/island-layout.ts apps/web/src/features/graph/island-layout.test.ts
git commit -m "feat(graph): add island layout force simulation with tests"
```

---

## Task 3: Neighborhood Layout

Radial placement of chunks around a focus chunk. 1-hop neighbors in an inner ring, 2-hop neighbors in an outer ring.

**Files:**

- Create: `apps/web/src/features/graph/neighborhood-layout.ts`
- Create: `apps/web/src/features/graph/neighborhood-layout.test.ts`

- [ ] **Step 1: Write failing tests**

```ts
// apps/web/src/features/graph/neighborhood-layout.test.ts
import { describe, expect, it } from "vitest";
import { layoutNeighborhood, type NeighborhoodInput } from "./neighborhood-layout";

describe("layoutNeighborhood", () => {
    it("places focus chunk at origin", () => {
        const result = layoutNeighborhood({
            focusChunkId: "c1",
            chunks: [{ id: "c1" }, { id: "c2" }],
            connections: [{ sourceId: "c1", targetId: "c2", relation: "depends_on" }]
        });
        expect(result.positions.c1).toEqual({ x: 0, y: 0 });
    });

    it("places 1-hop neighbors in inner ring", () => {
        const result = layoutNeighborhood({
            focusChunkId: "c1",
            chunks: [{ id: "c1" }, { id: "c2" }, { id: "c3" }],
            connections: [
                { sourceId: "c1", targetId: "c2", relation: "depends_on" },
                { sourceId: "c1", targetId: "c3", relation: "part_of" }
            ]
        });
        expect(result.hops.get("c2")).toBe(1);
        expect(result.hops.get("c3")).toBe(1);
        const dist2 = Math.hypot(result.positions.c2!.x, result.positions.c2!.y);
        const dist3 = Math.hypot(result.positions.c3!.x, result.positions.c3!.y);
        // Both should be at the same radius (inner ring)
        expect(Math.abs(dist2 - dist3)).toBeLessThan(1);
    });

    it("places 2-hop neighbors in outer ring", () => {
        const result = layoutNeighborhood({
            focusChunkId: "c1",
            chunks: [{ id: "c1" }, { id: "c2" }, { id: "c3" }],
            connections: [
                { sourceId: "c1", targetId: "c2", relation: "depends_on" },
                { sourceId: "c2", targetId: "c3", relation: "part_of" }
            ]
        });
        expect(result.hops.get("c3")).toBe(2);
        const dist2 = Math.hypot(result.positions.c2!.x, result.positions.c2!.y);
        const dist3 = Math.hypot(result.positions.c3!.x, result.positions.c3!.y);
        expect(dist3).toBeGreaterThan(dist2);
    });

    it("returns only chunks within 2 hops", () => {
        const result = layoutNeighborhood({
            focusChunkId: "c1",
            chunks: [{ id: "c1" }, { id: "c2" }, { id: "c3" }, { id: "c4" }],
            connections: [
                { sourceId: "c1", targetId: "c2", relation: "depends_on" },
                { sourceId: "c2", targetId: "c3", relation: "part_of" },
                { sourceId: "c3", targetId: "c4", relation: "references" }
            ]
        });
        expect(result.positions).toHaveProperty("c1");
        expect(result.positions).toHaveProperty("c2");
        expect(result.positions).toHaveProperty("c3");
        expect(result.positions).not.toHaveProperty("c4");
    });

    it("handles focus chunk with no connections", () => {
        const result = layoutNeighborhood({
            focusChunkId: "c1",
            chunks: [{ id: "c1" }],
            connections: []
        });
        expect(result.positions).toEqual({ c1: { x: 0, y: 0 } });
        expect(result.hops.get("c1")).toBe(0);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/web && npx vitest run src/features/graph/neighborhood-layout.test.ts` Expected: FAIL

- [ ] **Step 3: Implement neighborhood layout**

```ts
// apps/web/src/features/graph/neighborhood-layout.ts

export interface NeighborhoodInput {
    focusChunkId: string;
    chunks: Array<{ id: string }>;
    connections: Array<{ sourceId: string; targetId: string; relation: string }>;
}

export interface NeighborhoodResult {
    positions: Record<string, { x: number; y: number }>;
    hops: Map<string, number>;
    visibleConnections: Array<{ sourceId: string; targetId: string; relation: string }>;
}

const INNER_RING_RADIUS = 250;
const OUTER_RING_RADIUS = 450;
const MAX_HOPS = 2;

export function layoutNeighborhood(input: NeighborhoodInput): NeighborhoodResult {
    const { focusChunkId, chunks, connections } = input;

    // Build adjacency
    const adjacency = new Map<string, Array<{ neighbor: string; relation: string }>>();
    for (const conn of connections) {
        const fromList = adjacency.get(conn.sourceId) ?? [];
        fromList.push({ neighbor: conn.targetId, relation: conn.relation });
        adjacency.set(conn.sourceId, fromList);
        const toList = adjacency.get(conn.targetId) ?? [];
        toList.push({ neighbor: conn.sourceId, relation: conn.relation });
        adjacency.set(conn.targetId, toList);
    }

    // BFS from focus to compute hops
    const chunkIdSet = new Set(chunks.map(c => c.id));
    const hops = new Map<string, number>();
    hops.set(focusChunkId, 0);
    const queue = [focusChunkId];
    let qi = 0;

    while (qi < queue.length) {
        const current = queue[qi++]!;
        const currentHop = hops.get(current)!;
        if (currentHop >= MAX_HOPS) continue;

        for (const { neighbor } of adjacency.get(current) ?? []) {
            if (!hops.has(neighbor) && chunkIdSet.has(neighbor)) {
                hops.set(neighbor, currentHop + 1);
                queue.push(neighbor);
            }
        }
    }

    // Position chunks by hop level
    const positions: Record<string, { x: number; y: number }> = {};
    positions[focusChunkId] = { x: 0, y: 0 };

    const byHop = new Map<number, string[]>();
    for (const [id, hop] of hops) {
        if (id === focusChunkId) continue;
        const list = byHop.get(hop) ?? [];
        list.push(id);
        byHop.set(hop, list);
    }

    for (const [hop, ids] of byHop) {
        const radius = hop === 1 ? INNER_RING_RADIUS : OUTER_RING_RADIUS;
        for (let i = 0; i < ids.length; i++) {
            const angle = (2 * Math.PI * i) / ids.length - Math.PI / 2;
            positions[ids[i]!] = {
                x: Math.round(Math.cos(angle) * radius),
                y: Math.round(Math.sin(angle) * radius)
            };
        }
    }

    // Filter connections to only visible chunks
    const visibleIds = new Set(Object.keys(positions));
    const visibleConnections = connections.filter(c => visibleIds.has(c.sourceId) && visibleIds.has(c.targetId));

    return { positions, hops, visibleConnections };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/web && npx vitest run src/features/graph/neighborhood-layout.test.ts` Expected: All 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/graph/neighborhood-layout.ts apps/web/src/features/graph/neighborhood-layout.test.ts
git commit -m "feat(graph): add neighborhood radial layout with tests"
```

---

## Task 4: Zoom State Management

New hook managing the zoom level state machine (overview ↔ neighborhood ↔ detail) and simplified graph state.

**Files:**

- Create: `apps/web/src/features/graph/use-graph-zoom.ts`
- Modify: `apps/web/src/features/graph/use-graph-state.ts`

- [ ] **Step 1: Create the zoom state hook**

```ts
// apps/web/src/features/graph/use-graph-zoom.ts
import { useCallback, useMemo, useReducer } from "react";

export type ZoomLevel = "overview" | "neighborhood" | "detail";

export interface ZoomState {
    level: ZoomLevel;
    focusChunkId: string | null;
    activeIslandId: string | null;
    detailChunkId: string | null;
    breadcrumbs: Array<{ level: ZoomLevel; label: string; islandId?: string; chunkId?: string }>;
}

type ZoomAction =
    | { type: "ZOOM_TO_ISLAND"; islandId: string; focusChunkId: string }
    | { type: "ZOOM_TO_CHUNK"; chunkId: string; islandId: string }
    | { type: "OPEN_DETAIL"; chunkId: string }
    | { type: "CLOSE_DETAIL" }
    | { type: "ZOOM_TO_OVERVIEW" }
    | { type: "GO_BACK" }
    | { type: "SET_FOCUS_CHUNK"; chunkId: string };

const INITIAL_STATE: ZoomState = {
    level: "overview",
    focusChunkId: null,
    activeIslandId: null,
    detailChunkId: null,
    breadcrumbs: [{ level: "overview", label: "Overview" }]
};

function zoomReducer(state: ZoomState, action: ZoomAction): ZoomState {
    switch (action.type) {
        case "ZOOM_TO_ISLAND":
            return {
                level: "neighborhood",
                focusChunkId: action.focusChunkId,
                activeIslandId: action.islandId,
                detailChunkId: null,
                breadcrumbs: [
                    { level: "overview", label: "Overview" },
                    { level: "neighborhood", label: action.islandId, islandId: action.islandId, chunkId: action.focusChunkId }
                ]
            };
        case "ZOOM_TO_CHUNK":
            return {
                level: "neighborhood",
                focusChunkId: action.chunkId,
                activeIslandId: action.islandId,
                detailChunkId: null,
                breadcrumbs: [
                    { level: "overview", label: "Overview" },
                    { level: "neighborhood", label: action.islandId, islandId: action.islandId, chunkId: action.chunkId }
                ]
            };
        case "SET_FOCUS_CHUNK":
            return {
                ...state,
                focusChunkId: action.chunkId
            };
        case "OPEN_DETAIL":
            return {
                ...state,
                level: "detail",
                detailChunkId: action.chunkId,
                breadcrumbs: [
                    ...state.breadcrumbs.filter(b => b.level !== "detail"),
                    { level: "detail", label: "Detail", chunkId: action.chunkId }
                ]
            };
        case "CLOSE_DETAIL":
            return {
                ...state,
                level: "neighborhood",
                detailChunkId: null,
                breadcrumbs: state.breadcrumbs.filter(b => b.level !== "detail")
            };
        case "ZOOM_TO_OVERVIEW":
            return INITIAL_STATE;
        case "GO_BACK": {
            if (state.level === "detail") return zoomReducer(state, { type: "CLOSE_DETAIL" });
            if (state.level === "neighborhood") return INITIAL_STATE;
            return state;
        }
        default:
            return state;
    }
}

export function useGraphZoom(initialFocusChunkId?: string, initialIslandId?: string) {
    const initialState = useMemo<ZoomState>(() => {
        if (initialFocusChunkId && initialIslandId) {
            return {
                level: "neighborhood",
                focusChunkId: initialFocusChunkId,
                activeIslandId: initialIslandId,
                detailChunkId: null,
                breadcrumbs: [
                    { level: "overview", label: "Overview" },
                    { level: "neighborhood", label: initialIslandId, islandId: initialIslandId, chunkId: initialFocusChunkId }
                ]
            };
        }
        return INITIAL_STATE;
    }, [initialFocusChunkId, initialIslandId]);

    const [zoom, dispatchZoom] = useReducer(zoomReducer, initialState);

    const zoomToIsland = useCallback((islandId: string, focusChunkId: string) => {
        dispatchZoom({ type: "ZOOM_TO_ISLAND", islandId, focusChunkId });
    }, []);

    const zoomToChunk = useCallback((chunkId: string, islandId: string) => {
        dispatchZoom({ type: "ZOOM_TO_CHUNK", chunkId, islandId });
    }, []);

    const openDetail = useCallback((chunkId: string) => {
        dispatchZoom({ type: "OPEN_DETAIL", chunkId });
    }, []);

    const closeDetail = useCallback(() => {
        dispatchZoom({ type: "CLOSE_DETAIL" });
    }, []);

    const goBack = useCallback(() => {
        dispatchZoom({ type: "GO_BACK" });
    }, []);

    const goToOverview = useCallback(() => {
        dispatchZoom({ type: "ZOOM_TO_OVERVIEW" });
    }, []);

    const setFocusChunk = useCallback((chunkId: string) => {
        dispatchZoom({ type: "SET_FOCUS_CHUNK", chunkId });
    }, []);

    return {
        zoom,
        zoomToIsland,
        zoomToChunk,
        openDetail,
        closeDetail,
        goBack,
        goToOverview,
        setFocusChunk
    };
}
```

- [ ] **Step 2: Simplify GraphState — remove obsolete fields**

Replace the contents of `apps/web/src/features/graph/use-graph-state.ts`. Remove: `exploreMode`, `exploredNodeIds`, `layoutAlgorithm`,
`bundleEdges`, `useMainThread`, `timelineCutoff`, `edgeAnimated`, `expandedClusters`, `collapsedParents`, `focusedNodeId`. Keep: selection,
path-finding, search, filter, panel state.

```ts
// apps/web/src/features/graph/use-graph-state.ts
import { useCallback, useReducer } from "react";

export interface GraphState {
    selectedChunkId: string | null;
    multiSelectedIds: Set<string>;
    pathStartId: string | null;
    pathEndId: string | null;
    showHelp: boolean;
    showPathPanel: boolean;
    showSaveDialog: boolean;
    showDeleteConfirm: boolean;
    viewName: string;
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    searchQuery: string;
    groupingTagTypeId: string | null;
    showUngrouped: boolean;
    panelWidth: number;
    heatmapMode: boolean;
}

export type GraphAction =
    | { type: "SET_SELECTED_CHUNK"; id: string | null }
    | { type: "SET_MULTI_SELECTED"; ids: Set<string> }
    | { type: "TOGGLE_MULTI_SELECT"; id: string }
    | { type: "CLEAR_MULTI_SELECT" }
    | { type: "SET_PATH_START"; id: string | null }
    | { type: "SET_PATH_END"; id: string | null }
    | { type: "CLEAR_PATH" }
    | { type: "TOGGLE_HELP" }
    | { type: "SET_SHOW_PATH_PANEL"; show: boolean }
    | { type: "SET_SHOW_SAVE_DIALOG"; show: boolean }
    | { type: "SET_SHOW_DELETE_CONFIRM"; show: boolean }
    | { type: "SET_VIEW_NAME"; name: string }
    | { type: "TOGGLE_FILTER_TYPE"; filterType: string }
    | { type: "SET_FILTER_TYPES"; types: Set<string> }
    | { type: "TOGGLE_FILTER_RELATION"; relation: string }
    | { type: "SET_FILTER_RELATIONS"; relations: Set<string> }
    | { type: "SET_SEARCH_QUERY"; query: string }
    | { type: "SET_GROUPING_TAG_TYPE"; id: string | null }
    | { type: "TOGGLE_UNGROUPED" }
    | { type: "SET_PANEL_WIDTH"; width: number }
    | { type: "TOGGLE_HEATMAP" }
    | { type: "DESELECT_ALL" }
    | { type: "RESTORE_VIEW"; filterTypes: string[]; filterRelations: string[] };

const INITIAL_STATE: GraphState = {
    selectedChunkId: null,
    multiSelectedIds: new Set(),
    pathStartId: null,
    pathEndId: null,
    showHelp: false,
    showPathPanel: false,
    showSaveDialog: false,
    showDeleteConfirm: false,
    viewName: "",
    filterTypes: new Set(),
    filterRelations: new Set(),
    searchQuery: "",
    groupingTagTypeId: null,
    showUngrouped: true,
    panelWidth: 320,
    heatmapMode: false
};

function graphReducer(state: GraphState, action: GraphAction): GraphState {
    switch (action.type) {
        case "SET_SELECTED_CHUNK":
            return { ...state, selectedChunkId: action.id };
        case "SET_MULTI_SELECTED":
            return { ...state, multiSelectedIds: action.ids };
        case "TOGGLE_MULTI_SELECT": {
            const next = new Set(state.multiSelectedIds);
            if (next.has(action.id)) next.delete(action.id);
            else next.add(action.id);
            return { ...state, multiSelectedIds: next };
        }
        case "CLEAR_MULTI_SELECT":
            return { ...state, multiSelectedIds: new Set() };
        case "SET_PATH_START":
            return { ...state, pathStartId: action.id };
        case "SET_PATH_END":
            return { ...state, pathEndId: action.id };
        case "CLEAR_PATH":
            return { ...state, pathStartId: null, pathEndId: null, showPathPanel: false };
        case "TOGGLE_HELP":
            return { ...state, showHelp: !state.showHelp };
        case "SET_SHOW_PATH_PANEL":
            return { ...state, showPathPanel: action.show };
        case "SET_SHOW_SAVE_DIALOG":
            return { ...state, showSaveDialog: action.show };
        case "SET_SHOW_DELETE_CONFIRM":
            return { ...state, showDeleteConfirm: action.show };
        case "SET_VIEW_NAME":
            return { ...state, viewName: action.name };
        case "TOGGLE_FILTER_TYPE": {
            const next = new Set(state.filterTypes);
            if (next.has(action.filterType)) next.delete(action.filterType);
            else next.add(action.filterType);
            return { ...state, filterTypes: next };
        }
        case "SET_FILTER_TYPES":
            return { ...state, filterTypes: action.types };
        case "TOGGLE_FILTER_RELATION": {
            const next = new Set(state.filterRelations);
            if (next.has(action.relation)) next.delete(action.relation);
            else next.add(action.relation);
            return { ...state, filterRelations: next };
        }
        case "SET_FILTER_RELATIONS":
            return { ...state, filterRelations: action.relations };
        case "SET_SEARCH_QUERY":
            return { ...state, searchQuery: action.query };
        case "SET_GROUPING_TAG_TYPE":
            return { ...state, groupingTagTypeId: action.id };
        case "TOGGLE_UNGROUPED":
            return { ...state, showUngrouped: !state.showUngrouped };
        case "SET_PANEL_WIDTH":
            return { ...state, panelWidth: action.width };
        case "TOGGLE_HEATMAP":
            return { ...state, heatmapMode: !state.heatmapMode };
        case "DESELECT_ALL":
            return { ...state, selectedChunkId: null, multiSelectedIds: new Set() };
        case "RESTORE_VIEW":
            return {
                ...state,
                filterTypes: new Set(action.filterTypes),
                filterRelations: new Set(action.filterRelations)
            };
        default:
            return state;
    }
}

export function useGraphState() {
    const [state, dispatch] = useReducer(graphReducer, INITIAL_STATE);
    return { state, dispatch };
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/use-graph-zoom.ts apps/web/src/features/graph/use-graph-state.ts
git commit -m "feat(graph): add zoom state machine and simplify graph state"
```

---

## Task 5: Typed Edge Component

Renders edges with relation-specific color, line style, and marker. Replaces `floating-edge.tsx`.

**Files:**

- Create: `apps/web/src/features/graph/typed-edge.tsx`

- [ ] **Step 1: Implement typed edge**

```tsx
// apps/web/src/features/graph/typed-edge.tsx
import { memo } from "react";
import { BaseEdge, type EdgeProps, getBezierPath } from "@xyflow/react";

interface TypedEdgeData {
    relation: string;
    [key: string]: unknown;
}

const EDGE_STYLES: Record<
    string,
    {
        color: string;
        strokeWidth: number;
        strokeDasharray?: string;
        markerEnd?: string;
    }
> = {
    depends_on: { color: "#3b82f6", strokeWidth: 2, markerEnd: "arrow-filled" },
    part_of: { color: "#22c55e", strokeWidth: 2.5, markerEnd: "dot" },
    extends: { color: "#a78bfa", strokeWidth: 1.5, markerEnd: "arrow-open" },
    references: { color: "#94a3b8", strokeWidth: 1, strokeDasharray: "6 4" },
    related_to: { color: "#94a3b8", strokeWidth: 1, strokeDasharray: "6 4" },
    contradicts: { color: "#ef4444", strokeWidth: 2, strokeDasharray: "4 4", markerEnd: "slash" },
    alternative_to: { color: "#f59e0b", strokeWidth: 1.5, markerEnd: "fork" },
    supports: { color: "#06b6d4", strokeWidth: 1.5, strokeDasharray: "8 3" }
};

const DEFAULT_STYLE = { color: "#6b7280", strokeWidth: 1, strokeDasharray: undefined, markerEnd: undefined };

function TypedEdgeComponent({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, style }: EdgeProps) {
    const relation = (data as TypedEdgeData | undefined)?.relation ?? "related_to";
    const edgeStyle = EDGE_STYLES[relation] ?? DEFAULT_STYLE;

    const [edgePath] = getBezierPath({
        sourceX,
        sourceY,
        sourcePosition,
        targetX,
        targetY,
        targetPosition
    });

    return (
        <>
            <defs>
                <marker id={`arrow-filled-${id}`} markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                    <polygon points="0 0, 8 3, 0 6" fill={edgeStyle.color} />
                </marker>
                <marker id={`arrow-open-${id}`} markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                    <path d="M0,0 L8,3 L0,6" fill="none" stroke={edgeStyle.color} strokeWidth="1.5" />
                </marker>
                <marker id={`dot-${id}`} markerWidth="8" markerHeight="8" refX="4" refY="4" orient="auto">
                    <circle cx="4" cy="4" r="3" fill="none" stroke={edgeStyle.color} strokeWidth="1.5" />
                </marker>
            </defs>
            <BaseEdge
                id={id}
                path={edgePath}
                style={{
                    ...style,
                    stroke: edgeStyle.color,
                    strokeWidth: edgeStyle.strokeWidth,
                    strokeDasharray: edgeStyle.strokeDasharray
                }}
                markerEnd={edgeStyle.markerEnd ? `url(#${edgeStyle.markerEnd}-${id})` : undefined}
            />
        </>
    );
}

export const TypedEdge = memo(TypedEdgeComponent);
export default TypedEdge;
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/graph/typed-edge.tsx
git commit -m "feat(graph): add typed edge component with relation-specific visuals"
```

---

## Task 6: Island Node Component

Renders an island in the overview level with name, chunk count, icon, and health dot cluster.

**Files:**

- Create: `apps/web/src/features/graph/graph-island-node.tsx`

- [ ] **Step 1: Implement island node**

```tsx
// apps/web/src/features/graph/graph-island-node.tsx
import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export interface IslandNodeData {
    name: string;
    chunkCount: number;
    color: string;
    healthScores: number[];
    isSingleton: boolean;
    [key: string]: unknown;
}

function healthColor(score: number): string {
    if (score >= 80) return "#22c55e";
    if (score >= 60) return "#4ade80";
    if (score >= 40) return "#f59e0b";
    return "#ef4444";
}

function IslandNodeComponent({ data }: NodeProps) {
    const { name, chunkCount, color, healthScores, isSingleton } = data as IslandNodeData;

    if (isSingleton) {
        return (
            <div
                className="rounded-lg px-3 py-2 text-center backdrop-blur-sm"
                style={{
                    background: `${color}15`,
                    border: `1px solid ${color}30`
                }}
            >
                <Handle type="source" position={Position.Top} className="!invisible" />
                <Handle type="target" position={Position.Bottom} className="!invisible" />
                <div className="text-[11px] font-medium" style={{ color }}>
                    {name}
                </div>
                <div className="mt-1 text-[9px] text-slate-500">1 chunk</div>
            </div>
        );
    }

    return (
        <div
            className="min-w-[100px] rounded-3xl px-5 py-4 text-center backdrop-blur-sm"
            style={{
                background: `${color}12`,
                border: `1.5px solid ${color}25`
            }}
        >
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <Handle type="source" position={Position.Left} className="!invisible" id="left" />
            <Handle type="target" position={Position.Right} className="!invisible" id="right" />
            <div className="text-[11px] font-semibold" style={{ color }}>
                {name}
            </div>
            <div className="mt-1 text-[9px] text-slate-500">{chunkCount} chunks</div>
            {healthScores.length > 0 && (
                <div className="mt-2 flex justify-center gap-[2px]">
                    {healthScores.map((score, i) => (
                        <div key={i} className="h-[5px] w-[5px] rounded-full" style={{ background: healthColor(score) }} />
                    ))}
                </div>
            )}
        </div>
    );
}

export const GraphIslandNode = memo(IslandNodeComponent);
export default GraphIslandNode;
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/graph/graph-island-node.tsx
git commit -m "feat(graph): add island node component for overview level"
```

---

## Task 7: Chunk Card and Dot Components

Renders chunk nodes at the neighborhood zoom level. Cards for 1-hop (with summary, tags, health), dots for 2-hop.

**Files:**

- Create: `apps/web/src/features/graph/graph-chunk-card.tsx`
- Create: `apps/web/src/features/graph/graph-chunk-dot.tsx`

- [ ] **Step 1: Implement chunk card (neighborhood focus + neighbor)**

```tsx
// apps/web/src/features/graph/graph-chunk-card.tsx
import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export interface ChunkCardData {
    title: string;
    summary?: string | null;
    type: string;
    tags: Array<{ name: string; color: string }>;
    healthScore: number;
    isFocus: boolean;
    [key: string]: unknown;
}

function healthColor(score: number): string {
    if (score >= 80) return "#22c55e";
    if (score >= 60) return "#4ade80";
    if (score >= 40) return "#f59e0b";
    return "#ef4444";
}

function ChunkCardComponent({ data }: NodeProps) {
    const { title, summary, tags, healthScore, isFocus } = data as ChunkCardData;

    return (
        <div
            className="max-w-[160px] rounded-[10px] border bg-slate-800 text-left"
            style={{
                borderWidth: isFocus ? "2px" : "1px",
                borderColor: isFocus ? "#3b82f6" : "#334155",
                padding: isFocus ? "10px 14px" : "7px 11px",
                boxShadow: isFocus ? "0 0 24px rgba(59,130,246,0.25)" : "none"
            }}
        >
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <Handle type="source" position={Position.Left} className="!invisible" id="left" />
            <Handle type="target" position={Position.Right} className="!invisible" id="right" />
            <div className="flex items-center gap-1.5">
                <div className="h-2 w-2 flex-shrink-0 rounded-full" style={{ background: healthColor(healthScore) }} />
                <div className="truncate font-medium text-slate-200" style={{ fontSize: isFocus ? "11px" : "10px" }}>
                    {title}
                </div>
            </div>
            {summary && (
                <div className="mt-1 line-clamp-2 text-slate-500" style={{ fontSize: isFocus ? "9px" : "8px", lineHeight: "1.3" }}>
                    {summary}
                </div>
            )}
            {isFocus && tags.length > 0 && (
                <div className="mt-1.5 flex flex-wrap gap-1">
                    {tags.slice(0, 3).map(tag => (
                        <span
                            key={tag.name}
                            className="rounded-[3px] px-1 py-px text-[7px]"
                            style={{ background: `${tag.color}20`, color: tag.color }}
                        >
                            {tag.name}
                        </span>
                    ))}
                </div>
            )}
        </div>
    );
}

export const GraphChunkCard = memo(ChunkCardComponent);
export default GraphChunkCard;
```

- [ ] **Step 2: Implement chunk dot (2-hop)**

```tsx
// apps/web/src/features/graph/graph-chunk-dot.tsx
import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export interface ChunkDotData {
    title: string;
    color: string;
    [key: string]: unknown;
}

function ChunkDotComponent({ data }: NodeProps) {
    const { title, color } = data as ChunkDotData;
    return (
        <div className="group flex flex-col items-center">
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <div className="h-[8px] w-[8px] rounded-full opacity-70" style={{ background: color }} />
            <div className="mt-0.5 max-w-[80px] truncate text-center text-[7px] text-slate-500 opacity-0 transition-opacity group-hover:opacity-100">
                {title}
            </div>
        </div>
    );
}

export const GraphChunkDot = memo(ChunkDotComponent);
export default GraphChunkDot;
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/graph-chunk-card.tsx apps/web/src/features/graph/graph-chunk-dot.tsx
git commit -m "feat(graph): add chunk card and dot components for neighborhood level"
```

---

## Task 8: Rewrite Node Builder (use-graph-nodes.ts)

The central pipeline that builds React Flow nodes and edges based on the current zoom level.

**Files:**

- Modify: `apps/web/src/features/graph/use-graph-nodes.ts` (full rewrite)

- [ ] **Step 1: Rewrite use-graph-nodes.ts**

```ts
// apps/web/src/features/graph/use-graph-nodes.ts
import { useMemo } from "react";
import type { Edge, Node } from "@xyflow/react";

import { relationColor } from "@/features/chunks/relation-colors";
import type { Island, IslandBridge } from "@/features/graph/island-formation";
import type { NeighborhoodResult } from "@/features/graph/neighborhood-layout";
import type { ZoomLevel } from "@/features/graph/use-graph-zoom";
import type { GraphData } from "@/features/graph/use-graph-data";

interface UseGraphNodesParams {
    zoomLevel: ZoomLevel;
    // Overview data
    islands: Island[];
    bridges: IslandBridge[];
    islandPositions: Record<string, { x: number; y: number }>;
    islandHealthScores: Map<string, number[]>;
    islandColors: Map<string, string>;
    // Neighborhood data
    neighborhood: NeighborhoodResult | null;
    focusChunkId: string | null;
    // Shared data
    data: GraphData | undefined;
    chunkSummaries: Map<string, string | null>;
    chunkHealthScores: Map<string, number>;
    chunkTags: Map<string, Array<{ name: string; color: string }>>;
    // Filters
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    heatmapMode: boolean;
}

export function useGraphNodes(params: UseGraphNodesParams) {
    const {
        zoomLevel,
        islands,
        bridges,
        islandPositions,
        islandHealthScores,
        islandColors,
        neighborhood,
        focusChunkId,
        data,
        chunkSummaries,
        chunkHealthScores,
        chunkTags,
        filterTypes,
        filterRelations,
        heatmapMode
    } = params;

    const overviewNodes = useMemo<Node[]>(() => {
        if (zoomLevel !== "overview") return [];
        return islands
            .filter(island => island.chunkIds.length > 0 || island.ghostChunkIds.length > 0)
            .map(island => ({
                id: `island-${island.id}`,
                type: "island",
                position: islandPositions[island.id] ?? { x: 0, y: 0 },
                data: {
                    name: island.name,
                    chunkCount: island.chunkIds.length,
                    color: islandColors.get(island.id) ?? "#64748b",
                    healthScores: islandHealthScores.get(island.id) ?? [],
                    isSingleton: island.isSingleton
                }
            }));
    }, [zoomLevel, islands, islandPositions, islandHealthScores, islandColors]);

    const overviewEdges = useMemo<Edge[]>(() => {
        if (zoomLevel !== "overview") return [];
        return bridges
            .filter(b => filterRelations.size === 0 || filterRelations.has(b.dominantRelation))
            .map(bridge => ({
                id: `bridge-${bridge.fromIslandId}-${bridge.toIslandId}`,
                source: `island-${bridge.fromIslandId}`,
                target: `island-${bridge.toIslandId}`,
                type: "typed",
                data: { relation: bridge.dominantRelation },
                label: `×${bridge.count}`,
                labelStyle: { fill: "#64748b", fontSize: 9, fontFamily: "monospace" }
            }));
    }, [zoomLevel, bridges, filterRelations]);

    const neighborhoodNodes = useMemo<Node[]>(() => {
        if (zoomLevel !== "neighborhood" && zoomLevel !== "detail") return [];
        if (!neighborhood || !focusChunkId) return [];

        return Object.entries(neighborhood.positions)
            .map(([chunkId, pos]) => {
                const hop = neighborhood.hops.get(chunkId) ?? 0;
                const chunk = data?.chunks.find((c: { id: string }) => c.id === chunkId);
                if (!chunk) return null;

                if (filterTypes.size > 0 && !filterTypes.has(chunk.type)) return null;

                if (hop <= 1) {
                    return {
                        id: chunkId,
                        type: "chunkCard",
                        position: pos,
                        data: {
                            title: chunk.title,
                            summary: chunkSummaries.get(chunkId) ?? null,
                            type: chunk.type,
                            tags: chunkTags.get(chunkId) ?? [],
                            healthScore: chunkHealthScores.get(chunkId) ?? 50,
                            isFocus: chunkId === focusChunkId
                        }
                    };
                }

                return {
                    id: chunkId,
                    type: "chunkDot",
                    position: pos,
                    data: {
                        title: chunk.title,
                        color: relationColor(chunk.type)
                    }
                };
            })
            .filter(Boolean) as Node[];
    }, [zoomLevel, neighborhood, focusChunkId, data, filterTypes, chunkSummaries, chunkHealthScores, chunkTags]);

    const neighborhoodEdges = useMemo<Edge[]>(() => {
        if (zoomLevel !== "neighborhood" && zoomLevel !== "detail") return [];
        if (!neighborhood) return [];

        const visibleNodeIds = new Set(neighborhoodNodes.map(n => n.id));

        return neighborhood.visibleConnections
            .filter(conn => {
                if (filterRelations.size > 0 && !filterRelations.has(conn.relation)) return false;
                return visibleNodeIds.has(conn.sourceId) && visibleNodeIds.has(conn.targetId);
            })
            .map(conn => ({
                id: `${conn.sourceId}-${conn.targetId}-${conn.relation}`,
                source: conn.sourceId,
                target: conn.targetId,
                type: "typed",
                data: { relation: conn.relation }
            }));
    }, [zoomLevel, neighborhood, neighborhoodNodes, filterRelations]);

    const layoutNodes = zoomLevel === "overview" ? overviewNodes : neighborhoodNodes;
    const layoutEdges = zoomLevel === "overview" ? overviewEdges : neighborhoodEdges;

    return { layoutNodes, layoutEdges };
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/graph/use-graph-nodes.ts
git commit -m "feat(graph): rewrite node builder for zoom-level-aware rendering"
```

---

## Task 9: Rewrite Graph View Orchestrator

The main component that wires everything together: data → islands → layout → zoom → nodes → React Flow.

**Files:**

- Modify: `apps/web/src/features/graph/graph-view.tsx` (full rewrite)
- Modify: `apps/web/src/features/graph/use-graph-data.ts` (add island computation)

- [ ] **Step 1: Update use-graph-data.ts to compute islands**

Add island formation to the data pipeline. Replace the grouping-related code with island computation.

```ts
// apps/web/src/features/graph/use-graph-data.ts
import { useQuery } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";

import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import type { GraphAction } from "@/features/graph/use-graph-state";
import { formIslands, type IslandFormationResult } from "@/features/graph/island-formation";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type GraphData = NonNullable<ReturnType<typeof useGraphData>["data"]>;

export function useGraphData(dispatch: React.Dispatch<GraphAction>) {
    const { codebaseId, workspaceId } = useActiveCodebase();

    const { data, isLoading } = useQuery({
        queryKey: ["graph", codebaseId, workspaceId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.graph.get({
                    query: {
                        ...(workspaceId ? { workspaceId } : {}),
                        ...(codebaseId && codebaseId !== "global" && !workspaceId ? { codebaseId } : {})
                    }
                })
            );
        }
    });

    const search = useSearch({ strict: false }) as {
        pathFrom?: string;
        pathTo?: string;
        focus?: string;
        tagTypeId?: string;
        zoomLevel?: string;
    };

    useEffect(() => {
        if (search.pathFrom) {
            dispatch({ type: "SET_PATH_START", id: search.pathFrom });
            dispatch({ type: "SET_SHOW_PATH_PANEL", show: true });
        }
        if (search.pathTo) {
            dispatch({ type: "SET_PATH_END", id: search.pathTo });
            dispatch({ type: "SET_SHOW_PATH_PANEL", show: true });
        }
    }, [search.pathFrom, search.pathTo, dispatch]);

    // Scoped chunk tags
    const scopedChunkTags = useMemo(() => {
        if (!data?.chunkTags || !data?.chunks) return [] as NonNullable<typeof data>["chunkTags"];
        const chunkIds = new Set(data.chunks.map((c: { id: string }) => c.id));
        return data.chunkTags.filter((ct: { chunkId: string }) => chunkIds.has(ct.chunkId));
    }, [data?.chunkTags, data?.chunks]);

    // Available tag types
    const availableTagTypeIds = useMemo(() => {
        const ids = new Set<string>();
        for (const ct of scopedChunkTags as Array<{ tagTypeId?: string | null }>) {
            if (ct.tagTypeId) ids.add(ct.tagTypeId);
        }
        return ids;
    }, [scopedChunkTags]);

    // Auto-select grouping tag type: URL param → first available
    const [groupingTagTypeId, setGroupingTagTypeId] = useState<string | null>(search.tagTypeId ?? null);
    useEffect(() => {
        if (!groupingTagTypeId && availableTagTypeIds.size > 0) {
            const first = availableTagTypeIds.values().next().value;
            if (first) setGroupingTagTypeId(first);
        }
    }, [groupingTagTypeId, availableTagTypeIds]);

    // Island formation
    const islandData = useMemo<IslandFormationResult | null>(() => {
        if (!data || !groupingTagTypeId) return null;
        return formIslands({
            chunks: data.chunks as Array<{ id: string; type: string }>,
            connections: data.connections as Array<{ sourceId: string; targetId: string; relation: string }>,
            chunkTags: scopedChunkTags as Array<{ chunkId: string; tagTypeId: string | null; tagName: string }>,
            groupingTagTypeId
        });
    }, [data, groupingTagTypeId, scopedChunkTags]);

    // Initial focus chunk from URL
    const initialFocusChunkId = search.focus ?? null;
    const initialIslandId = useMemo(() => {
        if (!initialFocusChunkId || !islandData) return null;
        return islandData.chunkToIsland.get(initialFocusChunkId) ?? null;
    }, [initialFocusChunkId, islandData]);

    return {
        data,
        isLoading,
        codebaseId,
        workspaceId,
        scopedChunkTags,
        availableTagTypeIds,
        groupingTagTypeId,
        setGroupingTagTypeId,
        islandData,
        initialFocusChunkId,
        initialIslandId
    };
}
```

- [ ] **Step 2: Rewrite graph-view.tsx**

This is the main orchestrator. It wires: data → islands → zoom → layout → nodes → React Flow. The full implementation is long — here is the
structure. The implementer should follow this skeleton and fill in the JSX for panels, breadcrumbs, and controls.

```tsx
// apps/web/src/features/graph/graph-view.tsx
import { useCallback, useMemo } from "react";
import { ReactFlow, useNodesState, useEdgesState, type NodeTypes, type EdgeTypes } from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { getMostConnected } from "@/features/graph/graph-utils";
import { GraphIslandNode } from "@/features/graph/graph-island-node";
import { GraphChunkCard } from "@/features/graph/graph-chunk-card";
import { GraphChunkDot } from "@/features/graph/graph-chunk-dot";
import { TypedEdge } from "@/features/graph/typed-edge";
import { layoutIslands } from "@/features/graph/island-layout";
import { layoutNeighborhood } from "@/features/graph/neighborhood-layout";
import { useGraphData } from "@/features/graph/use-graph-data";
import { useGraphNodes } from "@/features/graph/use-graph-nodes";
import { useGraphState } from "@/features/graph/use-graph-state";
import { useGraphZoom } from "@/features/graph/use-graph-zoom";

const NODE_TYPES: NodeTypes = {
    island: GraphIslandNode,
    chunkCard: GraphChunkCard,
    chunkDot: GraphChunkDot
};

const EDGE_TYPES: EdgeTypes = {
    typed: TypedEdge
};

export default function GraphView() {
    const { state, dispatch } = useGraphState();
    const {
        data,
        isLoading,
        islandData,
        initialFocusChunkId,
        initialIslandId,
        availableTagTypeIds,
        groupingTagTypeId,
        setGroupingTagTypeId,
        scopedChunkTags
    } = useGraphData(dispatch);

    const { zoom, zoomToIsland, zoomToChunk, openDetail, closeDetail, goBack, goToOverview, setFocusChunk } = useGraphZoom(
        initialFocusChunkId ?? undefined,
        initialIslandId ?? undefined
    );

    // Island positions (overview)
    const islandPositions = useMemo(() => {
        if (!islandData) return {};
        return layoutIslands({
            islands: islandData.islands.map(i => ({ id: i.id, chunkCount: i.chunkIds.length })),
            bridges: islandData.bridges
        });
    }, [islandData]);

    // Island colors from tag type colors
    const islandColors = useMemo(() => {
        const colors = new Map<string, string>();
        if (!islandData || !data?.tagTypes) return colors;
        // Use the tag's color if available, fall back to a default palette
        const defaultPalette = ["#3b82f6", "#22c55e", "#f59e0b", "#a78bfa", "#ef4444", "#06b6d4", "#f472b6", "#fb923c"];
        islandData.islands.forEach((island, i) => {
            // Try to find a tag color from chunkTags for this island name
            const tag = (scopedChunkTags as Array<{ tagName: string; tagTypeColor?: string | null }>).find(
                ct => ct.tagName === island.name && ct.tagTypeColor
            );
            colors.set(island.id, tag?.tagTypeColor ?? defaultPalette[i % defaultPalette.length]!);
        });
        return colors;
    }, [islandData, data?.tagTypes, scopedChunkTags]);

    // Island health scores (placeholder: empty until health data is wired)
    const islandHealthScores = useMemo(() => new Map<string, number[]>(), []);

    // Chunk metadata maps (summaries, health, tags) for neighborhood
    const chunkSummaries = useMemo(() => {
        const map = new Map<string, string | null>();
        if (!data?.chunks) return map;
        for (const c of data.chunks as Array<{ id: string; summary?: string | null }>) {
            map.set(c.id, (c as { summary?: string | null }).summary ?? null);
        }
        return map;
    }, [data?.chunks]);

    const chunkHealthScores = useMemo(() => new Map<string, number>(), []);

    const chunkTags = useMemo(() => {
        const map = new Map<string, Array<{ name: string; color: string }>>();
        if (!scopedChunkTags) return map;
        for (const ct of scopedChunkTags as Array<{ chunkId: string; tagName: string; tagTypeColor?: string | null }>) {
            const tags = map.get(ct.chunkId) ?? [];
            tags.push({ name: ct.tagName, color: ct.tagTypeColor ?? "#64748b" });
            map.set(ct.chunkId, tags);
        }
        return map;
    }, [scopedChunkTags]);

    // Neighborhood layout
    const neighborhood = useMemo(() => {
        if (zoom.level === "overview" || !zoom.focusChunkId || !data) return null;
        return layoutNeighborhood({
            focusChunkId: zoom.focusChunkId,
            chunks: data.chunks as Array<{ id: string }>,
            connections: data.connections as Array<{ sourceId: string; targetId: string; relation: string }>
        });
    }, [zoom.level, zoom.focusChunkId, data]);

    // Build React Flow nodes/edges
    const { layoutNodes, layoutEdges } = useGraphNodes({
        zoomLevel: zoom.level,
        islands: islandData?.islands ?? [],
        bridges: islandData?.bridges ?? [],
        islandPositions,
        islandHealthScores,
        islandColors,
        neighborhood,
        focusChunkId: zoom.focusChunkId,
        data,
        chunkSummaries,
        chunkHealthScores,
        chunkTags,
        filterTypes: state.filterTypes,
        filterRelations: state.filterRelations,
        heatmapMode: state.heatmapMode
    });

    const [nodes, setNodes, onNodesChange] = useNodesState(layoutNodes);
    const [edges, setEdges, onEdgesChange] = useEdgesState(layoutEdges);

    // Sync layout nodes/edges to React Flow state
    useMemo(() => {
        setNodes(layoutNodes);
        setEdges(layoutEdges);
    }, [layoutNodes, layoutEdges, setNodes, setEdges]);

    // Click handlers
    const onNodeClick = useCallback(
        (_: React.MouseEvent, node: { id: string }) => {
            if (zoom.level === "overview") {
                // Clicked an island → zoom to neighborhood
                const islandId = node.id.replace("island-", "");
                const island = islandData?.islands.find(i => i.id === islandId);
                if (!island || !data) return;
                const focusId =
                    getMostConnected(
                        island.chunkIds.map(id => ({ id })),
                        data.connections as Array<{ source: string; target: string }>
                    ) ?? island.chunkIds[0];
                if (focusId) zoomToIsland(islandId, focusId);
            } else if (zoom.level === "neighborhood") {
                if (node.id === zoom.focusChunkId) {
                    openDetail(node.id);
                } else {
                    const islandId = islandData?.chunkToIsland.get(node.id) ?? zoom.activeIslandId;
                    if (islandId) {
                        setFocusChunk(node.id);
                    }
                }
            } else if (zoom.level === "detail") {
                if (node.id !== zoom.detailChunkId) {
                    setFocusChunk(node.id);
                    openDetail(node.id);
                }
            }
        },
        [zoom, islandData, data, zoomToIsland, openDetail, setFocusChunk]
    );

    // Keyboard: Esc to go back
    const onKeyDown = useCallback(
        (e: KeyboardEvent) => {
            if (e.key === "Escape") goBack();
            if (e.key === "?" && !e.ctrlKey && !e.metaKey) dispatch({ type: "TOGGLE_HELP" });
        },
        [goBack, dispatch]
    );

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <p className="text-muted-foreground">Loading graph...</p>
            </div>
        );
    }

    return (
        <div className="relative h-[calc(100vh-4rem)]" onKeyDown={onKeyDown as unknown as React.KeyboardEventHandler} tabIndex={0}>
            {/* Breadcrumbs */}
            <div className="absolute left-3 top-3 z-20 flex items-center gap-1.5 text-xs text-slate-500">
                {zoom.breadcrumbs.map((crumb, i) => (
                    <span key={i} className="flex items-center gap-1.5">
                        {i > 0 && <span>›</span>}
                        <button
                            onClick={() => {
                                if (crumb.level === "overview") goToOverview();
                                else if (crumb.level === "neighborhood" && crumb.islandId && crumb.chunkId) {
                                    zoomToIsland(crumb.islandId, crumb.chunkId);
                                }
                            }}
                            className={`hover:text-slate-300 ${i === zoom.breadcrumbs.length - 1 ? "text-slate-300" : ""}`}
                        >
                            {crumb.label}
                        </button>
                    </span>
                ))}
            </div>

            {/* Tag type picker */}
            {data?.tagTypes && data.tagTypes.length > 1 && (
                <div className="absolute right-3 top-3 z-20">
                    <select
                        value={groupingTagTypeId ?? ""}
                        onChange={e => setGroupingTagTypeId(e.target.value || null)}
                        className="rounded border border-slate-600 bg-slate-800 px-2 py-1 text-xs text-slate-300"
                    >
                        {(data.tagTypes as Array<{ id: string; name: string }>)
                            .filter(tt => availableTagTypeIds.has(tt.id))
                            .map(tt => (
                                <option key={tt.id} value={tt.id}>
                                    {tt.name}
                                </option>
                            ))}
                    </select>
                </div>
            )}

            {/* React Flow canvas */}
            <ReactFlow
                nodes={nodes}
                edges={edges}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onNodeClick={onNodeClick}
                nodeTypes={NODE_TYPES}
                edgeTypes={EDGE_TYPES}
                fitView
                minZoom={0.1}
                maxZoom={2}
                proOptions={{ hideAttribution: true }}
                className="bg-[#0f172a]"
            />

            {/* Detail panel */}
            {zoom.level === "detail" && zoom.detailChunkId && (
                <div
                    className="absolute right-0 top-0 h-full overflow-y-auto border-l border-slate-700 bg-slate-800"
                    style={{ width: state.panelWidth }}
                >
                    <div className="p-4">
                        <button onClick={closeDetail} className="mb-3 text-xs text-slate-500 hover:text-slate-300">
                            ← Close
                        </button>
                        {/* Detail content will be populated from chunk data */}
                        <h3 className="text-sm font-semibold text-slate-200">
                            {data?.chunks.find((c: { id: string }) => c.id === zoom.detailChunkId)?.title}
                        </h3>
                    </div>
                </div>
            )}

            {/* Node/edge counts */}
            <div className="absolute bottom-3 left-3 z-20 text-[9px] text-slate-600">
                {nodes.length} nodes · {edges.length} edges
            </div>
        </div>
    );
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/graph-view.tsx apps/web/src/features/graph/use-graph-data.ts
git commit -m "feat(graph): rewrite graph view with zoom orchestration"
```

---

## Task 10: Update Route and Search Params

Update the TanStack Router route to support the new URL parameters.

**Files:**

- Modify: `apps/web/src/routes/graph.tsx`

- [ ] **Step 1: Update GraphSearch interface and route**

```tsx
// apps/web/src/routes/graph.tsx
import { createFileRoute } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import { RouteErrorBoundary } from "@/components/route-error-boundary";
import { getUser } from "@/functions/get-user";

const GraphView = lazy(() => import("@/features/graph/graph-view"));

export interface GraphSearch {
    pathFrom?: string;
    pathTo?: string;
    focus?: string;
    tagTypeId?: string;
    zoomLevel?: "overview" | "neighborhood" | "detail";
    island?: string;
}

export const Route = createFileRoute("/graph")({
    validateSearch: (search: Record<string, unknown>): GraphSearch => ({
        pathFrom: typeof search.pathFrom === "string" ? search.pathFrom : undefined,
        pathTo: typeof search.pathTo === "string" ? search.pathTo : undefined,
        focus: typeof search.focus === "string" ? search.focus : undefined,
        tagTypeId: typeof search.tagTypeId === "string" ? search.tagTypeId : undefined,
        zoomLevel:
            search.zoomLevel === "overview" || search.zoomLevel === "neighborhood" || search.zoomLevel === "detail"
                ? search.zoomLevel
                : undefined,
        island: typeof search.island === "string" ? search.island : undefined
    }),
    component: () => (
        <RouteErrorBoundary fallbackTitle="Graph failed to render">
            <Suspense
                fallback={
                    <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                        <p className="text-muted-foreground">Loading graph...</p>
                    </div>
                }
            >
                <GraphView />
            </Suspense>
        </RouteErrorBoundary>
    ),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/graph.tsx
git commit -m "feat(graph): update route with zoom-level URL params"
```

---

## Task 11: Delete Old Files

Remove files that have been replaced by the new implementation.

**Files:**

- Delete: `apps/web/src/features/graph/force-layout.ts`
- Delete: `apps/web/src/features/graph/quadtree.ts`
- Delete: `apps/web/src/features/graph/layout.worker.ts`
- Delete: `apps/web/src/features/graph/layout-cache.ts`
- Delete: `apps/web/src/features/graph/cluster-strategy.ts`
- Delete: `apps/web/src/features/graph/graph-node.tsx`
- Delete: `apps/web/src/features/graph/graph-group-node.tsx`
- Delete: `apps/web/src/features/graph/graph-cluster-node.tsx`
- Delete: `apps/web/src/features/graph/floating-edge.tsx`
- Delete: `apps/web/src/features/graph/use-graph-layout.ts`
- Delete: `apps/web/src/features/graph/use-graph-grouping.ts`
- Delete: `apps/web/src/features/graph/layouts.ts`
- Delete: `apps/web/src/features/graph/graph-filter-form.tsx`
- Delete: `apps/web/src/features/graph/graph-filter-dialog.tsx`

- [ ] **Step 1: Delete old files**

```bash
cd apps/web/src/features/graph && rm -f \
  force-layout.ts \
  quadtree.ts \
  layout.worker.ts \
  layout-cache.ts \
  cluster-strategy.ts \
  graph-node.tsx \
  graph-group-node.tsx \
  graph-cluster-node.tsx \
  floating-edge.tsx \
  use-graph-layout.ts \
  use-graph-grouping.ts \
  layouts.ts \
  graph-filter-form.tsx \
  graph-filter-dialog.tsx
```

- [ ] **Step 2: Fix any remaining imports**

Search for imports of deleted modules and remove or redirect them. The main files that import from deleted modules are `graph-view.tsx`
(already rewritten) and `use-graph-nodes.ts` (already rewritten). Check for any other references:

```bash
cd apps/web && grep -r "from.*graph-node\|from.*graph-group-node\|from.*graph-cluster-node\|from.*floating-edge\|from.*force-layout\|from.*quadtree\|from.*layout\.worker\|from.*layout-cache\|from.*cluster-strategy\|from.*use-graph-layout\|from.*use-graph-grouping\|from.*layouts\|from.*graph-filter-form\|from.*graph-filter-dialog" src/ --include="*.ts" --include="*.tsx" -l
```

Fix any files that still reference deleted modules.

- [ ] **Step 3: Run type check**

```bash
pnpm run check-types
```

Expected: No errors from deleted imports (some type errors may remain from incomplete wiring — those are addressed in Task 12).

- [ ] **Step 4: Commit**

```bash
git add -A apps/web/src/features/graph/
git commit -m "refactor(graph): remove old layout, grouping, and node components"
```

---

## Task 12: Integration — Wire Health Scores and Polish

Connect health scores (currently placeholder Maps) to real data. Add the remaining UI polish: search overlay, heatmap toggle, edge legend.

**Files:**

- Modify: `apps/web/src/features/graph/graph-view.tsx`
- Modify: `apps/web/src/features/graph/use-graph-data.ts`

- [ ] **Step 1: Fetch chunk health data**

The graph API currently returns `chunks` without health scores. Health is computed on-demand via the chunk detail endpoint. For the graph,
add a batch health fetch (or compute client-side from available chunk metadata).

Add to `use-graph-data.ts` after the existing graph query:

```ts
// Add a query for chunk health scores (batch)
const { data: healthData } = useQuery({
    queryKey: ["graph-health", codebaseId, workspaceId],
    queryFn: async () => {
        // The knowledge health endpoint returns aggregate data
        // For per-chunk scores, we compute a simplified version client-side
        return null; // Will compute from chunk metadata
    },
    enabled: !!data
});

// Compute simplified health scores from chunk metadata
const chunkHealthScores = useMemo(() => {
    const scores = new Map<string, number>();
    if (!data?.chunks) return scores;
    const now = Date.now();
    for (const chunk of data.chunks as Array<{ id: string; createdAt: string }>) {
        const age = (now - new Date(chunk.createdAt).getTime()) / (1000 * 60 * 60 * 24);
        // Simplified: freshness only (0-100, decays over 180 days)
        const freshness = Math.max(0, Math.min(100, 100 - (age / 180) * 100));
        scores.set(chunk.id, Math.round(freshness));
    }
    return scores;
}, [data?.chunks]);

// Island health scores (aggregated from chunk scores)
const islandHealthScores = useMemo(() => {
    const map = new Map<string, number[]>();
    if (!islandData) return map;
    for (const island of islandData.islands) {
        map.set(
            island.id,
            island.chunkIds.map(id => chunkHealthScores.get(id) ?? 50)
        );
    }
    return map;
}, [islandData, chunkHealthScores]);
```

Return `chunkHealthScores` and `islandHealthScores` from the hook, and wire them into graph-view.tsx where the placeholder `new Map()`
currently is.

- [ ] **Step 2: Add search overlay to graph-view.tsx**

Add a search input in the top bar and wire it to highlight matching nodes:

```tsx
{
    /* Search bar — add after breadcrumbs */
}
<div className="absolute right-3 top-12 z-20 flex items-center gap-2">
    <input
        type="text"
        placeholder="Search chunks..."
        value={state.searchQuery}
        onChange={e => dispatch({ type: "SET_SEARCH_QUERY", query: e.target.value })}
        className="w-48 rounded border border-slate-600 bg-slate-800/80 px-2 py-1 text-xs text-slate-300 placeholder:text-slate-500"
    />
    {state.searchQuery && (
        <button onClick={() => dispatch({ type: "SET_SEARCH_QUERY", query: "" })} className="text-xs text-slate-500 hover:text-slate-300">
            ×
        </button>
    )}
</div>;
```

- [ ] **Step 3: Add heatmap toggle**

Add a toggle button next to the tag type picker:

```tsx
<button
    onClick={() => dispatch({ type: "TOGGLE_HEATMAP" })}
    className={`rounded border px-2 py-1 text-xs ${
        state.heatmapMode ? "border-amber-500 bg-amber-500/20 text-amber-300" : "border-slate-600 bg-slate-800 text-slate-400"
    }`}
>
    Health
</button>
```

When `heatmapMode` is true, island node borders and chunk card borders should use the health color instead of the type/tag color. Pass
`heatmapMode` through to the node data and adjust border colors in `graph-island-node.tsx` and `graph-chunk-card.tsx`.

- [ ] **Step 4: Wire path finding panel**

The spec requires path finding to be kept. The state already has `pathStartId`, `pathEndId`, and `showPathPanel`. Add the path panel UI from
the current implementation — simplified version showing start/end selection and path length. Use `findShortestPath` from `graph-utils.ts` to
compute paths across both overview and neighborhood levels.

- [ ] **Step 5: Add edge legend**

```tsx
{
    /* Edge legend — bottom right */
}
<div className="absolute bottom-3 right-3 z-20 flex gap-3 text-[8px] text-slate-500">
    <span>
        <span className="text-blue-400">━▸</span> depends_on
    </span>
    <span>
        <span className="text-green-400">━━</span> part_of
    </span>
    <span>
        <span className="text-purple-400">━━</span> extends
    </span>
    <span>
        <span className="text-red-400">╌╌</span> contradicts
    </span>
</div>;
```

- [ ] **Step 6: Update saved views format**

The saved views hook (`use-saved-views.ts`) stores `layoutAlgorithm` and `collapsedParents` which no longer exist. Update the `GraphView`
interface to store `groupingTagTypeId` and `heatmapMode` instead:

```ts
interface GraphView {
    name: string;
    filterTypes: string[];
    filterRelations: string[];
    groupingTagTypeId?: string;
    heatmapMode?: boolean;
    focusChunkId?: string;
}
```

Update `useSavedGraphViews` to read/write the new format. Old saved views with `layoutAlgorithm` should be silently migrated (ignore unknown
fields).

- [ ] **Step 7: Run the dev server and verify in browser**

```bash
pnpm dev
```

Open http://localhost:3001/graph and verify:

1. Overview shows islands with health dots
2. Clicking an island zooms to neighborhood
3. Focus chunk at center with neighbor cards around it
4. Edge colors/styles match relation types
5. Breadcrumbs work
6. Esc goes back
7. Search highlights matching chunks/islands
8. No console errors

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/features/graph/
git commit -m "feat(graph): wire health scores, search, heatmap, edge legend"
```

---

## Task 13: Run All Tests and Final Type Check

**Files:** None (verification only)

- [ ] **Step 1: Run graph tests**

```bash
cd apps/web && npx vitest run src/features/graph/
```

Expected: All island-formation, island-layout, and neighborhood-layout tests pass.

- [ ] **Step 2: Run full type check**

```bash
pnpm run check-types
```

Expected: No type errors.

- [ ] **Step 3: Run full test suite**

```bash
pnpm test
```

Expected: All existing tests still pass.

- [ ] **Step 4: Run CI pipeline**

```bash
pnpm ci
```

Expected: Type check, lint, test, build, format all pass.

- [ ] **Step 5: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix(graph): address type check and test issues from redesign"
```
