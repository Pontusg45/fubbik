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
        // Given the inline inputs and test fixtures.
        // When
        const result = formIslands(makeInput());
        // Then
        expect(result.islands).toHaveLength(3); // auth, api, ungrouped
        const auth = result.islands.find(i => i.name === "auth");
        expect(auth?.chunkIds).toEqual(["c1", "c2"]);
        const api = result.islands.find(i => i.name === "api");
        expect(api?.chunkIds).toEqual(["c3", "c4"]);
    });

    it("puts untagged chunks in an ungrouped island", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = formIslands(makeInput());
        const ungrouped = result.islands.find(i => i.id === "ungrouped");
        // Then
        expect(ungrouped?.chunkIds).toEqual(["c5"]);
    });

    it("computes bridge connections between islands", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = formIslands(makeInput());
        const bridge = result.bridges.find(
            b => (b.fromIslandId === "auth" && b.toIslandId === "api") || (b.fromIslandId === "api" && b.toIslandId === "auth")
        );
        // Then
        expect(bridge).toBeDefined();
        expect(bridge!.count).toBe(1); // c2→c3
    });

    it("marks single-chunk groups as singletons", () => {
        // Given
        const input = makeInput({
            chunks: [{ id: "c1", type: "note" }],
            connections: [],
            chunkTags: [{ chunkId: "c1", tagTypeId: "tt1", tagName: "lonely" }]
        });
        // When
        const result = formIslands(input);
        const lonely = result.islands.find(i => i.name === "lonely");
        // Then
        expect(lonely?.isSingleton).toBe(true);
    });

    it("assigns multi-tagged chunks to the first matching island with ghosts in others", () => {
        // Given
        const input = makeInput({
            chunkTags: [
                { chunkId: "c1", tagTypeId: "tt1", tagName: "auth" },
                { chunkId: "c1", tagTypeId: "tt1", tagName: "api" },
                { chunkId: "c2", tagTypeId: "tt1", tagName: "auth" }
            ]
        });
        // When
        const result = formIslands(input);
        const auth = result.islands.find(i => i.name === "auth");
        const api = result.islands.find(i => i.name === "api");
        // Then
        expect(auth?.chunkIds).toContain("c1");
        expect(api?.chunkIds).not.toContain("c1");
        expect(api?.ghostChunkIds).toContain("c1");
    });

    it("returns dominant relation type per bridge", () => {
        // Given
        const input = makeInput({
            connections: [
                { sourceId: "c1", targetId: "c3", relation: "depends_on" },
                { sourceId: "c2", targetId: "c3", relation: "depends_on" },
                { sourceId: "c2", targetId: "c4", relation: "references" }
            ]
        });
        // When
        const result = formIslands(input);
        const bridge = result.bridges.find(b => b.fromIslandId === "auth" && b.toIslandId === "api");
        // Then
        expect(bridge?.dominantRelation).toBe("depends_on");
        expect(bridge?.count).toBe(3);
    });
});
