import { describe, expect, it } from "vitest";

import { indexDirectory, indexFile } from "../code-index/service";

describe("code-index service", () => {
    it("exports indexFile", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof indexFile).toBe("function");
    });

    it("exports indexDirectory", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof indexDirectory).toBe("function");
    });
});
