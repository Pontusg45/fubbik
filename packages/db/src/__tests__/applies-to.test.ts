import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { chunkAppliesTo } from "../schema/applies-to";

describe("chunkAppliesTo table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkAppliesTo);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("pattern");
        expect(columns).toHaveProperty("note");
    });
});
