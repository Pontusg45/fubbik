import type { DatabaseError } from "@fubbik/db/errors";
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
    getRuleById,
    getRulesForMatrix,
    getMaxRuleOrder,
    reorderRules as reorderRulesRepo,
    insertRuleVersion as insertRuleVersionRepo,
    getRuleVersions as getRuleVersionsRepo,
    getCellByRuleDimension,
    getCellInMatrix,
    ruleAndDimensionInMatrix,
    createCell as createCellRepo,
    deleteCell as deleteCellRepo,
    getCellRequirementCount,
    linkCellRequirement as linkCellRequirementRepo,
    unlinkCellRequirement as unlinkCellRequirementRepo,
    getRequirementsForCell as getRequirementsForCellRepo,
    linkCellCode as linkCellCodeRepo,
    deleteCellCode as deleteCellCodeRepo,
    getCodeForCell as getCodeForCellRepo,
    getBehaviorsForCodePath as getBehaviorsForCodePathRepo,
    recordTestResult as recordTestResultRepo,
    getTestResultsForCell as getTestResultsForCellRepo,
    getMatrixView
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

// --- Matrix ---

export function createMatrix(
    userId: string,
    body: {
        name: string;
        layer: string;
        description?: string;
        spaceId?: string;
    }
): Effect.Effect<
    {
        id: string;
        name: string;
        layer: string;
        description: string | null;
        spaceId: string | null;
        userId: string;
        createdAt: Date;
        updatedAt: Date;
    },
    ValidationError | DatabaseError
> {
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

export function listMatrices(userId: string, filters?: { spaceId?: string; layer?: string }) {
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
        Effect.flatMap(maxOrder =>
            createDimensionRepo({
                id: crypto.randomUUID(),
                matrixId,
                name: body.name,
                order: maxOrder + 1
            })
        )
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

export interface RuleWhyFields {
    rationale?: string;
    alternatives?: string;
    consequences?: string;
    counterexample?: string;
}

export function addRule(
    matrixId: string,
    userId: string,
    body: { title: string; description?: string; category?: string } & RuleWhyFields
) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getMaxRuleOrder(matrixId)),
        Effect.flatMap(maxOrder =>
            createRuleRepo({
                id: crypto.randomUUID(),
                matrixId,
                title: body.title,
                description: body.description,
                category: body.category,
                rationale: body.rationale,
                alternatives: body.alternatives,
                consequences: body.consequences,
                counterexample: body.counterexample,
                order: maxOrder + 1
            })
        )
    );
}

export function updateRule(
    matrixId: string,
    ruleId: string,
    userId: string,
    body: {
        title?: string;
        description?: string | null;
        category?: string | null;
        rationale?: string | null;
        alternatives?: string | null;
        consequences?: string | null;
        counterexample?: string | null;
    }
) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getRuleById(ruleId, matrixId)),
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Rule" })))),
        // Snapshot the pre-edit state into append-only history before mutating.
        Effect.tap(existing =>
            insertRuleVersionRepo({
                id: crypto.randomUUID(),
                ruleId,
                changedBy: userId,
                snapshot: {
                    title: existing.title,
                    description: existing.description,
                    category: existing.category,
                    rationale: existing.rationale,
                    alternatives: existing.alternatives,
                    consequences: existing.consequences,
                    counterexample: existing.counterexample
                }
            })
        ),
        Effect.flatMap(() => updateRuleRepo(ruleId, matrixId, body)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Rule" }))))
    );
}

export function getRuleHistory(matrixId: string, ruleId: string, userId: string) {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getRuleVersionsRepo(ruleId))
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

/**
 * SECURITY: `matrixId`/`userId` are not decorative. This used to take only
 * `ruleId` and `dimensionId` — both attacker-supplied body fields — while the
 * route's `:id` matrix param went unread and the session was discarded. Any
 * authenticated user could toggle cells in anyone's matrix.
 *
 * Both ends are now checked: the matrix must belong to the caller, and the
 * rule and dimension must both belong to that matrix.
 */
export function toggleCell(
    matrixId: string,
    userId: string,
    ruleId: string,
    dimensionId: string
): Effect.Effect<ToggleCellResult, DatabaseError | ValidationError | NotFoundError> {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => ruleAndDimensionInMatrix(ruleId, dimensionId, matrixId)),
        Effect.flatMap(ok => (ok ? Effect.succeed(ok) : Effect.fail(new NotFoundError({ resource: "Rule or dimension" })))),
        Effect.flatMap(() => toggleCellUnscoped(ruleId, dimensionId))
    );
}

function toggleCellUnscoped(ruleId: string, dimensionId: string): Effect.Effect<ToggleCellResult, DatabaseError | ValidationError> {
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
                        return Effect.fail(
                            new ValidationError({
                                message: `Cell has ${count} linked requirement(s). Unlink them first.`
                            })
                        );
                    }
                    return deleteCellRepo(existing.id).pipe(Effect.map(() => ({ action: "deleted" as const, cell: existing })));
                })
            );
        })
    );
}

/**
 * Proves the cell belongs to a matrix the caller owns, then runs `next`.
 *
 * SECURITY: every function below used to take a bare `cellId` with no
 * ownership check anywhere in the chain — the route discarded the session and
 * ignored its own `:id` param. That allowed cross-user reads AND writes:
 * attaching your requirement to a stranger's behavior cell, deleting their
 * code links, or reading what they had linked.
 */
function withOwnedCell<A, E>(
    matrixId: string,
    cellId: string,
    userId: string,
    next: (cell: { id: string; ruleId: string; dimensionId: string; createdAt: Date }) => Effect.Effect<A, E>
): Effect.Effect<A, E | NotFoundError | DatabaseError> {
    return getMatrixById(matrixId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Matrix" })))),
        Effect.flatMap(() => getCellInMatrix(cellId, matrixId)),
        Effect.flatMap(cell => (cell ? Effect.succeed(cell) : Effect.fail(new NotFoundError({ resource: "Cell" })))),
        Effect.flatMap(next)
    );
}

export function linkRequirementToCell(matrixId: string, cellId: string, userId: string, requirementId: string) {
    return withOwnedCell(matrixId, cellId, userId, () => linkCellRequirementRepo(cellId, requirementId));
}

export function unlinkRequirementFromCell(matrixId: string, cellId: string, userId: string, requirementId: string) {
    return withOwnedCell(matrixId, cellId, userId, () =>
        unlinkCellRequirementRepo(cellId, requirementId).pipe(
            Effect.flatMap(deleted =>
                deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Cell-Requirement link" }))
            )
        )
    );
}

export function getRequirementsForCell(matrixId: string, cellId: string, userId: string) {
    return withOwnedCell(matrixId, cellId, userId, () => getRequirementsForCellRepo(cellId));
}

// --- Cell Code Links ---

const CODE_LINK_KINDS = ["file", "symbol", "test"] as const;

export function linkCodeToCell(matrixId: string, cellId: string, userId: string, body: { kind: string; ref: string }) {
    return withOwnedCell(matrixId, cellId, userId, () =>
        Effect.gen(function* () {
            if (!CODE_LINK_KINDS.includes(body.kind as (typeof CODE_LINK_KINDS)[number])) {
                return yield* Effect.fail(
                    new ValidationError({ message: `Code link kind must be one of: ${CODE_LINK_KINDS.join(", ")}` })
                );
            }
            if (!body.ref.trim()) {
                return yield* Effect.fail(new ValidationError({ message: "Code link ref is required" }));
            }
            return yield* linkCellCodeRepo({ id: crypto.randomUUID(), cellId, kind: body.kind, ref: body.ref.trim() });
        })
    );
}

export function unlinkCodeFromCell(matrixId: string, cellId: string, userId: string, codeId: string) {
    return withOwnedCell(matrixId, cellId, userId, () =>
        deleteCellCodeRepo(codeId, cellId).pipe(
            Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Code link" }))))
        )
    );
}

export function getCodeForCell(matrixId: string, cellId: string, userId: string) {
    return withOwnedCell(matrixId, cellId, userId, () => getCodeForCellRepo(cellId));
}

export function getBehaviorsForCodePath(userId: string, path: string) {
    return getBehaviorsForCodePathRepo(userId, path);
}

// --- Cell Test Results ---

export function recordTestResult(
    matrixId: string,
    cellId: string,
    userId: string,
    body: { testRef: string; status: string; detail?: string }
) {
    return withOwnedCell(matrixId, cellId, userId, () =>
        Effect.gen(function* () {
            if (body.status !== "pass" && body.status !== "fail") {
                return yield* Effect.fail(new ValidationError({ message: "Test status must be 'pass' or 'fail'" }));
            }
            return yield* recordTestResultRepo({
                id: crypto.randomUUID(),
                cellId,
                testRef: body.testRef,
                status: body.status,
                detail: body.detail
            });
        })
    );
}

export function getTestResultsForCell(matrixId: string, cellId: string, userId: string) {
    return withOwnedCell(matrixId, cellId, userId, () => getTestResultsForCellRepo(cellId));
}

// --- Matrix View ---

type CellStatus = "specified" | "unspecified" | "violated" | "verified";

export interface ViewCell {
    id: string;
    status: CellStatus;
    requirementCount: number;
    codeCount: number;
    passingTestCount: number;
    failingTestCount: number;
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
                    let verified = 0;

                    for (const cell of cells) {
                        const key = `${cell.ruleId}:${cell.dimensionId}`;
                        let status: CellStatus;
                        // A failing requirement or a failing test means the behavior is violated.
                        if (cell.failingCount > 0 || cell.failingTestCount > 0) {
                            status = "violated";
                            violated++;
                        } else if (cell.passingTestCount > 0) {
                            // Real, passing test evidence is the strongest signal.
                            status = "verified";
                            verified++;
                        } else if (cell.requirementCount > 0) {
                            status = "specified";
                            specified++;
                        } else {
                            status = "unspecified";
                            unspecified++;
                        }
                        cellMap[key] = {
                            id: cell.id,
                            status,
                            requirementCount: cell.requirementCount,
                            codeCount: cell.codeCount,
                            passingTestCount: cell.passingTestCount,
                            failingTestCount: cell.failingTestCount
                        };
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
                            verified,
                            total: specified + unspecified + violated + verified
                        }
                    };
                })
            )
        )
    );
}
