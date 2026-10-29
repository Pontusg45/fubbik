import { describe, expect, it } from "vitest";

import { fetchCapabilities, filterNavItems, isFeatureEnabled } from "./use-capabilities";

const items = [
    { label: "Chunks", to: "/chunks" },
    { label: "Plans", to: "/plans" },
    { label: "New plan", to: "/plans/new" },
    { label: "Matrices", to: "/matrices" }
];

describe("filterNavItems", () => {
    it("keeps_untagged_items", () => {
        expect(filterNavItems(items, new Set()).map(i => i.label)).toEqual(["Chunks"]);
    });

    it("hides_items_whose_feature_is_disabled", () => {
        expect(filterNavItems(items, new Set(["plans"])).map(i => i.label)).toEqual(["Chunks", "Plans", "New plan"]);
    });

    it("maps_nested_routes_to_their_feature", () => {
        expect(filterNavItems(items, new Set(["matrices"])).map(i => i.to)).not.toContain("/plans/new");
    });

    it("nav_falls_back_to_all_items_when_capabilities_fail", () => {
        expect(filterNavItems(items, null)).toEqual(items);
    });
});

describe("fetchCapabilities", () => {
    it("capabilities_failure_resolves_to_null_without_throwing", async () => {
        const notFound = () => Promise.resolve({ data: null, error: { status: 404, value: "Not Found" } });
        const networkDown = () => Promise.reject(new TypeError("Failed to fetch"));

        await expect(fetchCapabilities(notFound)).resolves.toBeNull();
        await expect(fetchCapabilities(networkDown)).resolves.toBeNull();
    });

    it("returns_the_enabled_feature_set", async () => {
        const ok = () => Promise.resolve({ data: { features: ["plans", "ai"] }, error: null });

        await expect(fetchCapabilities(ok)).resolves.toEqual(new Set(["plans", "ai"]));
    });
});

describe("isFeatureEnabled", () => {
    it("is_null_when_capabilities_are_unknown", () => {
        expect(isFeatureEnabled(null, "plans")).toBeNull();
    });

    it("reports_known_switches", () => {
        expect(isFeatureEnabled(new Set(["plans"]), "plans")).toBe(true);
        expect(isFeatureEnabled(new Set(["plans"]), "overlays")).toBe(false);
    });
});
