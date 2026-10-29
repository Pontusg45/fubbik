import { describe, expect, it } from "vitest";

import { filterNavItems } from "./navigation-items";

const items = [
    { label: "Chunks", to: "/chunks" },
    { label: "Plans", to: "/plans", feature: "plans" },
    { label: "Matrices", to: "/matrices", feature: "matrices" }
];

describe("filterNavItems", () => {
    it("keeps_untagged_items", () => {
        expect(filterNavItems(items, new Set()).map(i => i.label)).toEqual(["Chunks"]);
    });

    it("hides_items_whose_feature_is_disabled", () => {
        expect(filterNavItems(items, new Set(["plans"])).map(i => i.label)).toEqual(["Chunks", "Plans"]);
    });

    it("nav_falls_back_to_all_items_when_capabilities_fail", () => {
        expect(filterNavItems(items, null)).toEqual(items);
    });
});
