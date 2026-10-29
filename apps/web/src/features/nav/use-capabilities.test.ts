import { describe, expect, it } from "vitest";

import { filterNavItems } from "./use-capabilities";

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
