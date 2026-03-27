import {
    createSavedGraph as createSavedGraphRepo,
    deleteSavedGraph as deleteSavedGraphRepo,
    getSavedGraphById,
    listSavedGraphs as listSavedGraphsRepo,
    updateSavedGraph as updateSavedGraphRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function listSavedGraphs(userId: string, codebaseId?: string | null) {
    return listSavedGraphsRepo(userId, codebaseId);
}

export function getSavedGraphDetail(id: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getSavedGraphById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "SavedGraph" }));
        return found;
    });
}

export function createSavedGraph(
    userId: string,
    body: {
        name: string;
        description?: string;
        chunkIds: string[];
        positions: Record<string, { x: number; y: number }>;
        layoutAlgorithm?: string;
        codebaseId?: string | null;
    }
) {
    return Effect.gen(function* () {
        if (!body.name.trim()) {
            return yield* Effect.fail(new ValidationError({ message: "Saved graph name is required" }));
        }

        const id = crypto.randomUUID();
        return yield* createSavedGraphRepo({
            id,
            name: body.name.trim(),
            description: body.description,
            chunkIds: body.chunkIds,
            positions: body.positions,
            layoutAlgorithm: body.layoutAlgorithm ?? "force",
            userId,
            codebaseId: body.codebaseId
        });
    });
}

export function updateSavedGraph(
    id: string,
    userId: string,
    body: {
        name?: string;
        description?: string | null;
        chunkIds?: string[];
        positions?: Record<string, { x: number; y: number }>;
        layoutAlgorithm?: string;
    }
) {
    return Effect.gen(function* () {
        const found = yield* getSavedGraphById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "SavedGraph" }));

        if (body.name !== undefined && !body.name.trim()) {
            return yield* Effect.fail(new ValidationError({ message: "Saved graph name cannot be empty" }));
        }

        const updated = yield* updateSavedGraphRepo(id, userId, {
            name: body.name?.trim(),
            description: body.description,
            chunkIds: body.chunkIds,
            positions: body.positions,
            layoutAlgorithm: body.layoutAlgorithm
        });
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "SavedGraph" }));
        return updated;
    });
}

export function deleteSavedGraph(id: string, userId: string) {
    return deleteSavedGraphRepo(id, userId).pipe(
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "SavedGraph" }))
        )
    );
}
