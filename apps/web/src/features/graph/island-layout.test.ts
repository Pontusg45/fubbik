import { describe, expect, it } from "vitest";

import { layoutIslands, type IslandLayoutInput } from "./island-layout";

describe("layoutIslands", () => {
    it("assigns positions to all islands", () => {
        // Given
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
        // When
        const positions = layoutIslands(input);
        // Then
        expect(Object.keys(positions)).toHaveLength(3);
        expect(positions.auth).toHaveProperty("x");
        expect(positions.auth).toHaveProperty("y");
    });

    it("places connected islands closer than unconnected ones", () => {
        // Given
        const input: IslandLayoutInput = {
            islands: [
                { id: "a", chunkCount: 5 },
                { id: "b", chunkCount: 5 },
                { id: "c", chunkCount: 5 }
            ],
            bridges: [{ fromIslandId: "a", toIslandId: "b", count: 10 }]
        };
        const positions = layoutIslands(input);
        const distAB = Math.hypot(positions.a!.x - positions.b!.x, positions.a!.y - positions.b!.y);
        // When
        const distAC = Math.hypot(positions.a!.x - positions.c!.x, positions.a!.y - positions.c!.y);
        // Then
        expect(distAB).toBeLessThan(distAC);
    });

    it("returns empty object for empty input", () => {
        // Given the inline inputs and test fixtures.
        // When
        const positions = layoutIslands({ islands: [], bridges: [] });
        // Then
        expect(positions).toEqual({});
    });

    it("handles single island", () => {
        // Given the inline inputs and test fixtures.
        // When
        const positions = layoutIslands({
            islands: [{ id: "solo", chunkCount: 3 }],
            bridges: []
        });
        // Then
        expect(positions.solo).toEqual({ x: 0, y: 0 });
    });
});
