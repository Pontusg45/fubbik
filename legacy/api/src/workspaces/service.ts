import {
    addSpaceToWorkspace as addSpaceRepo,
    createWorkspace as createWorkspaceRepo,
    deleteWorkspace as deleteWorkspaceRepo,
    getSpaceById,
    getSpacesForWorkspace,
    getWorkspaceById,
    listWorkspaces as listWorkspacesRepo,
    removeSpaceFromWorkspace as removeSpaceRepo,
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

        const spaces = yield* getSpacesForWorkspace(id);

        return { ...found, spaces };
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
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Workspace" }))))
    );
}

export function addSpaceToWorkspace(workspaceId: string, userId: string, spaceId: string) {
    return Effect.gen(function* () {
        const ws = yield* getWorkspaceById(workspaceId, userId);
        if (!ws) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));

        const sp = yield* getSpaceById(spaceId, userId);
        if (!sp) return yield* Effect.fail(new NotFoundError({ resource: "Space" }));

        return yield* addSpaceRepo(workspaceId, spaceId);
    });
}

export function removeSpaceFromWorkspace(workspaceId: string, userId: string, spaceId: string) {
    return Effect.gen(function* () {
        const ws = yield* getWorkspaceById(workspaceId, userId);
        if (!ws) return yield* Effect.fail(new NotFoundError({ resource: "Workspace" }));

        const deleted = yield* removeSpaceRepo(workspaceId, spaceId);
        if (!deleted) return yield* Effect.fail(new NotFoundError({ resource: "WorkspaceSpace" }));
        return deleted;
    });
}
