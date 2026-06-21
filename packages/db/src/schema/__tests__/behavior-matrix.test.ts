import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";

import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement,
    behaviorRuleVersion,
    behaviorCellCode,
    behaviorTestResult
} from "../behavior-matrix";

describe("behavior-matrix schema", () => {
    it("behaviorMatrix has expected columns", () => {
        const cols = getTableColumns(behaviorMatrix);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("name");
        expect(cols).toHaveProperty("layer");
        expect(cols).toHaveProperty("description");
        expect(cols).toHaveProperty("spaceId");
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

    it("behaviorRule has the decision-context columns", () => {
        const cols = getTableColumns(behaviorRule);
        expect(cols).toHaveProperty("rationale");
        expect(cols).toHaveProperty("alternatives");
        expect(cols).toHaveProperty("consequences");
        expect(cols).toHaveProperty("counterexample");
    });

    it("behaviorRuleVersion has expected columns", () => {
        const cols = getTableColumns(behaviorRuleVersion);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("ruleId");
        expect(cols).toHaveProperty("snapshot");
        expect(cols).toHaveProperty("changedBy");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorCellCode has expected columns", () => {
        const cols = getTableColumns(behaviorCellCode);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("kind");
        expect(cols).toHaveProperty("ref");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorTestResult has expected columns", () => {
        const cols = getTableColumns(behaviorTestResult);
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("testRef");
        expect(cols).toHaveProperty("status");
        expect(cols).toHaveProperty("detail");
        expect(cols).toHaveProperty("runAt");
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
