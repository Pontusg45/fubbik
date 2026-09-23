import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { usageEvent } from "../../schema/usage-event";

describe("usageEvent table", () => {
    it("has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const columns = getTableColumns(usageEvent);
        // Then
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("kind");
        expect(columns).toHaveProperty("chunkIds");
        expect(columns).toHaveProperty("query");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
    });
});
