import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { chunkFileRef } from "../schema/file-ref";

describe("chunkFileRef table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkFileRef);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("path");
        expect(columns).toHaveProperty("anchor");
        expect(columns).toHaveProperty("relation");
    });
});
