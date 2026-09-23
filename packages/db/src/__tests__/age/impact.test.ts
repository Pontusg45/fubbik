import { describe, expect, it } from "vitest";

import { computeImpactRipple } from "../../age/impact";
import type { ImpactTarget } from "../../age/impact";

describe("impact", () => {
    it("exports computeImpactRipple", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(typeof computeImpactRipple).toBe("function");
    });

    it("ImpactTarget type has expected shape", () => {
        // Given
        const target: ImpactTarget = {
            chunkId: "test",
            degree: 0.9,
            hops: 1,
            path: ["depends_on"]
        };
        // When the operation is evaluated by the assertion.
        // Then
        expect(target.chunkId).toBe("test");
        expect(target.degree).toBe(0.9);
    });
});
