import { describe, expect, it } from "vitest";

import { getCoReferenceCounts, getRecentUsageEvents, insertUsageEvent } from "../../repository/usage-event";

describe("usage-event repository", () => {
    it("exports insertUsageEvent", () => {
        expect(typeof insertUsageEvent).toBe("function");
    });

    it("exports getRecentUsageEvents", () => {
        expect(typeof getRecentUsageEvents).toBe("function");
    });

    it("exports getCoReferenceCounts", () => {
        expect(typeof getCoReferenceCounts).toBe("function");
    });
});
