import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { codebase, chunkCodebase } from "../schema/codebase";

describe("codebase table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(codebase);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("name");
        expect(columns).toHaveProperty("remoteUrl");
        expect(columns).toHaveProperty("localPaths");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("chunkCodebase table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkCodebase);
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("codebaseId");
    });
});
