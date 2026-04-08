import { Effect } from "effect";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { isAgeAvailable } from "./client";
import {
    checkCircular,
    findShortestPath,
    getNeighborhood,
    getOrphanChunkIds
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
    await Effect.runPromise(
        createEdge("connects", "chunk", uid("A"), "chunk", uid("B"), { relation: "related_to" })
    );
    await Effect.runPromise(
        createEdge("connects", "chunk", uid("B"), "chunk", uid("C"), { relation: "related_to" })
    );

    // Create a star: center + 3 spokes (for neighborhood test)
    await Effect.runPromise(ensureVertex("chunk", uid("center")));
    await Effect.runPromise(ensureVertex("chunk", uid("spoke1")));
    await Effect.runPromise(ensureVertex("chunk", uid("spoke2")));
    await Effect.runPromise(ensureVertex("chunk", uid("spoke3")));
    await Effect.runPromise(
        createEdge("connects", "chunk", uid("center"), "chunk", uid("spoke1"), { relation: "related_to" })
    );
    await Effect.runPromise(
        createEdge("connects", "chunk", uid("center"), "chunk", uid("spoke2"), { relation: "related_to" })
    );
    await Effect.runPromise(
        createEdge("connects", "chunk", uid("center"), "chunk", uid("spoke3"), { relation: "related_to" })
    );

    // Isolated chunk (for orphan test)
    await Effect.runPromise(ensureVertex("chunk", uid("orphan")));

    // Requirement chain for circular detection: reqA → reqB → reqC (depends_on edges)
    // checkCircular(reqC, reqA) should return true (reqA depends transitively on reqC)
    await Effect.runPromise(ensureVertex("requirement", uid("reqA")));
    await Effect.runPromise(ensureVertex("requirement", uid("reqB")));
    await Effect.runPromise(ensureVertex("requirement", uid("reqC")));
    await Effect.runPromise(
        createEdge("depends_on", "requirement", uid("reqA"), "requirement", uid("reqB"))
    );
    await Effect.runPromise(
        createEdge("depends_on", "requirement", uid("reqB"), "requirement", uid("reqC"))
    );
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
        uid("orphan")
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
        if (!ageReady) return;
        const path = await Effect.runPromise(findShortestPath(uid("A"), uid("C")));
        expect(path).not.toBeNull();
        expect(path).toContain(uid("A"));
        expect(path).toContain(uid("C"));
    });

    it("returns null when no path exists between unconnected chunks", async () => {
        if (!ageReady) return;
        const path = await Effect.runPromise(findShortestPath(uid("A"), uid("orphan")));
        expect(path).toBeNull();
    });
});

describe("getNeighborhood", () => {
    it("returns all direct neighbors of the center at hop depth 1", async () => {
        if (!ageReady) return;
        const neighbors = await Effect.runPromise(getNeighborhood(uid("center"), 1));
        expect(neighbors.length).toBe(3);
        expect(neighbors).toContain(uid("spoke1"));
        expect(neighbors).toContain(uid("spoke2"));
        expect(neighbors).toContain(uid("spoke3"));
    });

    it("returns empty array for isolated chunk", async () => {
        if (!ageReady) return;
        const neighbors = await Effect.runPromise(getNeighborhood(uid("orphan"), 1));
        expect(neighbors.length).toBe(0);
    });
});

describe("getOrphanChunkIds", () => {
    it("includes isolated chunk in orphan list", async () => {
        if (!ageReady) return;
        const orphans = await Effect.runPromise(getOrphanChunkIds());
        expect(orphans).toContain(uid("orphan"));
    });

    it("does not include connected chunks in orphan list", async () => {
        if (!ageReady) return;
        const orphans = await Effect.runPromise(getOrphanChunkIds());
        expect(orphans).not.toContain(uid("A"));
        expect(orphans).not.toContain(uid("center"));
    });
});

describe("checkCircular", () => {
    it("detects that adding reqC→reqA would be circular (reqA already leads to reqC)", async () => {
        if (!ageReady) return;
        // Chain is reqA→reqB→reqC via depends_on.
        // checkCircular(requirementId, dependsOnId):
        //   "does dependsOnId have a path to requirementId?"
        // checkCircular(reqC, reqA) = does reqA have a depends_on path to reqC? → true
        const isCircular = await Effect.runPromise(checkCircular(uid("reqC"), uid("reqA")));
        expect(isCircular).toBe(true);
    });

    it("returns false for non-circular direction", async () => {
        if (!ageReady) return;
        // checkCircular(reqA, reqC) = does reqC have a depends_on path to reqA? → false
        const isCircular = await Effect.runPromise(checkCircular(uid("reqA"), uid("reqC")));
        expect(isCircular).toBe(false);
    });
});
