import { describe, expect, it } from "vitest";

import { buildRecentPageItems } from "./command-items";

const pages = [
    { path: "/chunks", title: "Chunks" },
    { path: "/plans/abc", title: "A plan" }
] as Parameters<typeof buildRecentPageItems>[0];

describe("buildRecentPageItems", () => {
    it("recent_pages_hide_disabled_routes", () => {
        const items = buildRecentPageItems(pages, () => {}, new Set());
        expect(items.map(i => i.title)).toEqual(["Chunks"]);
    });
    it("keeps_all_when_capabilities_unknown", () => {
        expect(buildRecentPageItems(pages, () => {}, null)).toHaveLength(2);
    });
});
