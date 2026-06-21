import { and, eq, sql, inArray, asc, desc } from "drizzle-orm";

import { db, dbEffect } from "../index";
import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement,
    behaviorCellCode,
    behaviorTestResult,
    behaviorRuleVersion,
    type BehaviorRuleSnapshot
} from "../schema/behavior-matrix";
import { requirement } from "../schema/requirement";

// --- Matrix CRUD ---

export function createMatrix(params: { id: string; name: string; layer: string; description?: string; spaceId?: string; userId: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorMatrix).values(params).returning();
        if (!created) throw new Error("createMatrix: insert returned no row");
        return created;
    });
}

export function getMatrixById(id: string, userId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(behaviorMatrix)
            .where(and(eq(behaviorMatrix.id, id), eq(behaviorMatrix.userId, userId)));
        return found ?? null;
    });
}

export function listMatrices(userId: string, filters?: { spaceId?: string; layer?: string }) {
    return dbEffect(async () => {
        const conditions = [eq(behaviorMatrix.userId, userId)];
        if (filters?.spaceId) conditions.push(eq(behaviorMatrix.spaceId, filters.spaceId));
        if (filters?.layer) conditions.push(eq(behaviorMatrix.layer, filters.layer));

        return db
            .select()
            .from(behaviorMatrix)
            .where(and(...conditions))
            .orderBy(asc(behaviorMatrix.name));
    });
}

export function updateMatrix(id: string, userId: string, data: { name?: string; description?: string | null }) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorMatrix)
            .set(data)
            .where(and(eq(behaviorMatrix.id, id), eq(behaviorMatrix.userId, userId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteMatrix(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorMatrix)
            .where(and(eq(behaviorMatrix.id, id), eq(behaviorMatrix.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

// --- Dimension CRUD ---

export function createDimension(params: { id: string; matrixId: string; name: string; order: number }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorDimension).values(params).returning();
        if (!created) throw new Error("createDimension: insert returned no row");
        return created;
    });
}

export function updateDimension(id: string, matrixId: string, data: { name?: string }) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorDimension)
            .set(data)
            .where(and(eq(behaviorDimension.id, id), eq(behaviorDimension.matrixId, matrixId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteDimension(id: string, matrixId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorDimension)
            .where(and(eq(behaviorDimension.id, id), eq(behaviorDimension.matrixId, matrixId)))
            .returning();
        return deleted ?? null;
    });
}

export function getDimensionsForMatrix(matrixId: string) {
    return dbEffect(() =>
        db.select().from(behaviorDimension).where(eq(behaviorDimension.matrixId, matrixId)).orderBy(asc(behaviorDimension.order))
    );
}

export function getMaxDimensionOrder(matrixId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ max: sql<number>`coalesce(max(${behaviorDimension.order}), -1)::int` })
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId));
        return result?.max ?? -1;
    });
}

export function reorderDimensions(dimensionIds: string[]) {
    return dbEffect(async () => {
        for (let i = 0; i < dimensionIds.length; i++) {
            const dimId = dimensionIds[i];
            if (!dimId) continue;
            await db.update(behaviorDimension).set({ order: i }).where(eq(behaviorDimension.id, dimId));
        }
    });
}

// --- Rule CRUD ---

export function createRule(params: {
    id: string;
    matrixId: string;
    title: string;
    description?: string;
    category?: string;
    rationale?: string;
    alternatives?: string;
    consequences?: string;
    counterexample?: string;
    order: number;
}) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorRule).values(params).returning();
        if (!created) throw new Error("createRule: insert returned no row");
        return created;
    });
}

export function getRuleById(id: string, matrixId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(behaviorRule)
            .where(and(eq(behaviorRule.id, id), eq(behaviorRule.matrixId, matrixId)));
        return found ?? null;
    });
}

export function updateRule(
    id: string,
    matrixId: string,
    data: {
        title?: string;
        description?: string | null;
        category?: string | null;
        rationale?: string | null;
        alternatives?: string | null;
        consequences?: string | null;
        counterexample?: string | null;
    }
) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorRule)
            .set(data)
            .where(and(eq(behaviorRule.id, id), eq(behaviorRule.matrixId, matrixId)))
            .returning();
        return updated ?? null;
    });
}

// --- Rule version history (append-only) ---

export function insertRuleVersion(params: { id: string; ruleId: string; snapshot: BehaviorRuleSnapshot; changedBy?: string | null }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorRuleVersion).values(params).returning();
        if (!created) throw new Error("insertRuleVersion: insert returned no row");
        return created;
    });
}

export function getRuleVersions(ruleId: string) {
    return dbEffect(() =>
        db.select().from(behaviorRuleVersion).where(eq(behaviorRuleVersion.ruleId, ruleId)).orderBy(desc(behaviorRuleVersion.createdAt))
    );
}

export function deleteRule(id: string, matrixId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorRule)
            .where(and(eq(behaviorRule.id, id), eq(behaviorRule.matrixId, matrixId)))
            .returning();
        return deleted ?? null;
    });
}

export function getRulesForMatrix(matrixId: string) {
    return dbEffect(() => db.select().from(behaviorRule).where(eq(behaviorRule.matrixId, matrixId)).orderBy(asc(behaviorRule.order)));
}

export function getMaxRuleOrder(matrixId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ max: sql<number>`coalesce(max(${behaviorRule.order}), -1)::int` })
            .from(behaviorRule)
            .where(eq(behaviorRule.matrixId, matrixId));
        return result?.max ?? -1;
    });
}

export function reorderRules(ruleIds: string[]) {
    return dbEffect(async () => {
        for (let i = 0; i < ruleIds.length; i++) {
            const ruleId = ruleIds[i];
            if (!ruleId) continue;
            await db.update(behaviorRule).set({ order: i }).where(eq(behaviorRule.id, ruleId));
        }
    });
}

// --- Cell CRUD ---

export function getCellByRuleDimension(ruleId: string, dimensionId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(behaviorCell)
            .where(and(eq(behaviorCell.ruleId, ruleId), eq(behaviorCell.dimensionId, dimensionId)));
        return found ?? null;
    });
}

export function createCell(params: { id: string; ruleId: string; dimensionId: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorCell).values(params).returning();
        if (!created) throw new Error("createCell: insert returned no row");
        return created;
    });
}

export function deleteCell(id: string) {
    return dbEffect(async () => {
        const [deleted] = await db.delete(behaviorCell).where(eq(behaviorCell.id, id)).returning();
        return deleted ?? null;
    });
}

export function getCellRequirementCount(cellId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ count: sql<number>`count(*)::int` })
            .from(behaviorCellRequirement)
            .where(eq(behaviorCellRequirement.cellId, cellId));
        return result?.count ?? 0;
    });
}

// --- Cell Requirement Links ---

export function linkCellRequirement(cellId: string, requirementId: string) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorCellRequirement).values({ cellId, requirementId }).onConflictDoNothing().returning();
        return created ?? null;
    });
}

export function unlinkCellRequirement(cellId: string, requirementId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorCellRequirement)
            .where(and(eq(behaviorCellRequirement.cellId, cellId), eq(behaviorCellRequirement.requirementId, requirementId)))
            .returning();
        return deleted ?? null;
    });
}

export function getRequirementsForCell(cellId: string) {
    return dbEffect(() =>
        db
            .select({
                requirementId: behaviorCellRequirement.requirementId,
                title: requirement.title,
                status: requirement.status
            })
            .from(behaviorCellRequirement)
            .innerJoin(requirement, eq(behaviorCellRequirement.requirementId, requirement.id))
            .where(eq(behaviorCellRequirement.cellId, cellId))
    );
}

// --- Cell Code Links ---

export function linkCellCode(params: { id: string; cellId: string; kind: string; ref: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorCellCode).values(params).onConflictDoNothing().returning();
        return created ?? null;
    });
}

export function deleteCellCode(id: string, cellId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorCellCode)
            .where(and(eq(behaviorCellCode.id, id), eq(behaviorCellCode.cellId, cellId)))
            .returning();
        return deleted ?? null;
    });
}

export function getCodeForCell(cellId: string) {
    return dbEffect(() =>
        db.select().from(behaviorCellCode).where(eq(behaviorCellCode.cellId, cellId)).orderBy(asc(behaviorCellCode.createdAt))
    );
}

/**
 * Reverse lookup: which behavior rules govern a given file path? Matches code
 * links whose stored ref is the queried path, a suffix of it, or (for symbol
 * refs) prefixed by it. Scoped to the user via the owning matrix.
 */
export function getBehaviorsForCodePath(userId: string, path: string) {
    return dbEffect(() =>
        db
            .select({
                ruleId: behaviorRule.id,
                ruleTitle: behaviorRule.title,
                description: behaviorRule.description,
                rationale: behaviorRule.rationale,
                counterexample: behaviorRule.counterexample,
                matrixId: behaviorMatrix.id,
                matrixName: behaviorMatrix.name,
                layer: behaviorMatrix.layer,
                dimensionName: behaviorDimension.name,
                kind: behaviorCellCode.kind,
                ref: behaviorCellCode.ref
            })
            .from(behaviorCellCode)
            .innerJoin(behaviorCell, eq(behaviorCellCode.cellId, behaviorCell.id))
            .innerJoin(behaviorRule, eq(behaviorCell.ruleId, behaviorRule.id))
            .innerJoin(behaviorDimension, eq(behaviorCell.dimensionId, behaviorDimension.id))
            .innerJoin(behaviorMatrix, eq(behaviorRule.matrixId, behaviorMatrix.id))
            .where(
                and(
                    eq(behaviorMatrix.userId, userId),
                    sql`(${behaviorCellCode.ref} = ${path} OR ${path} LIKE '%' || ${behaviorCellCode.ref} OR ${behaviorCellCode.ref} LIKE ${path} || '::%')`
                )
            )
    );
}

// --- Cell Test Results ---

export function recordTestResult(params: { id: string; cellId: string; testRef: string; status: string; detail?: string }) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorTestResult).values(params).returning();
        if (!created) throw new Error("recordTestResult: insert returned no row");
        return created;
    });
}

export function getTestResultsForCell(cellId: string) {
    return dbEffect(() =>
        db.select().from(behaviorTestResult).where(eq(behaviorTestResult.cellId, cellId)).orderBy(desc(behaviorTestResult.runAt))
    );
}

// --- Matrix View (full grid query) ---

export function getMatrixView(matrixId: string) {
    return dbEffect(async () => {
        const dimensions = await db
            .select()
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId))
            .orderBy(asc(behaviorDimension.order));

        const rules = await db.select().from(behaviorRule).where(eq(behaviorRule.matrixId, matrixId)).orderBy(asc(behaviorRule.order));

        const ruleIds = rules.map(r => r.id);
        if (ruleIds.length === 0) {
            return { dimensions, rules, cells: [] as MatrixViewCell[] };
        }

        // Requirement aggregates (one row per cell — no fan-out from other joins).
        const reqCells = await db
            .select({
                id: behaviorCell.id,
                ruleId: behaviorCell.ruleId,
                dimensionId: behaviorCell.dimensionId,
                requirementCount: sql<number>`count(${behaviorCellRequirement.requirementId})::int`.as("requirement_count"),
                failingCount: sql<number>`count(case when ${requirement.status} = 'failing' then 1 end)::int`.as("failing_count")
            })
            .from(behaviorCell)
            .leftJoin(behaviorCellRequirement, eq(behaviorCellRequirement.cellId, behaviorCell.id))
            .leftJoin(requirement, eq(behaviorCellRequirement.requirementId, requirement.id))
            .where(inArray(behaviorCell.ruleId, ruleIds))
            .groupBy(behaviorCell.id, behaviorCell.ruleId, behaviorCell.dimensionId);

        const cellIds = reqCells.map(c => c.id);

        // Code-link counts per cell (separate query to avoid join fan-out).
        const codeCounts =
            cellIds.length === 0
                ? []
                : await db
                      .select({
                          cellId: behaviorCellCode.cellId,
                          codeCount: sql<number>`count(*)::int`.as("code_count")
                      })
                      .from(behaviorCellCode)
                      .where(inArray(behaviorCellCode.cellId, cellIds))
                      .groupBy(behaviorCellCode.cellId);

        // Test-result counts per cell.
        const testCounts =
            cellIds.length === 0
                ? []
                : await db
                      .select({
                          cellId: behaviorTestResult.cellId,
                          passingTestCount: sql<number>`count(case when ${behaviorTestResult.status} = 'pass' then 1 end)::int`.as(
                              "passing_test_count"
                          ),
                          failingTestCount: sql<number>`count(case when ${behaviorTestResult.status} = 'fail' then 1 end)::int`.as(
                              "failing_test_count"
                          )
                      })
                      .from(behaviorTestResult)
                      .where(inArray(behaviorTestResult.cellId, cellIds))
                      .groupBy(behaviorTestResult.cellId);

        const codeByCell = new Map(codeCounts.map(c => [c.cellId, c.codeCount]));
        const testByCell = new Map(testCounts.map(t => [t.cellId, t]));

        const cells: MatrixViewCell[] = reqCells.map(c => {
            const test = testByCell.get(c.id);
            return {
                id: c.id,
                ruleId: c.ruleId,
                dimensionId: c.dimensionId,
                requirementCount: c.requirementCount,
                failingCount: c.failingCount,
                codeCount: codeByCell.get(c.id) ?? 0,
                passingTestCount: test?.passingTestCount ?? 0,
                failingTestCount: test?.failingTestCount ?? 0
            };
        });

        return { dimensions, rules, cells };
    });
}

export interface MatrixViewCell {
    id: string;
    ruleId: string;
    dimensionId: string;
    requirementCount: number;
    failingCount: number;
    codeCount: number;
    passingTestCount: number;
    failingTestCount: number;
}
