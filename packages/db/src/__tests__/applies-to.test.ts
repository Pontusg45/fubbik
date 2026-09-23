import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { chunkAppliesTo } from "../schema/applies-to";

describe("chunkAppliesTo table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(chunkAppliesTo);
        // Then
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("pattern");
        expect(columns).toHaveProperty("note");
    });
});
