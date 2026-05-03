import { and, eq, gte, ilike, lte, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunkFeatureDelta, feature, featureCodebase, userActiveFeature } from "../schema/feature";
import { codebase } from "../schema/codebase";

export function createFeature(params: {
    id: string;
    name: string;
    description?: string;
    priority: number;
    color?: string;
    userId: string;
}) {
    return dbEffect(async () => {
        const [created] = await db.insert(feature).values(params).returning();
        return created!;
    });
}

export function getFeatureById(id: string, userId: string) {
    return dbEffect(async () => {
        const [found] = await db
            .select()
            .from(feature)
            .where(and(eq(feature.id, id), eq(feature.userId, userId)));
        return found ?? null;
    });
}

export function listFeatures(userId: string, filters?: { codebaseId?: string; status?: string; search?: string }) {
    return dbEffect(async () => {
        const conditions = [eq(feature.userId, userId)];

        if (filters?.status) {
            conditions.push(eq(feature.status, filters.status));
        }
        if (filters?.search) {
            conditions.push(ilike(feature.name, `%${filters.search}%`));
        }

        const features = await db
            .select({
                id: feature.id,
                name: feature.name,
                description: feature.description,
                priority: feature.priority,
                status: feature.status,
                color: feature.color,
                createdAt: feature.createdAt,
                updatedAt: feature.updatedAt,
                deltaCount: sql<number>`count(${chunkFeatureDelta.id})::int`.as("delta_count"),
            })
            .from(feature)
            .leftJoin(chunkFeatureDelta, eq(chunkFeatureDelta.featureId, feature.id))
            .where(and(...conditions))
            .groupBy(feature.id)
            .orderBy(feature.priority);

        if (!filters?.codebaseId) return features;

        const featureIdsInCodebase = await db
            .select({ featureId: featureCodebase.featureId })
            .from(featureCodebase)
            .where(eq(featureCodebase.codebaseId, filters.codebaseId));
        const idSet = new Set(featureIdsInCodebase.map(r => r.featureId));

        const allLinked = await db
            .select({ featureId: featureCodebase.featureId })
            .from(featureCodebase);
        const linkedSet = new Set(allLinked.map(r => r.featureId));

        return features.filter(f => idSet.has(f.id) || !linkedSet.has(f.id));
    });
}

export function updateFeature(
    id: string,
    userId: string,
    data: {
        name?: string;
        description?: string | null;
        priority?: number;
        status?: string;
        color?: string | null;
    },
) {
    return dbEffect(async () => {
        const [updated] = await db
            .update(feature)
            .set(data)
            .where(and(eq(feature.id, id), eq(feature.userId, userId)))
            .returning();
        return updated ?? null;
    });
}

export function deleteFeature(id: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(feature)
            .where(and(eq(feature.id, id), eq(feature.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

export function setFeatureCodebases(featureId: string, codebaseIds: string[]) {
    return dbEffect(async () => {
        await db.delete(featureCodebase).where(eq(featureCodebase.featureId, featureId));
        if (codebaseIds.length === 0) return [];
        return db
            .insert(featureCodebase)
            .values(codebaseIds.map(codebaseId => ({ featureId, codebaseId })))
            .returning();
    });
}

export function getCodebasesForFeature(featureId: string) {
    return dbEffect(() =>
        db
            .select({ id: codebase.id, name: codebase.name })
            .from(featureCodebase)
            .innerJoin(codebase, eq(featureCodebase.codebaseId, codebase.id))
            .where(eq(featureCodebase.featureId, featureId)),
    );
}

export function shiftPriorities(userId: string, newPriority: number, direction: "up" | "down") {
    return dbEffect(async () => {
        if (direction === "up") {
            await db
                .update(feature)
                .set({ priority: sql`${feature.priority} + 1` })
                .where(and(eq(feature.userId, userId), gte(feature.priority, newPriority)));
        } else {
            await db
                .update(feature)
                .set({ priority: sql`${feature.priority} - 1` })
                .where(and(eq(feature.userId, userId), lte(feature.priority, newPriority)));
        }
    });
}

export function getMaxPriority(userId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ max: sql<number>`coalesce(max(${feature.priority}), 0)::int` })
            .from(feature)
            .where(eq(feature.userId, userId));
        return result?.max ?? 0;
    });
}

export function getActiveFeatureIds(userId: string) {
    return dbEffect(() =>
        db
            .select({ featureId: userActiveFeature.featureId })
            .from(userActiveFeature)
            .where(eq(userActiveFeature.userId, userId)),
    );
}

export function setActiveFeatures(userId: string, featureIds: string[]) {
    return dbEffect(async () => {
        await db.delete(userActiveFeature).where(eq(userActiveFeature.userId, userId));
        if (featureIds.length === 0) return [];
        return db
            .insert(userActiveFeature)
            .values(featureIds.map(featureId => ({ userId, featureId })))
            .returning();
    });
}

export function featureNameConflict(id: string, userId: string, name: string) {
    return dbEffect(async () => {
        const [hit] = await db
            .select({ id: feature.id })
            .from(feature)
            .where(and(eq(feature.userId, userId), eq(feature.name, name), sql`${feature.id} != ${id}`))
            .limit(1);
        return !!hit;
    });
}
