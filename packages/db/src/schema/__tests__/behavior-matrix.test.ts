import { describe, expect, it } from "vitest";
import { getTableColumns } from "drizzle-orm";
import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement
} from "../behavior-matrix";

describe("behavior-matrix schema", () => {
    it("behaviorMatrix has expected columns", () => {
        const cols = getTableColumns(behaviorMatrix);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("name");
        expect(cols).toHaveProperty("layer");
        expect(cols).toHaveProperty("description");
        expect(cols).toHaveProperty("codebaseId");
        expect(cols).toHaveProperty("userId");
        expect(cols).toHaveProperty("createdAt");
        expect(cols).toHaveProperty("updatedAt");
    });

    it("behaviorDimension has expected columns", () => {
        const cols = getTableColumns(behaviorDimension);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("matrixId");
        expect(cols).toHaveProperty("name");
        expect(cols).toHaveProperty("order");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorRule has expected columns", () => {
        const cols = getTableColumns(behaviorRule);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("matrixId");
        expect(cols).toHaveProperty("title");
        expect(cols).toHaveProperty("description");
        expect(cols).toHaveProperty("category");
        expect(cols).toHaveProperty("order");
        expect(cols).toHaveProperty("createdAt");
        expect(cols).toHaveProperty("updatedAt");
    });

    it("behaviorCell has expected columns", () => {
        const cols = getTableColumns(behaviorCell);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("ruleId");
        expect(cols).toHaveProperty("dimensionId");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorCellRequirement has expected columns", () => {
        const cols = getTableColumns(behaviorCellRequirement);
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("requirementId");
    });
});
