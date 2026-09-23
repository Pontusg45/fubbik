import { describe, expect, it } from "vitest";

import { detectEmergentConcepts, listConcepts } from "../concepts/service";

describe("concepts service", () => {
    it("exports detectEmergentConcepts", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof detectEmergentConcepts).toBe("function");
    });

    it("exports listConcepts", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof listConcepts).toBe("function");
    });
});
