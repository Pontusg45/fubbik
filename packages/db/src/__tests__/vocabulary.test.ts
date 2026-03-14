import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { vocabularyEntry } from "../schema/vocabulary";

describe("vocabularyEntry table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(vocabularyEntry);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("word");
        expect(columns).toHaveProperty("category");
        expect(columns).toHaveProperty("expects");
        expect(columns).toHaveProperty("codebaseId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});
