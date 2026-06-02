import {
    countChunksInSpace,
    createSpace as createSpaceRepo,
    deleteSpace as deleteSpaceRepo,
    getCodeSpaceByLocalPath,
    getCodeSpaceByRemoteUrl,
    getSpaceById,
    getSpaceWithCodeMetadata,
    listSpaces as listSpacesRepo,
    resetSpaceData as resetSpaceDataRepo,
    updateSpace as updateSpaceRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";
import { normalizeGitUrl } from "./normalize-url";

export function listSpaces(userId: string) {
    return listSpacesRepo(userId);
}

export function getSpace(spaceId: string, userId: string) {
    return getSpaceWithCodeMetadata(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" }))))
    );
}

export interface CreateSpaceBody {
    name: string;
    kind?: string;
    description?: string;
    remoteUrl?: string;
    localPaths?: string[];
}

export function createSpace(userId: string, body: CreateSpaceBody) {
    const id = crypto.randomUUID();
    const kind = body.kind ?? "code";
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : undefined;
    return Effect.suspend(() => {
        if (kind !== "code" || !remoteUrl) return Effect.void;
        return getCodeSpaceByRemoteUrl(remoteUrl, userId).pipe(
            Effect.flatMap(existing =>
                existing
                    ? Effect.fail(new ValidationError({ message: "A space with this remote URL already exists" }))
                    : Effect.void
            )
        );
    }).pipe(
        Effect.flatMap(() =>
            createSpaceRepo({
                id,
                name: body.name,
                kind,
                description: body.description,
                userId,
                code: kind === "code" ? { remoteUrl, localPaths: body.localPaths } : undefined
            })
        )
    );
}

export interface UpdateSpaceBody {
    name?: string;
    description?: string | null;
    remoteUrl?: string | null;
    localPaths?: string[];
}

export function updateSpace(spaceId: string, userId: string, body: UpdateSpaceBody) {
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : body.remoteUrl;
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(found =>
            updateSpaceRepo(spaceId, userId, {
                name: body.name,
                description: body.description,
                code:
                    found.kind === "code"
                        ? { remoteUrl: remoteUrl ?? null, localPaths: body.localPaths ?? [] }
                        : undefined
            })
        )
    );
}

export function resetSpace(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => resetSpaceDataRepo(spaceId, userId))
    );
}

export function deleteSpace(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => resetSpaceDataRepo(spaceId, userId)),
        Effect.flatMap(() => deleteSpaceRepo(spaceId, userId))
    );
}

export function detectSpace(userId: string, query: { remoteUrl?: string; localPath?: string }) {
    const normalizedUrl = query.remoteUrl ? normalizeGitUrl(query.remoteUrl) : undefined;
    if (normalizedUrl) return getCodeSpaceByRemoteUrl(normalizedUrl, userId);
    if (query.localPath) return getCodeSpaceByLocalPath(query.localPath, userId);
    return Effect.succeed(null);
}

export function getSpaceChunkCount(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => countChunksInSpace(spaceId))
    );
}
