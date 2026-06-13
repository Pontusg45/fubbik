import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import { graphEvent } from "../../schema/graph-event";

describe("graphEvent table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(graphEvent);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("vertexLabel");
        expect(columns).toHaveProperty("vertexId");
        expect(columns).toHaveProperty("edgeType");
        expect(columns).toHaveProperty("edgeTargetId");
        expect(columns).toHaveProperty("action");
        expect(columns).toHaveProperty("snapshot");
        expect(columns).toHaveProperty("createdAt");
    });
});
