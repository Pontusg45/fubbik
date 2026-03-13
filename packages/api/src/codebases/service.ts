import {
    countChunksInCodebase,
    createCodebase as createCodebaseRepo,
    deleteCodebase as deleteCodebaseRepo,
    getCodebaseById,
    getCodebaseByLocalPath,
    getCodebaseByRemoteUrl,
    listCodebases as listCodebasesRepo,
    updateCodebase as updateCodebaseRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";
import { normalizeGitUrl } from "./normalize-url";

export function listCodebases(userId: string) {
    return listCodebasesRepo(userId);
}

export function getCodebase(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" }))))
    );
}

export function createCodebase(userId: string, body: { name: string; remoteUrl?: string; localPaths?: string[] }) {
    const id = crypto.randomUUID();
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : undefined;
    return Effect.suspend(() => {
        if (!remoteUrl) return Effect.void;
        return getCodebaseByRemoteUrl(remoteUrl, userId).pipe(
            Effect.flatMap(existing =>
                existing
                    ? Effect.fail(new ValidationError({ message: "A codebase with this remote URL already exists" }))
                    : Effect.void
            )
        );
    }).pipe(
        Effect.flatMap(() =>
            createCodebaseRepo({ id, name: body.name, remoteUrl, localPaths: body.localPaths ?? [], userId })
        )
    );
}

export function updateCodebase(
    codebaseId: string,
    userId: string,
    body: { name?: string; remoteUrl?: string | null; localPaths?: string[] }
) {
    const remoteUrl = body.remoteUrl ? normalizeGitUrl(body.remoteUrl) : body.remoteUrl;
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" })))),
        Effect.flatMap(() => updateCodebaseRepo(codebaseId, userId, { name: body.name, remoteUrl, localPaths: body.localPaths }))
    );
}

export function deleteCodebase(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" })))),
        Effect.flatMap(() => deleteCodebaseRepo(codebaseId, userId))
    );
}

export function detectCodebase(userId: string, query: { remoteUrl?: string; localPath?: string }) {
    const normalizedUrl = query.remoteUrl ? normalizeGitUrl(query.remoteUrl) : undefined;

    if (normalizedUrl) {
        return getCodebaseByRemoteUrl(normalizedUrl, userId);
    }
    if (query.localPath) {
        return getCodebaseByLocalPath(query.localPath, userId);
    }
    return Effect.succeed(null);
}

export function getCodebaseChunkCount(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Codebase" })))),
        Effect.flatMap(() => countChunksInCodebase(codebaseId))
    );
}
