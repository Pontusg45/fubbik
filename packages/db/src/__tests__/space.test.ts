import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { space, chunkSpace } from "../schema/space";
import { spaceCodeMetadata } from "../schema/space-code-metadata";

describe("space table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(space);
        // Then
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("name");
        expect(columns).toHaveProperty("kind");
        expect(columns).toHaveProperty("description");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("spaceCodeMetadata table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(spaceCodeMetadata);
        // Then
        expect(columns).toHaveProperty("spaceId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("remoteUrl");
        expect(columns).toHaveProperty("localPaths");
    });
});

describe("chunkSpace table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(chunkSpace);
        // Then
        expect(columns).toHaveProperty("chunkId");
        expect(columns).toHaveProperty("spaceId");
    });
});
