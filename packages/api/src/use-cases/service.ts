import {
    createUseCase as createUseCaseRepo,
    deleteUseCase as deleteUseCaseRepo,
    getUseCaseById,
    listRequirementsByUseCase,
    listUseCases as listUseCasesRepo,
    updateUseCase as updateUseCaseRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

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
    }
) {
    return createUseCaseRepo({
        id: crypto.randomUUID(),
        name: body.name,
        description: body.description,
        codebaseId: body.codebaseId,
        userId
    });
}

export function updateUseCase(
    id: string,
    userId: string,
    body: {
        name?: string;
        description?: string | null;
        order?: number;
    }
) {
    return getUseCaseById(id, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "UseCase" }))
        ),
        Effect.flatMap(() => updateUseCaseRepo(id, userId, body)),
        Effect.flatMap(updated =>
            updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "UseCase" }))
        )
    );
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
