import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { vocabularyEntry } from "../schema/vocabulary";

describe("vocabularyEntry table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(vocabularyEntry);
        // Then
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("word");
        expect(columns).toHaveProperty("category");
        expect(columns).toHaveProperty("expects");
        expect(columns).toHaveProperty("spaceId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});
