import {
    createUseCase as createUseCaseRepo,
    deleteUseCase as deleteUseCaseRepo,
    getUseCaseById,
    listRequirementsByUseCase,
    listUseCases as listUseCasesRepo,
    updateUseCase as updateUseCaseRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function listUseCases(userId: string, codebaseId?: string) {
    return listUseCasesRepo(userId, codebaseId);
}

export function getUseCase(id: string, userId: string) {
    return getUseCaseById(id, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "UseCase" }))
        )
    );
}

export function createUseCase(
    userId: string,
    body: {
        name: string;
        description?: string;
        codebaseId?: string;
        parentId?: string;
    }
) {
    return Effect.gen(function* () {
        if (body.parentId) {
            const parent = yield* getUseCaseById(body.parentId, userId);
            if (!parent) return yield* Effect.fail(new NotFoundError({ resource: "Parent use case" }));
            if (parent.parentId) return yield* Effect.fail(new ValidationError({ message: "Cannot nest more than one level deep" }));
        }
        return yield* createUseCaseRepo({
            id: crypto.randomUUID(),
            name: body.name,
            description: body.description,
            codebaseId: body.codebaseId,
            parentId: body.parentId,
            userId
        });
    });
}

export function updateUseCase(
    id: string,
    userId: string,
    body: {
        name?: string;
        description?: string | null;
        order?: number;
        parentId?: string | null;
    }
) {
    return Effect.gen(function* () {
        const found = yield* getUseCaseById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "UseCase" }));

        if (body.parentId !== undefined && body.parentId !== null) {
            if (body.parentId === id) {
                return yield* Effect.fail(new ValidationError({ message: "Cannot set use case as its own parent" }));
            }
            const parent = yield* getUseCaseById(body.parentId, userId);
            if (!parent) return yield* Effect.fail(new NotFoundError({ resource: "Parent use case" }));
            if (parent.parentId) return yield* Effect.fail(new ValidationError({ message: "Cannot nest more than one level deep" }));
            if (parent.parentId === id) {
                return yield* Effect.fail(new ValidationError({ message: "Cannot set a child use case as parent" }));
            }
        }

        const updated = yield* updateUseCaseRepo(id, userId, body);
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "UseCase" }));
        return updated;
    });
}

export function deleteUseCase(id: string, userId: string) {
    return deleteUseCaseRepo(id, userId).pipe(
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "UseCase" }))
        )
    );
}

export function getUseCaseRequirements(id: string, userId: string) {
    return getUseCaseById(id, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "UseCase" }))
        ),
        Effect.flatMap(() => listRequirementsByUseCase(id, userId))
    );
}
