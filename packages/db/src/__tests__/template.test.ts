import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { chunkTemplate } from "../schema/template";

describe("chunkTemplate table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkTemplate);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("name");
        expect(columns).toHaveProperty("description");
        expect(columns).toHaveProperty("type");
        expect(columns).toHaveProperty("content");
        expect(columns).toHaveProperty("isBuiltIn");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
    });
});
