import { describe, expect, it } from "vitest";

import { createEdgeLogged, deleteEdgeLogged, deleteVertexLogged, ensureVertexLogged } from "../../age/sync-logged";

describe("sync-logged", () => {
    it("exports ensureVertexLogged", () => {
        expect(typeof ensureVertexLogged).toBe("function");
    });

    it("exports createEdgeLogged", () => {
        expect(typeof createEdgeLogged).toBe("function");
    });

    it("exports deleteVertexLogged", () => {
        expect(typeof deleteVertexLogged).toBe("function");
    });

    it("exports deleteEdgeLogged", () => {
        expect(typeof deleteEdgeLogged).toBe("function");
    });
});
