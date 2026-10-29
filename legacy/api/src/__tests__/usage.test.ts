import { describe, expect, it } from "vitest";

import { recordUsage, aggregateCoReferences } from "../usage/service";

describe("usage service", () => {
    it("exports recordUsage", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof recordUsage).toBe("function");
    });

    it("exports aggregateCoReferences", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof aggregateCoReferences).toBe("function");
    });
});
