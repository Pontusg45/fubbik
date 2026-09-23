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
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorMatrix);
        // Then
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
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorDimension);
        // Then
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("matrixId");
        expect(cols).toHaveProperty("name");
        expect(cols).toHaveProperty("order");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorRule has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorRule);
        // Then
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
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorRule);
        // Then
        expect(cols).toHaveProperty("rationale");
        expect(cols).toHaveProperty("alternatives");
        expect(cols).toHaveProperty("consequences");
        expect(cols).toHaveProperty("counterexample");
    });

    it("behaviorRuleVersion has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorRuleVersion);
        // Then
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("ruleId");
        expect(cols).toHaveProperty("snapshot");
        expect(cols).toHaveProperty("changedBy");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorCellCode has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorCellCode);
        // Then
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("kind");
        expect(cols).toHaveProperty("ref");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorTestResult has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorTestResult);
        // Then
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("testRef");
        expect(cols).toHaveProperty("status");
        expect(cols).toHaveProperty("detail");
        expect(cols).toHaveProperty("runAt");
    });

    it("behaviorCell has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorCell);
        // Then
        expect(cols).toHaveProperty("id");
        expect(cols).toHaveProperty("ruleId");
        expect(cols).toHaveProperty("dimensionId");
        expect(cols).toHaveProperty("createdAt");
    });

    it("behaviorCellRequirement has expected columns", () => {
        // Given the inline inputs and test fixtures.
        // When
        const cols = getTableColumns(behaviorCellRequirement);
        // Then
        expect(cols).toHaveProperty("cellId");
        expect(cols).toHaveProperty("requirementId");
    });
});
