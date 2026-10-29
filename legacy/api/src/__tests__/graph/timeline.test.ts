import { describe, expect, it } from "vitest";

import { reconstructGraphAt } from "../../graph/timeline-service";

describe("timeline service", () => {
    it("exports reconstructGraphAt", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof reconstructGraphAt).toBe("function");
    });
});
