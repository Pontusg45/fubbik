import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { requirement, requirementChunk } from "../schema/requirement";

describe("requirement table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(requirement);
        // Then
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("title");
        expect(columns).toHaveProperty("description");
        expect(columns).toHaveProperty("steps");
        expect(columns).toHaveProperty("status");
        expect(columns).toHaveProperty("priority");
        expect(columns).toHaveProperty("spaceId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("requirementChunk table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(requirementChunk);
        // Then
        expect(columns).toHaveProperty("requirementId");
        expect(columns).toHaveProperty("chunkId");
    });
});
