import { describe, expect, it } from "vitest";

import { detectEmergentConcepts, listConcepts } from "../concepts/service";

describe("concepts service", () => {
    it("exports detectEmergentConcepts", () => {
        expect(typeof detectEmergentConcepts).toBe("function");
    });

    it("exports listConcepts", () => {
        expect(typeof listConcepts).toBe("function");
    });
});
