import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { rmSync } from "node:fs";
import { discover } from "../../lib/setup/discover";

const TMP_DIR = join(import.meta.dirname, "__tmp_discover__");

beforeEach(() => { mkdirSync(TMP_DIR, { recursive: true }); });
afterEach(() => { rmSync(TMP_DIR, { recursive: true, force: true }); });

describe("discover", () => {
	it("combines all tiers into a single result", () => {
		writeFileSync(join(TMP_DIR, "README.md"), "# My App\n\nHello world");
		writeFileSync(join(TMP_DIR, "package.json"), JSON.stringify({
			name: "my-app",
			dependencies: { react: "^18.0.0" },
			devDependencies: { vitest: "^1.0.0" },
		}));
		mkdirSync(join(TMP_DIR, "src", "__tests__"), { recursive: true });
		writeFileSync(join(TMP_DIR, "src", "__tests__", "app.test.ts"), "test('works', () => {})");

		const result = discover(TMP_DIR, { name: "my-app", remoteUrl: null, localPath: TMP_DIR });

		expect(result.chunks.length).toBeGreaterThanOrEqual(3);
		expect(result.chunks.some(c => c.tier === 1)).toBe(true);
		expect(result.chunks.some(c => c.tier === 2)).toBe(true);
		expect(result.chunks.some(c => c.tier === 3)).toBe(true);
		expect(result.tags.length).toBeGreaterThan(0);
		expect(result.codebase.name).toBe("my-app");
	});

	it("returns empty result for empty directory", () => {
		const result = discover(TMP_DIR, { name: "empty", remoteUrl: null, localPath: TMP_DIR });
		expect(result.chunks).toEqual([]);
		expect(result.connections).toEqual([]);
		expect(result.tips).toEqual([]);
	});
});
