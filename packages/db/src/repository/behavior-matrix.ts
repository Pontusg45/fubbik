import { and, eq, sql, inArray, asc } from "drizzle-orm";

import { db, dbEffect } from "../index";
import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement
} from "../schema/behavior-matrix";
import { requirement } from "../schema/requirement";

// --- Matrix CRUD ---

export function createMatrix(params: {
    id: string;
    name: string;
    layer: string;
    description?: string;
    spaceId?: string;
    userId: string;
}) {
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
        db
            .select()
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId))
            .orderBy(asc(behaviorDimension.order))
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
            await db
                .update(behaviorDimension)
                .set({ order: i })
                .where(eq(behaviorDimension.id, dimId));
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
    order: number;
}) {
    return dbEffect(async () => {
        const [created] = await db.insert(behaviorRule).values(params).returning();
        if (!created) throw new Error("createRule: insert returned no row");
        return created;
    });
}

export function updateRule(id: string, matrixId: string, data: { title?: string; description?: string | null; category?: string | null }) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(behaviorRule)
            .set(data)
            .where(and(eq(behaviorRule.id, id), eq(behaviorRule.matrixId, matrixId)))
            .returning();
        return updated ?? null;
    });
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
    return dbEffect(() =>
        db
            .select()
            .from(behaviorRule)
            .where(eq(behaviorRule.matrixId, matrixId))
            .orderBy(asc(behaviorRule.order))
    );
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
            await db
                .update(behaviorRule)
                .set({ order: i })
                .where(eq(behaviorRule.id, ruleId));
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
        const [created] = await db
            .insert(behaviorCellRequirement)
            .values({ cellId, requirementId })
            .onConflictDoNothing()
            .returning();
        return created ?? null;
    });
}

export function unlinkCellRequirement(cellId: string, requirementId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(behaviorCellRequirement)
            .where(and(
                eq(behaviorCellRequirement.cellId, cellId),
                eq(behaviorCellRequirement.requirementId, requirementId)
            ))
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

// --- Matrix View (full grid query) ---

export function getMatrixView(matrixId: string) {
    return dbEffect(async () => {
        const dimensions = await db
            .select()
            .from(behaviorDimension)
            .where(eq(behaviorDimension.matrixId, matrixId))
            .orderBy(asc(behaviorDimension.order));

        const rules = await db
            .select()
            .from(behaviorRule)
            .where(eq(behaviorRule.matrixId, matrixId))
            .orderBy(asc(behaviorRule.order));

        const cells = await db
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
            .where(
                inArray(
                    behaviorCell.ruleId,
                    rules.map(r => r.id)
                )
            )
            .groupBy(behaviorCell.id, behaviorCell.ruleId, behaviorCell.dimensionId);

        return { dimensions, rules, cells };
    });
}
