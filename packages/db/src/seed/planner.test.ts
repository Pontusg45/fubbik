import { describe, expect, it } from "vitest";

import { planModules, type SeedModuleDescriptor } from "./planner";

const registry: SeedModuleDescriptor[] = [
    { name: "core", deps: [], scenarios: ["minimal", "demo"] },
    { name: "tags", deps: ["core"], scenarios: ["demo"] },
    { name: "chunks", deps: ["core", "tags"], scenarios: ["demo"] },
    { name: "plans", deps: ["chunks"], scenarios: ["demo"] }
];

describe("planModules", () => {
    it("includes transitive dependencies for an only selection", () => {
        expect(planModules(registry, { scenario: "demo", only: new Set(["plans"]) })).toEqual([
            "core",
            "tags",
            "chunks",
            "plans"
        ]);
    });

    it("rejects skipping a dependency required by the scenario", () => {
        expect(() => planModules(registry, { scenario: "demo", skip: new Set(["tags"]) })).toThrow(
            "Cannot skip tags; required by chunks"
        );
    });

    it("topologically sorts independently of registry order", () => {
        expect(planModules([...registry].reverse(), { scenario: "demo" })).toEqual(["core", "tags", "chunks", "plans"]);
    });

    it("rejects dependency cycles with the participating module names", () => {
        const cyclic: SeedModuleDescriptor[] = [
            { name: "one", deps: ["two"], scenarios: ["demo"] },
            { name: "two", deps: ["one"], scenarios: ["demo"] }
        ];

        expect(() => planModules(cyclic, { scenario: "demo" })).toThrow("Seed module dependency cycle: one, two");
    });
});
