import {
    createFeature as createFeatureRepo,
    deleteFeature as deleteFeatureRepo,
    featureNameConflict,
    getActiveFeatureIds as getActiveFeatureIdsRepo,
    getCodebasesForFeature,
    getFeatureById,
    getMaxPriority,
    listFeatures as listFeaturesRepo,
    setActiveFeatures as setActiveFeaturesRepo,
    setFeatureCodebases,
    shiftPriorities,
    updateFeature as updateFeatureRepo
} from "@fubbik/db/repository";
import {
    batchFetchDeltas,
    getDeltasForChunk as getDeltasForChunkRepo,
    getDeltasForFeature as getDeltasForFeatureRepo,
    mergeFeatureDeltas,
    upsertDelta as upsertDeltaRepo,
    deleteDelta as deleteDeltaRepo
} from "@fubbik/db/repository";
import {
    getChunkById
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { DatabaseError } from "@fubbik/db/errors";
import { NotFoundError, ValidationError } from "../errors";
import { enrichChunk } from "../enrich/service";
import { logger } from "../logger";

const DELTA_ALLOWED_FIELDS = new Set(["title", "content", "type", "rationale", "alternatives", "consequences", "summary"]);

function validateDelta(delta: Record<string, unknown>): Effect.Effect<Record<string, unknown>, ValidationError> {
    const invalid = Object.keys(delta).filter(k => !DELTA_ALLOWED_FIELDS.has(k));
    if (invalid.length > 0) {
        return Effect.fail(new ValidationError({ message: `Invalid delta fields: ${invalid.join(", ")}. Allowed: ${[...DELTA_ALLOWED_FIELDS].join(", ")}` }));
    }
    if (Object.keys(delta).length === 0) {
        return Effect.fail(new ValidationError({ message: "Delta must contain at least one field" }));
    }
    return Effect.succeed(delta);
}

export function createFeature(userId: string, body: {
    name: string;
    description?: string;
    priority?: number;
    color?: string;
    codebaseIds?: string[];
}) {
    const id = crypto.randomUUID();
    return (body.priority !== undefined ? Effect.succeed(body.priority) : getMaxPriority(userId).pipe(Effect.map(max => max + 1))).pipe(
        Effect.flatMap(priority =>
            createFeatureRepo({ id, name: body.name, description: body.description, priority, color: body.color, userId })
        ),
        Effect.tap(() => {
            if (body.codebaseIds && body.codebaseIds.length > 0) {
                return setFeatureCodebases(id, body.codebaseIds);
            }
            return Effect.void;
        })
    );
}

export function getFeatureDetail(featureId: string, userId: string) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(found =>
            Effect.all({
                feature: Effect.succeed(found),
                codebases: getCodebasesForFeature(featureId),
                deltas: getDeltasForFeatureRepo(featureId)
            })
        )
    );
}

export function listFeatures(userId: string, filters?: { codebaseId?: string; status?: string; search?: string }) {
    return listFeaturesRepo(userId, filters);
}

export function updateFeature(featureId: string, userId: string, body: {
    name?: string;
    description?: string | null;
    priority?: number;
    status?: string;
    color?: string | null;
    codebaseIds?: string[];
}) {
    const guard = body.name !== undefined
        ? featureNameConflict(featureId, userId, body.name).pipe(
            Effect.flatMap(conflict =>
                conflict
                    ? Effect.fail(new ValidationError({ message: `Feature "${body.name}" already exists` }))
                    : Effect.succeed(undefined)
            )
        )
        : Effect.succeed(undefined);

    const { codebaseIds, ...repoBody } = body;

    return guard.pipe(
        Effect.flatMap(() => updateFeatureRepo(featureId, userId, repoBody)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.tap(() => {
            if (codebaseIds !== undefined) {
                return setFeatureCodebases(featureId, codebaseIds);
            }
            return Effect.void;
        })
    );
}

export function deleteFeatureService(featureId: string, userId: string) {
    return deleteFeatureRepo(featureId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Feature" }))))
    );
}

export function reorderFeature(featureId: string, userId: string, newPriority: number) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(existing => {
            if (existing.priority === newPriority) return Effect.succeed(existing);
            return shiftPriorities(userId, newPriority, "up").pipe(
                Effect.flatMap(() => updateFeatureRepo(featureId, userId, { priority: newPriority })),
                Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Feature" }))))
            );
        })
    );
}

export function getActiveFeatures(userId: string) {
    return getActiveFeatureIdsRepo(userId).pipe(
        Effect.map(rows => rows.map(r => r.featureId))
    );
}

export function setActiveFeatures(userId: string, featureIds: string[]) {
    if (featureIds.length === 0) {
        return setActiveFeaturesRepo(userId, []);
    }
    // Verify all features belong to the user
    return listFeaturesRepo(userId).pipe(
        Effect.flatMap((userFeatures): Effect.Effect<{ featureId: string; userId: string }[], ValidationError | DatabaseError, never> => {
            const ownedIds = new Set(userFeatures.map(f => f.id));
            const invalid = featureIds.filter(id => !ownedIds.has(id));
            if (invalid.length > 0) {
                return Effect.fail(new ValidationError({ message: `Features not found: ${invalid.join(", ")}` }));
            }
            return setActiveFeaturesRepo(userId, featureIds);
        })
    );
}

export function getDeltasForChunk(chunkId: string) {
    return getDeltasForChunkRepo(chunkId);
}

export function getDeltasForFeature(featureId: string) {
    return getDeltasForFeatureRepo(featureId);
}

export function upsertDelta(chunkId: string, featureId: string, userId: string, delta: Record<string, unknown>) {
    return validateDelta(delta).pipe(
        Effect.flatMap(() => getFeatureById(featureId, userId)),
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(() => getChunkById(chunkId, userId)),
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => upsertDeltaRepo({ id: crypto.randomUUID(), chunkId, featureId, delta }))
    );
}

export function deleteDelta(chunkId: string, featureId: string, userId: string) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(() => deleteDeltaRepo(chunkId, featureId)),
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Delta" }))))
    );
}

export function mergeFeature(featureId: string, userId: string) {
    return getFeatureById(featureId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Feature" })))),
        Effect.flatMap(found => {
            if (found.status === "merged") {
                return Effect.fail<ValidationError>(new ValidationError({ message: "Feature is already merged" }));
            }
            return Effect.succeed(undefined);
        }),
        Effect.flatMap(() => getDeltasForFeatureRepo(featureId)),
        Effect.flatMap(deltas => {
            if (deltas.length === 0) {
                return updateFeatureRepo(featureId, userId, { status: "merged" }).pipe(Effect.asVoid);
            }

            // Atomic merge: version snapshots + delta application + cleanup in one transaction
            return mergeFeatureDeltas(featureId, userId, deltas.map(d => ({
                chunkId: d.chunkId,
                delta: d.delta as Record<string, unknown>
            }))).pipe(
                Effect.tap(affectedChunkIds => {
                    // Fire-and-forget re-enrichment for affected chunks
                    for (const chunkId of affectedChunkIds) {
                        Effect.runPromise(enrichChunk(chunkId)).catch(err => {
                            logger.error(`[merge] Failed to re-enrich chunk ${chunkId}:`, { err });
                        });
                    }
                    return Effect.void;
                }),
                Effect.asVoid
            );
        })
    );
}

// Re-export for convenience
export { batchFetchDeltas };
