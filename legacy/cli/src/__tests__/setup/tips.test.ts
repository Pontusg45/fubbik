import { describe, expect, it } from "vitest";

import { mergeTips } from "../../lib/setup/tips";
import type { Tip } from "../../lib/setup/types";

describe("mergeTips", () => {
    it("deduplicates tips with the same title", () => {
        // Given
        const tips: Tip[] = [
            { title: "Testing Patterns", detail: "vitest dep found" },
            { title: "Testing Patterns", detail: "test files found" }
        ];
        // When
        const merged = mergeTips(tips);
        // Then
        expect(merged.length).toBe(1);
        expect(merged[0]!.title).toBe("Testing Patterns");
    });

    it("keeps tips with different titles", () => {
        // Given
        const tips: Tip[] = [
            { title: "Testing Patterns", detail: "vitest found" },
            { title: "Database Patterns", detail: "drizzle found" }
        ];
        // When
        const merged = mergeTips(tips);
        // Then
        expect(merged.length).toBe(2);
    });

    it("returns empty for empty input", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(mergeTips([])).toEqual([]);
    });
});
