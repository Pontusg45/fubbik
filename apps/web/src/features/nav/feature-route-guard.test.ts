import { describe, expect, it } from "vitest";

import { routeGuardState, shouldBlockRoute } from "./feature-route-guard";

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

describe("routeGuardState", () => {
    it("pending_gated_route_shows_loading", () => {
        expect(routeGuardState("/plans/abc", null, true)).toBe("loading");
    });
    it("pending_core_route_renders", () => {
        expect(routeGuardState("/chunks", null, true)).toBe("render");
    });
    it("resolved_disabled_route_is_blocked", () => {
        expect(routeGuardState("/plans", new Set(), false)).toBe("blocked");
    });
    it("failed_capabilities_render", () => {
        expect(routeGuardState("/plans", null, false)).toBe("render");
    });
});
