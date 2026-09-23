import { describe, expect, it } from "vitest";

import { createEdgeLogged, deleteEdgeLogged, deleteVertexLogged, ensureVertexLogged } from "../../age/sync-logged";

describe("sync-logged", () => {
    it("exports ensureVertexLogged", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof ensureVertexLogged).toBe("function");
    });

    it("exports createEdgeLogged", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof createEdgeLogged).toBe("function");
    });

    it("exports deleteVertexLogged", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof deleteVertexLogged).toBe("function");
    });

    it("exports deleteEdgeLogged", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof deleteEdgeLogged).toBe("function");
    });
});
