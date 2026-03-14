import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { requirement, requirementChunk } from "../schema/requirement";

describe("requirement table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(requirement);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("title");
        expect(columns).toHaveProperty("description");
        expect(columns).toHaveProperty("steps");
        expect(columns).toHaveProperty("status");
        expect(columns).toHaveProperty("priority");
        expect(columns).toHaveProperty("codebaseId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("requirementChunk table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(requirementChunk);
        expect(columns).toHaveProperty("requirementId");
        expect(columns).toHaveProperty("chunkId");
    });
});
