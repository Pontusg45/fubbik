import { describe, expect, it } from "vitest";

import { layoutNeighborhood } from "./neighborhood-layout";

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
