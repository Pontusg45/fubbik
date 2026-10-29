import { describe, expect, it } from "vitest";

import { shouldBlockRoute } from "./feature-route-guard";

describe("shouldBlockRoute", () => {
    it("guard_blocks_disabled_route", () => {
        expect(shouldBlockRoute("/plans", new Set(["requirements"]))).toBe(true);
    });
    it("guard_allows_when_capabilities_unknown", () => {
        expect(shouldBlockRoute("/plans", null)).toBe(false);
    });
    it("guard_allows_core_routes", () => {
        expect(shouldBlockRoute("/chunks", new Set())).toBe(false);
        expect(shouldBlockRoute("/dashboard", new Set())).toBe(false);
    });
    it("guard_blocks_nested_route", () => {
        expect(shouldBlockRoute("/plans/abc", new Set())).toBe(true);
    });
    it("guard_allows_enabled_route", () => {
        expect(shouldBlockRoute("/plans/abc", new Set(["plans"]))).toBe(false);
    });
});
