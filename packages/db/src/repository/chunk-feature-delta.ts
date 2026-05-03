import { and, eq, inArray } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkVersion } from "../schema/chunk-version";
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

export function mergeFeatureDeltas(
    featureId: string,
    userId: string,
    deltas: Array<{ chunkId: string; delta: Record<string, unknown> }>
) {
    return dbEffect(async () => {
        return await db.transaction(async tx => {
            const affectedChunkIds: string[] = [];

            for (const deltaRow of deltas) {
                // Fetch base chunk
                const [existing] = await tx
                    .select()
                    .from(chunk)
                    .where(eq(chunk.id, deltaRow.chunkId));
                if (!existing) continue;

                // Create version snapshot
                const [maxVersion] = await tx
                    .select({
                        max: eq(chunkVersion.chunkId, deltaRow.chunkId)
                    })
                    .from(chunkVersion)
                    .where(eq(chunkVersion.chunkId, deltaRow.chunkId));

                // Count existing versions for next version number
                const versionRows = await tx
                    .select({ version: chunkVersion.version })
                    .from(chunkVersion)
                    .where(eq(chunkVersion.chunkId, deltaRow.chunkId));
                const nextVersion = versionRows.length > 0
                    ? Math.max(...versionRows.map(v => v.version)) + 1
                    : 1;

                await tx.insert(chunkVersion).values({
                    id: crypto.randomUUID(),
                    chunkId: deltaRow.chunkId,
                    version: nextVersion,
                    title: existing.title,
                    content: existing.content,
                    type: existing.type,
                    tags: []
                });

                // Apply delta to base chunk
                await tx
                    .update(chunk)
                    .set(deltaRow.delta as Record<string, unknown>)
                    .where(eq(chunk.id, deltaRow.chunkId));

                affectedChunkIds.push(deltaRow.chunkId);
            }

            // Delete all deltas for this feature
            await tx
                .delete(chunkFeatureDelta)
                .where(eq(chunkFeatureDelta.featureId, featureId));

            // Update feature status
            await tx
                .update(feature)
                .set({ status: "merged" })
                .where(and(eq(feature.id, featureId), eq(feature.userId, userId)));

            return affectedChunkIds;
        });
    });
}
