import { Effect } from "effect";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { isAgeAvailable } from "./client";
import {
    checkCircular,
    detectCommunities,
    findBridgeChunks,
    findShortestPath,
    findShortestPathWithDetails,
    getConnectionDegrees,
    getDownstreamChunks,
    getGraphProximityBoost,
    getNeighborhood,
    getOrphanChunkIds,
    getUpstreamChunks
} from "./query";
import { createEdge, deleteVertex, ensureVertex } from "./sync";

const PREFIX = "test-age-query-";

function uid(name: string) {
    return `${PREFIX}${name}`;
}

let ageReady = false;

beforeAll(async () => {
    ageReady = await isAgeAvailable();
    if (!ageReady) {
        console.log("AGE not available — skipping query tests");
        return;
    }

    // Create a linear chain: A → B → C (for path-finding and circular tests)
    await Effect.runPromise(ensureVertex("chunk", uid("A")));
    await Effect.runPromise(ensureVertex("chunk", uid("B")));
    await Effect.runPromise(ensureVertex("chunk", uid("C")));
    await Effect.runPromise(createEdge("connects", "chunk", uid("A"), "chunk", uid("B"), { relation: "related_to" }));
    await Effect.runPromise(createEdge("connects", "chunk", uid("B"), "chunk", uid("C"), { relation: "related_to" }));

    // Create a star: center + 3 spokes (for neighborhood test)
    await Effect.runPromise(ensureVertex("chunk", uid("center")));
    await Effect.runPromise(ensureVertex("chunk", uid("spoke1")));
    await Effect.runPromise(ensureVertex("chunk", uid("spoke2")));
    await Effect.runPromise(ensureVertex("chunk", uid("spoke3")));
    await Effect.runPromise(createEdge("connects", "chunk", uid("center"), "chunk", uid("spoke1"), { relation: "related_to" }));
    await Effect.runPromise(createEdge("connects", "chunk", uid("center"), "chunk", uid("spoke2"), { relation: "related_to" }));
    await Effect.runPromise(createEdge("connects", "chunk", uid("center"), "chunk", uid("spoke3"), { relation: "related_to" }));

    // Isolated chunk (for orphan test)
    await Effect.runPromise(ensureVertex("chunk", uid("orphan")));

    // Two routes to the same target: one direct and one through an intermediate node.
    await Effect.runPromise(ensureVertex("chunk", uid("short")));
    await Effect.runPromise(ensureVertex("chunk", uid("via")));
    await Effect.runPromise(ensureVertex("chunk", uid("target")));
    await Effect.runPromise(createEdge("connects", "chunk", uid("short"), "chunk", uid("target")));
    await Effect.runPromise(createEdge("connects", "chunk", uid("short"), "chunk", uid("via")));
    await Effect.runPromise(createEdge("connects", "chunk", uid("via"), "chunk", uid("target")));

    // Requirement chain for circular detection: reqA → reqB → reqC (depends_on edges)
    // checkCircular(reqC, reqA) should return true (reqA depends transitively on reqC)
    await Effect.runPromise(ensureVertex("requirement", uid("reqA")));
    await Effect.runPromise(ensureVertex("requirement", uid("reqB")));
    await Effect.runPromise(ensureVertex("requirement", uid("reqC")));
    await Effect.runPromise(createEdge("depends_on", "requirement", uid("reqA"), "requirement", uid("reqB")));
    await Effect.runPromise(createEdge("depends_on", "requirement", uid("reqB"), "requirement", uid("reqC")));
});

afterAll(async () => {
    if (!ageReady) return;
    const chunkIds = [
        uid("A"),
        uid("B"),
        uid("C"),
        uid("center"),
        uid("spoke1"),
        uid("spoke2"),
        uid("spoke3"),
        uid("orphan"),
        uid("short"),
        uid("via"),
        uid("target")
    ];
    for (const id of chunkIds) {
        await Effect.runPromise(deleteVertex("chunk", id));
    }
    const reqIds = [uid("reqA"), uid("reqB"), uid("reqC")];
    for (const id of reqIds) {
        await Effect.runPromise(deleteVertex("requirement", id));
    }
});

describe("findShortestPath", () => {
    it("finds path between connected chunks (A connected to C via B)", async () => {
        // Given
        if (!ageReady) return;
        // When
        const path = await Effect.runPromise(findShortestPath(uid("A"), uid("C")));
        // Then
        expect(path).not.toBeNull();
        expect(path).toContain(uid("A"));
        expect(path).toContain(uid("C"));
    });

    it("returns null when no path exists between unconnected chunks", async () => {
        // Given
        if (!ageReady) return;
        // When
        const path = await Effect.runPromise(findShortestPath(uid("A"), uid("orphan")));
        // Then
        expect(path).toBeNull();
    });
});

describe("getNeighborhood", () => {
    it("returns all direct neighbors of the center at hop depth 1", async () => {
        // Given
        if (!ageReady) return;
        // When
        const neighbors = await Effect.runPromise(getNeighborhood(uid("center"), 1));
        // Then
        expect(neighbors.length).toBe(3);
        expect(neighbors).toContain(uid("spoke1"));
        expect(neighbors).toContain(uid("spoke2"));
        expect(neighbors).toContain(uid("spoke3"));
    });

    it("returns empty array for isolated chunk", async () => {
        // Given
        if (!ageReady) return;
        // When
        const neighbors = await Effect.runPromise(getNeighborhood(uid("orphan"), 1));
        // Then
        expect(neighbors.length).toBe(0);
    });
});

describe("getOrphanChunkIds", () => {
    it("includes isolated chunk in orphan list", async () => {
        // Given
        if (!ageReady) return;
        // When
        const orphans = await Effect.runPromise(getOrphanChunkIds());
        // Then
        expect(orphans).toContain(uid("orphan"));
    });

    it("does not include connected chunks in orphan list", async () => {
        // Given
        if (!ageReady) return;
        // When
        const orphans = await Effect.runPromise(getOrphanChunkIds());
        // Then
        expect(orphans).not.toContain(uid("A"));
        expect(orphans).not.toContain(uid("center"));
    });
});

describe("checkCircular", () => {
    it("detects that adding reqC→reqA would be circular (reqA already leads to reqC)", async () => {
        // Given
        if (!ageReady) return;
        // When
        // Chain is reqA→reqB→reqC via depends_on.
        // checkCircular(requirementId, dependsOnId):
        //   "does dependsOnId have a path to requirementId?"
        // checkCircular(reqC, reqA) = does reqA have a depends_on path to reqC? → true
        const isCircular = await Effect.runPromise(checkCircular(uid("reqC"), uid("reqA")));
        // Then
        expect(isCircular).toBe(true);
    });

    it("returns false for non-circular direction", async () => {
        // Given
        if (!ageReady) return;
        // When
        // checkCircular(reqA, reqC) = does reqC have a depends_on path to reqA? → false
        const isCircular = await Effect.runPromise(checkCircular(uid("reqA"), uid("reqC")));
        // Then
        expect(isCircular).toBe(false);
    });
});

describe("getConnectionDegrees", () => {
    it("returns correct degree for chain and star nodes", async () => {
        // Given
        if (!ageReady) return;
        const ids = [uid("A"), uid("B"), uid("C"), uid("center")];
        // When
        const map = await Effect.runPromise(getConnectionDegrees(ids));
        // Then
        expect(map.get(uid("B"))).toBe(2);
        expect(map.get(uid("center"))).toBe(3);
        expect(map.get(uid("A"))).toBe(1);
        expect(map.get(uid("C"))).toBe(1);
    });

    it("omits orphan nodes with no edges", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getConnectionDegrees([uid("orphan")]));
        // Then
        expect(map.get(uid("orphan"))).toBeUndefined();
    });

    it("returns empty map for empty input", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getConnectionDegrees([]));
        // Then
        expect(map.size).toBe(0);
    });
});

describe("getGraphProximityBoost", () => {
    it("uses the shortest route when multiple paths reach the target", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getGraphProximityBoost(uid("short"), [uid("target")], 3));
        // Then
        expect(map.get(uid("target"))).toBe(1);
    });

    it("respects the hop limit", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getGraphProximityBoost(uid("A"), [uid("B"), uid("C")], 1));
        // Then
        expect(map.get(uid("B"))).toBe(1);
        expect(map.has(uid("C"))).toBe(false);
    });
    it("returns higher boost for closer nodes", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getGraphProximityBoost(uid("A"), [uid("B"), uid("C")], 3));
        // Then
        // B is 1 hop from A → boost 1/1 = 1
        // C is 2 hops from A → boost 1/2 = 0.5
        expect(map.get(uid("B"))).toBeGreaterThan(map.get(uid("C"))!);
    });

    it("omits unreachable nodes", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getGraphProximityBoost(uid("A"), [uid("B"), uid("orphan")], 3));
        // Then
        expect(map.has(uid("B"))).toBe(true);
        expect(map.has(uid("orphan"))).toBe(false);
    });

    it("returns empty map when anchor has no connections", async () => {
        // Given
        if (!ageReady) return;
        // When
        const map = await Effect.runPromise(getGraphProximityBoost(uid("orphan"), [uid("A"), uid("B")], 3));
        // Then
        expect(map.size).toBe(0);
    });
});

describe("getDownstreamChunks", () => {
    it("returns downstream nodes following directed edges", async () => {
        // Given
        if (!ageReady) return;
        // When
        // Chain: A → B → C (directed connects edges)
        const downstream = await Effect.runPromise(getDownstreamChunks(uid("A"), 3));
        // Then
        expect(downstream).toContain(uid("B"));
        expect(downstream).toContain(uid("C"));
    });

    it("returns empty array for leaf node with no outgoing edges", async () => {
        // Given
        if (!ageReady) return;
        // When
        const downstream = await Effect.runPromise(getDownstreamChunks(uid("C"), 3));
        // Then
        expect(downstream.length).toBe(0);
    });

    it("returns empty array for orphan node", async () => {
        // Given
        if (!ageReady) return;
        // When
        const downstream = await Effect.runPromise(getDownstreamChunks(uid("orphan"), 3));
        // Then
        expect(downstream.length).toBe(0);
    });
});

describe("detectCommunities", () => {
    it("finds at least 2 communities among all test chunks", async () => {
        // Given
        if (!ageReady) return;
        const allIds = [uid("A"), uid("B"), uid("C"), uid("center"), uid("spoke1"), uid("spoke2"), uid("spoke3"), uid("orphan")];
        // When
        const communities = await Effect.runPromise(detectCommunities(allIds, 3));
        // Then
        expect(communities.length).toBeGreaterThanOrEqual(2);
    });

    it("star cluster has 4 members", async () => {
        // Given
        if (!ageReady) return;
        const allIds = [uid("A"), uid("B"), uid("C"), uid("center"), uid("spoke1"), uid("spoke2"), uid("spoke3"), uid("orphan")];
        // When
        const communities = await Effect.runPromise(detectCommunities(allIds, 3));
        const starCluster = communities.find(c => c.members.includes(uid("center")));
        // Then
        expect(starCluster).toBeDefined();
        expect(starCluster!.members.length).toBe(4);
    });

    it("excludes orphan from all communities (singleton)", async () => {
        // Given
        if (!ageReady) return;
        const allIds = [uid("A"), uid("B"), uid("C"), uid("center"), uid("spoke1"), uid("spoke2"), uid("spoke3"), uid("orphan")];
        // When
        const communities = await Effect.runPromise(detectCommunities(allIds, 3));
        const orphanCommunity = communities.find(c => c.members.includes(uid("orphan")));
        // Then
        expect(orphanCommunity).toBeUndefined();
    });

    it("returns empty array for empty input", async () => {
        // Given
        if (!ageReady) return;
        // When
        const communities = await Effect.runPromise(detectCommunities([], 3));
        // Then
        expect(communities.length).toBe(0);
    });
});

describe("findBridgeChunks", () => {
    it("identifies B as a bridge in chain A-B-C", async () => {
        // Given
        if (!ageReady) return;
        // When
        const bridges = await Effect.runPromise(findBridgeChunks([uid("A"), uid("B"), uid("C")]));
        // Then
        expect(bridges).toContain(uid("B"));
    });

    it("identifies center as a bridge in star topology", async () => {
        // Given
        if (!ageReady) return;
        // When
        const bridges = await Effect.runPromise(findBridgeChunks([uid("center"), uid("spoke1"), uid("spoke2"), uid("spoke3")]));
        // Then
        expect(bridges).toContain(uid("center"));
    });

    it("does not identify leaf nodes as bridges", async () => {
        // Given
        if (!ageReady) return;
        // When
        const bridges = await Effect.runPromise(findBridgeChunks([uid("A"), uid("B"), uid("C")]));
        // Then
        expect(bridges).not.toContain(uid("A"));
        expect(bridges).not.toContain(uid("C"));
    });

    it("returns empty array for empty input", async () => {
        // Given
        if (!ageReady) return;
        // When
        const bridges = await Effect.runPromise(findBridgeChunks([]));
        // Then
        expect(bridges.length).toBe(0);
    });
});

describe("findShortestPathWithDetails", () => {
    it("returns full path from A to C with intermediate nodes and edges", async () => {
        // Given
        if (!ageReady) return;
        // When
        const result = await Effect.runPromise(findShortestPathWithDetails(uid("A"), uid("C")));
        // Then
        expect(result).not.toBeNull();
        expect(result!.nodes).toHaveLength(3);
        expect(result!.nodes[0]).toBe(uid("A"));
        expect(result!.nodes[1]).toBe(uid("B"));
        expect(result!.nodes[2]).toBe(uid("C"));
        expect(result!.edges).toHaveLength(2);
        expect(result!.edges[0]!.relation).toBe("related_to");
        expect(result!.hops).toBe(2);
    });

    it("returns direct path from A to B", async () => {
        // Given
        if (!ageReady) return;
        // When
        const result = await Effect.runPromise(findShortestPathWithDetails(uid("A"), uid("B")));
        // Then
        expect(result).not.toBeNull();
        expect(result!.nodes).toHaveLength(2);
        expect(result!.edges).toHaveLength(1);
        expect(result!.hops).toBe(1);
    });

    it("returns null when no path exists", async () => {
        // Given
        if (!ageReady) return;
        // When
        const result = await Effect.runPromise(findShortestPathWithDetails(uid("A"), uid("orphan")));
        // Then
        expect(result).toBeNull();
    });
});

describe("getUpstreamChunks", () => {
    it("returns upstream nodes for C (B and A point to C via chain)", async () => {
        // Given
        if (!ageReady) return;
        // When
        const upstream = await Effect.runPromise(getUpstreamChunks(uid("C"), 3));
        // Then
        expect(upstream).toContain(uid("B"));
        expect(upstream).toContain(uid("A"));
    });

    it("returns empty array for root node A (no incoming edges)", async () => {
        // Given
        if (!ageReady) return;
        // When
        const upstream = await Effect.runPromise(getUpstreamChunks(uid("A"), 3));
        // Then
        expect(upstream.length).toBe(0);
    });

    it("returns empty array for orphan node", async () => {
        // Given
        if (!ageReady) return;
        // When
        const upstream = await Effect.runPromise(getUpstreamChunks(uid("orphan"), 3));
        // Then
        expect(upstream.length).toBe(0);
    });
});
