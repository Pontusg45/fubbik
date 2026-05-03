import { and, eq, inArray } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkFeatureDelta, feature } from "../schema/feature";

export function upsertDelta(params: {
    id: string;
    chunkId: string;
    featureId: string;
    delta: Record<string, unknown>;
}) {
    return dbEffect(async () => {
        const [result] = await db
            .insert(chunkFeatureDelta)
            .values(params)
            .onConflictDoUpdate({
                target: [chunkFeatureDelta.chunkId, chunkFeatureDelta.featureId],
                set: { delta: params.delta, updatedAt: new Date() }
            })
            .returning();
        return result!;
    });
}

export function getDeltasForChunk(chunkId: string) {
    return dbEffect(() =>
        db
            .select({
                id: chunkFeatureDelta.id,
                chunkId: chunkFeatureDelta.chunkId,
                featureId: chunkFeatureDelta.featureId,
                delta: chunkFeatureDelta.delta,
                featureName: feature.name,
                featurePriority: feature.priority,
                featureColor: feature.color,
                featureStatus: feature.status,
                createdAt: chunkFeatureDelta.createdAt,
                updatedAt: chunkFeatureDelta.updatedAt
            })
            .from(chunkFeatureDelta)
            .innerJoin(feature, eq(chunkFeatureDelta.featureId, feature.id))
            .where(eq(chunkFeatureDelta.chunkId, chunkId))
            .orderBy(feature.priority)
    );
}

export function getDeltasForFeature(featureId: string) {
    return dbEffect(() =>
        db
            .select({
                id: chunkFeatureDelta.id,
                chunkId: chunkFeatureDelta.chunkId,
                featureId: chunkFeatureDelta.featureId,
                delta: chunkFeatureDelta.delta,
                chunkTitle: chunk.title,
                createdAt: chunkFeatureDelta.createdAt,
                updatedAt: chunkFeatureDelta.updatedAt
            })
            .from(chunkFeatureDelta)
            .innerJoin(chunk, eq(chunkFeatureDelta.chunkId, chunk.id))
            .where(eq(chunkFeatureDelta.featureId, featureId))
    );
}

export function batchFetchDeltas(chunkIds: string[], featureIds: string[]) {
    if (chunkIds.length === 0 || featureIds.length === 0) {
        return dbEffect(async () => []);
    }
    return dbEffect(() =>
        db
            .select({
                id: chunkFeatureDelta.id,
                chunkId: chunkFeatureDelta.chunkId,
                featureId: chunkFeatureDelta.featureId,
                delta: chunkFeatureDelta.delta,
                featurePriority: feature.priority
            })
            .from(chunkFeatureDelta)
            .innerJoin(feature, eq(chunkFeatureDelta.featureId, feature.id))
            .where(
                and(
                    inArray(chunkFeatureDelta.chunkId, chunkIds),
                    inArray(chunkFeatureDelta.featureId, featureIds)
                )
            )
            .orderBy(feature.priority)
    );
}

export function deleteDelta(chunkId: string, featureId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(chunkFeatureDelta)
            .where(
                and(
                    eq(chunkFeatureDelta.chunkId, chunkId),
                    eq(chunkFeatureDelta.featureId, featureId)
                )
            )
            .returning();
        return deleted ?? null;
    });
}

export function deleteDeltasForFeature(featureId: string) {
    return dbEffect(async () => {
        await db
            .delete(chunkFeatureDelta)
            .where(eq(chunkFeatureDelta.featureId, featureId));
    });
}
