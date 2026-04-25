import { describe, expect, it } from "vitest";
import { mergeTips } from "../../lib/setup/tips";
import type { Tip } from "../../lib/setup/types";

describe("mergeTips", () => {
	it("deduplicates tips with the same title", () => {
		const tips: Tip[] = [
			{ title: "Testing Patterns", detail: "vitest dep found" },
			{ title: "Testing Patterns", detail: "test files found" },
		];
		const merged = mergeTips(tips);
		expect(merged.length).toBe(1);
		expect(merged[0]!.title).toBe("Testing Patterns");
	});

	it("keeps tips with different titles", () => {
		const tips: Tip[] = [
			{ title: "Testing Patterns", detail: "vitest found" },
			{ title: "Database Patterns", detail: "drizzle found" },
		];
		const merged = mergeTips(tips);
		expect(merged.length).toBe(2);
	});

	it("returns empty for empty input", () => {
		expect(mergeTips([])).toEqual([]);
	});
});
