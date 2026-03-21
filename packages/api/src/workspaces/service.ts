import {
    addCodebaseToWorkspace as addCodebaseRepo,
    createWorkspace as createWorkspaceRepo,
    deleteWorkspace as deleteWorkspaceRepo,
    getCodebaseById,
    getCodebasesForWorkspace,
    getWorkspaceById,
    listWorkspaces as listWorkspacesRepo,
    removeCodebaseFromWorkspace as removeCodebaseRepo,
    updateWorkspace as updateWorkspaceRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function listWorkspaces(userId: string) {
    return listWorkspacesRepo(userId);
}

export function getWorkspaceDetail(id: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getWorkspaceById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));

        const codebases = yield* getCodebasesForWorkspace(id);

        return { ...found, codebases };
    });
}

export function createWorkspace(
    userId: string,
    body: {
        name: string;
        description?: string;
    }
) {
    return Effect.gen(function* () {
        if (!body.name.trim()) {
            return yield* Effect.fail(new ValidationError({ message: "Workspace name is required" }));
        }

        const workspaceId = crypto.randomUUID();
        return yield* createWorkspaceRepo({
            id: workspaceId,
            name: body.name.trim(),
            description: body.description,
            userId
        });
    });
}

export function updateWorkspace(
    id: string,
    userId: string,
    body: {
        name?: string;
        description?: string | null;
    }
) {
    return Effect.gen(function* () {
        const found = yield* getWorkspaceById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));

        if (body.name !== undefined && !body.name.trim()) {
            return yield* Effect.fail(new ValidationError({ message: "Workspace name cannot be empty" }));
        }

        const updated = yield* updateWorkspaceRepo(id, userId, {
            name: body.name?.trim(),
            description: body.description
        });
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));
        return updated;
    });
}

export function deleteWorkspace(id: string, userId: string) {
    return deleteWorkspaceRepo(id, userId).pipe(
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Workspace" }))
        )
    );
}

export function addCodebaseToWorkspace(workspaceId: string, userId: string, codebaseId: string) {
    return Effect.gen(function* () {
        const ws = yield* getWorkspaceById(workspaceId, userId);
        if (!ws) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));

        const cb = yield* getCodebaseById(codebaseId, userId);
        if (!cb) return yield* Effect.fail(new NotFoundError({ resource: "Codebase" }));

        return yield* addCodebaseRepo(workspaceId, codebaseId);
    });
}

export function removeCodebaseFromWorkspace(workspaceId: string, userId: string, codebaseId: string) {
    return Effect.gen(function* () {
        const ws = yield* getWorkspaceById(workspaceId, userId);
        if (!ws) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));

        const deleted = yield* removeCodebaseRepo(workspaceId, codebaseId);
        if (!deleted) return yield* Effect.fail(new NotFoundError({ resource: "WorkspaceCodebase" }));
        return deleted;
    });
}
