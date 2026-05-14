import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@fubbik/db/repository", () => ({
    createMatrix: vi.fn(),
    getMatrixById: vi.fn(),
    listMatrices: vi.fn(),
    updateMatrix: vi.fn(),
    deleteMatrix: vi.fn(),
    createDimension: vi.fn(),
    updateDimension: vi.fn(),
    deleteDimension: vi.fn(),
    getDimensionsForMatrix: vi.fn(),
    getMaxDimensionOrder: vi.fn(),
    reorderDimensions: vi.fn(),
    createRule: vi.fn(),
    updateRule: vi.fn(),
    deleteRule: vi.fn(),
    getRulesForMatrix: vi.fn(),
    getMaxRuleOrder: vi.fn(),
    reorderRules: vi.fn(),
    getCellByRuleDimension: vi.fn(),
    createCell: vi.fn(),
    deleteCell: vi.fn(),
    getCellRequirementCount: vi.fn(),
    linkCellRequirement: vi.fn(),
    unlinkCellRequirement: vi.fn(),
    getRequirementsForCell: vi.fn(),
    getMatrixView: vi.fn()
}));

import {
    createMatrix as createMatrixRepo,
    getMatrixById,
    getDimensionsForMatrix,
    getMaxDimensionOrder,
    createDimension as createDimensionRepo,
    getRulesForMatrix,
    getMaxRuleOrder,
    createRule as createRuleRepo,
    getCellByRuleDimension,
    createCell as createCellRepo,
    deleteCell as deleteCellRepo,
    getCellRequirementCount,
    unlinkCellRequirement as unlinkCellRequirementRepo,
    getMatrixView
} from "@fubbik/db/repository";

import * as service from "./service";

function mockMatrix(overrides?: Record<string, unknown>) {
    return {
        id: "mat-1",
        name: "Domain Invariants",
        layer: "invariant",
        description: null,
        codebaseId: null,
        userId: "user-1",
        createdAt: new Date(),
        updatedAt: new Date(),
        ...overrides
    };
}

beforeEach(() => vi.clearAllMocks());

describe("createMatrix", () => {
    it("creates a matrix and returns it", async () => {
        const mat = mockMatrix();
        vi.mocked(createMatrixRepo).mockReturnValue(Effect.succeed(mat));

        const result = await Effect.runPromise(service.createMatrix("user-1", {
            name: "Domain Invariants",
            layer: "invariant"
        }));

        expect(result).toMatchObject({ name: "Domain Invariants", layer: "invariant" });
        expect(createMatrixRepo).toHaveBeenCalledOnce();
    });

    it("rejects invalid layer values", async () => {
        await expect(Effect.runPromise(service.createMatrix("user-1", {
            name: "Bad Matrix",
            layer: "invalid"
        }))).rejects.toThrow();
    });
});

describe("getMatrixDetail", () => {
    it("returns matrix with dimensions and rules", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(mockMatrix()) as any);
        vi.mocked(getDimensionsForMatrix).mockReturnValue(Effect.succeed([{ id: "dim-1", name: "Chunk" }]) as any);
        vi.mocked(getRulesForMatrix).mockReturnValue(Effect.succeed([{ id: "rule-1", title: "Cascade deletes" }]) as any);

        const result = await Effect.runPromise(service.getMatrixDetail("mat-1", "user-1"));

        expect(result.matrix.name).toBe("Domain Invariants");
        expect(result.dimensions).toHaveLength(1);
        expect(result.rules).toHaveLength(1);
    });

    it("fails with NotFoundError for missing matrix", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(null) as any);

        await expect(Effect.runPromise(service.getMatrixDetail("nope", "user-1"))).rejects.toThrow();
    });
});

describe("addDimension", () => {
    it("creates dimension with auto-incremented order", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(mockMatrix()) as any);
        vi.mocked(getMaxDimensionOrder).mockReturnValue(Effect.succeed(2) as any);
        vi.mocked(createDimensionRepo).mockReturnValue(Effect.succeed({ id: "dim-new", name: "API", order: 3 }) as any);

        const result = await Effect.runPromise(service.addDimension("mat-1", "user-1", { name: "API" }));

        expect(result.name).toBe("API");
        expect(createDimensionRepo).toHaveBeenCalledWith(
            expect.objectContaining({ matrixId: "mat-1", name: "API", order: 3 })
        );
    });

    it("fails when matrix not found", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(null) as any);

        await expect(Effect.runPromise(service.addDimension("nope", "user-1", { name: "API" }))).rejects.toThrow();
    });
});

describe("addRule", () => {
    it("creates rule with auto-incremented order", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(mockMatrix()) as any);
        vi.mocked(getMaxRuleOrder).mockReturnValue(Effect.succeed(1) as any);
        vi.mocked(createRuleRepo).mockReturnValue(Effect.succeed({ id: "rule-new", title: "No orphans", order: 2 }) as any);

        const result = await Effect.runPromise(service.addRule("mat-1", "user-1", { title: "No orphans" }));

        expect(result.title).toBe("No orphans");
        expect(createRuleRepo).toHaveBeenCalledWith(
            expect.objectContaining({ matrixId: "mat-1", title: "No orphans", order: 2 })
        );
    });
});

describe("toggleCell", () => {
    it("creates cell when none exists", async () => {
        vi.mocked(getCellByRuleDimension).mockReturnValue(Effect.succeed(null) as any);
        vi.mocked(createCellRepo).mockReturnValue(Effect.succeed({ id: "cell-1", ruleId: "rule-1", dimensionId: "dim-1" }) as any);

        const result = await Effect.runPromise(service.toggleCell("rule-1", "dim-1"));

        expect(result).toMatchObject({ action: "created" });
        expect(createCellRepo).toHaveBeenCalledOnce();
    });

    it("deletes cell with no linked requirements", async () => {
        vi.mocked(getCellByRuleDimension).mockReturnValue(Effect.succeed({ id: "cell-1" }) as any);
        vi.mocked(getCellRequirementCount).mockReturnValue(Effect.succeed(0) as any);
        vi.mocked(deleteCellRepo).mockReturnValue(Effect.succeed({ id: "cell-1" }) as any);

        const result = await Effect.runPromise(service.toggleCell("rule-1", "dim-1"));

        expect(result).toMatchObject({ action: "deleted" });
    });

    it("fails when cell has linked requirements", async () => {
        vi.mocked(getCellByRuleDimension).mockReturnValue(Effect.succeed({ id: "cell-1" }) as any);
        vi.mocked(getCellRequirementCount).mockReturnValue(Effect.succeed(2) as any);

        await expect(Effect.runPromise(service.toggleCell("rule-1", "dim-1"))).rejects.toThrow();
    });
});

describe("unlinkRequirementFromCell", () => {
    it("returns deleted link", async () => {
        vi.mocked(unlinkCellRequirementRepo).mockReturnValue(Effect.succeed({ cellId: "cell-1", requirementId: "req-1" }) as any);

        const result = await Effect.runPromise(service.unlinkRequirementFromCell("cell-1", "req-1"));

        expect(result).toMatchObject({ cellId: "cell-1", requirementId: "req-1" });
    });

    it("fails with NotFoundError when link not found", async () => {
        vi.mocked(unlinkCellRequirementRepo).mockReturnValue(Effect.succeed(null) as any);

        await expect(Effect.runPromise(service.unlinkRequirementFromCell("cell-1", "req-1"))).rejects.toThrow();
    });
});

describe("getMatrixView", () => {
    it("computes cell statuses correctly", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(mockMatrix()) as any);
        vi.mocked(getMatrixView).mockReturnValue(Effect.succeed({
            dimensions: [{ id: "dim-1", name: "Chunk", order: 0 }],
            rules: [{ id: "rule-1", title: "Cascade", category: null, order: 0 }],
            cells: [
                { id: "c1", ruleId: "rule-1", dimensionId: "dim-1", requirementCount: 2, failingCount: 0 },
                { id: "c2", ruleId: "rule-1", dimensionId: "dim-2", requirementCount: 0, failingCount: 0 },
                { id: "c3", ruleId: "rule-1", dimensionId: "dim-3", requirementCount: 3, failingCount: 1 }
            ]
        }) as any);

        const result = await Effect.runPromise(service.getMatrixViewService("mat-1", "user-1"));

        const cells = result.cells;
        expect(cells["rule-1:dim-1"]?.status).toBe("specified");
        expect(cells["rule-1:dim-2"]?.status).toBe("unspecified");
        expect(cells["rule-1:dim-3"]?.status).toBe("violated");
        expect(result.summary.specified).toBe(1);
        expect(result.summary.unspecified).toBe(1);
        expect(result.summary.violated).toBe(1);
        expect(result.summary.total).toBe(3);
    });

    it("fails with NotFoundError for missing matrix", async () => {
        vi.mocked(getMatrixById).mockReturnValue(Effect.succeed(null) as any);

        await expect(Effect.runPromise(service.getMatrixViewService("nope", "user-1"))).rejects.toThrow();
    });
});
