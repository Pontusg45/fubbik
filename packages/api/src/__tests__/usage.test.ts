import { describe, expect, it } from "vitest";
import { recordUsage, aggregateCoReferences } from "../usage/service";

describe("usage service", () => {
    it("exports recordUsage", () => {
        expect(typeof recordUsage).toBe("function");
    });

    it("exports aggregateCoReferences", () => {
        expect(typeof aggregateCoReferences).toBe("function");
    });
});
