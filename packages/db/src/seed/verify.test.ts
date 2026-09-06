import { describe, expect, it } from "vitest";

import { assertSeedIntegrity } from "./verify";

describe("assertSeedIntegrity", () => {
    it("rejects a seed containing orphaned rows", () => {
        expect(() =>
            assertSeedIntegrity([
                { label: "orphan chunk_tag rows", count: 2 },
                { label: "tasks without parent plan", count: 0 }
            ])
        ).toThrow("orphan chunk_tag rows: 2");
    });

    it("accepts a seed whose integrity probes are all empty", () => {
        expect(() => assertSeedIntegrity([{ label: "orphan chunk_tag rows", count: 0 }])).not.toThrow();
    });
});
