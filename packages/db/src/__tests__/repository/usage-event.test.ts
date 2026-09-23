import { describe, expect, it } from "vitest";

import { getCoReferenceCounts, getRecentUsageEvents, insertUsageEvent } from "../../repository/usage-event";

describe("usage-event repository", () => {
    it("exports insertUsageEvent", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof insertUsageEvent).toBe("function");
    });

    it("exports getRecentUsageEvents", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof getRecentUsageEvents).toBe("function");
    });

    it("exports getCoReferenceCounts", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof getCoReferenceCounts).toBe("function");
    });
});
