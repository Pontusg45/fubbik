import {
    createMatrix as createMatrixRepo,
    getMatrixById,
    listMatrices as listMatricesRepo,
    updateMatrix as updateMatrixRepo,
    deleteMatrix as deleteMatrixRepo,
    createDimension as createDimensionRepo,
    updateDimension as updateDimensionRepo,
    deleteDimension as deleteDimensionRepo,
    getDimensionsForMatrix,
    getMaxDimensionOrder,
    reorderDimensions as reorderDimensionsRepo,
    createRule as createRuleRepo,
    updateRule as updateRuleRepo,
    deleteRule as deleteRuleRepo,
    getRulesForMatrix,
    getMaxRuleOrder,
    reorderRules as reorderRulesRepo,
    getCellByRuleDimension,
    createCell as createCellRepo,
    deleteCell as deleteCellRepo,
    getCellRequirementCount,
    linkCellRequirement as linkCellRequirementRepo,
    unlinkCellRequirement as unlinkCellRequirementRepo,
    getRequirementsForCell as getRequirementsForCellRepo,
    getMatrixView
} from "@fubbik/db/repository";
import { Effect } from "effect";

import type { DatabaseError } from "@fubbik/db/errors";
import { NotFoundError, ValidationError } from "../errors";

// --- Matrix ---

export function createMatrix(userId: string, body: {
    name: string;
    layer: string;
    description?: string;
    codebaseId?: string;
}): Effect.Effect<{
    id: string;
    name: string;
    layer: string;
    description: string | null;
    codebaseId: string | null;
    userId: string;
    createdAt: Date;
    updatedAt: Date;
}, ValidationError | DatabaseError> {
    if (body.layer !== "invariant" && body.layer !== "contract") {
        return Effect.fail(new ValidationError({ message: "Layer must be 'invariant' or 'contract'" }));
    }
    const id = crypto.randomUUID();
    return createMatrixRepo({ id, ...body, userId });
}

export function getMatrixDetail(matrixId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(matrix =>
            Effect.all({
                matrix: Effect.succeed(matrix),
                dimensions: getDimensionsForMatrix(matrixId),
                rules: getRulesForMatrix(matrixId)
            })
        )
    );
}

export function listMatrices(userId: string, filters?: { codebaseId?: string; layer?: string }) {
    return listMatricesRepo(userId, filters);
}

export function updateMatrix(matrixId: string, userId: string, body: { name?: string; description?: string | null }) {
    return updateMatrixRepo(matrixId, userId, body).pipe(
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Matrix" }))))
    );
}

export function deleteMatrixService(matrixId: string, userId: string) {
    return deleteMatrixRepo(matrixId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Matrix" }))))
    );
}

// --- Dimensions ---

export function addDimension(matrixId: string, userId: string, body: { name: string }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getMaxDimensionOrder(matrixId)),
        Effect.flatMap(maxOrder => createDimensionRepo({
            id: crypto.randomUUID(),
            matrixId,
            name: body.name,
            order: maxOrder + 1
        }))
    );
}

export function renameDimension(matrixId: string, dimId: string, userId: string, body: { name: string }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => updateDimensionRepo(dimId, matrixId, body)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Dimension" }))))
    );
}

export function removeDimension(matrixId: string, dimId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => deleteDimensionRepo(dimId, matrixId)),
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Dimension" }))))
    );
}

export function reorderDimensions(matrixId: string, userId: string, dimensionIds: string[]) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => reorderDimensionsRepo(dimensionIds)),
        Effect.map(() => ({ message: "Reordered" }))
    );
}

// --- Rules ---

export function addRule(matrixId: string, userId: string, body: { title: string; description?: string; category?: string }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getMaxRuleOrder(matrixId)),
        Effect.flatMap(maxOrder => createRuleRepo({
            id: crypto.randomUUID(),
            matrixId,
            title: body.title,
            description: body.description,
            category: body.category,
            order: maxOrder + 1
        }))
    );
}

export function updateRule(matrixId: string, ruleId: string, userId: string, body: { title?: string; description?: string | null; category?: string | null }) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => updateRuleRepo(ruleId, matrixId, body)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Rule" }))))
    );
}

export function removeRule(matrixId: string, ruleId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => deleteRuleRepo(ruleId, matrixId)),
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Rule" }))))
    );
}

export function reorderRules(matrixId: string, userId: string, ruleIds: string[]) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => reorderRulesRepo(ruleIds)),
        Effect.map(() => ({ message: "Reordered" }))
    );
}

// --- Cells ---

export interface ToggleCellResult {
    action: "created" | "deleted";
    cell: { id: string; ruleId: string; dimensionId: string; createdAt: Date };
}

export function toggleCell(
    ruleId: string,
    dimensionId: string
): Effect.Effect<ToggleCellResult, DatabaseError | ValidationError> {
    return getCellByRuleDimension(ruleId, dimensionId).pipe(
        Effect.flatMap((existing): Effect.Effect<ToggleCellResult, DatabaseError | ValidationError> => {
            if (!existing) {
                return createCellRepo({ id: crypto.randomUUID(), ruleId, dimensionId }).pipe(
                    Effect.map(cell => ({ action: "created" as const, cell }))
                );
            }
            return getCellRequirementCount(existing.id).pipe(
                Effect.flatMap((count): Effect.Effect<ToggleCellResult, ValidationError | DatabaseError> => {
                    if (count > 0) {
                        return Effect.fail(new ValidationError({
                            message: `Cell has ${count} linked requirement(s). Unlink them first.`
                        }));
                    }
                    return deleteCellRepo(existing.id).pipe(
                        Effect.map(() => ({ action: "deleted" as const, cell: existing }))
                    );
                })
            );
        })
    );
}

export function linkRequirementToCell(cellId: string, requirementId: string) {
    return linkCellRequirementRepo(cellId, requirementId);
}

export function unlinkRequirementFromCell(cellId: string, requirementId: string) {
    return unlinkCellRequirementRepo(cellId, requirementId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Cell-Requirement link" }))))
    );
}

export function getRequirementsForCell(cellId: string) {
    return getRequirementsForCellRepo(cellId);
}

// --- Matrix View ---

type CellStatus = "specified" | "unspecified" | "violated";

export interface ViewCell {
    id: string;
    status: CellStatus;
    requirementCount: number;
}

export function getMatrixViewService(matrixId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(matrix =>
            getMatrixView(matrixId).pipe(
                Effect.map(({ dimensions, rules, cells }) => {
                    const cellMap: Record<string, ViewCell | null> = {};
                    let specified = 0;
                    let unspecified = 0;
                    let violated = 0;

                    for (const cell of cells) {
                        const key = `${cell.ruleId}:${cell.dimensionId}`;
                        let status: CellStatus;
                        if (cell.failingCount > 0) {
                            status = "violated";
                            violated++;
                        } else if (cell.requirementCount > 0) {
                            status = "specified";
                            specified++;
                        } else {
                            status = "unspecified";
                            unspecified++;
                        }
                        cellMap[key] = { id: cell.id, status, requirementCount: cell.requirementCount };
                    }

                    return {
                        matrix,
                        dimensions,
                        rules,
                        cells: cellMap,
                        summary: {
                            specified,
                            unspecified,
                            violated,
                            total: specified + unspecified + violated
                        }
                    };
                })
            )
        )
    );
}
