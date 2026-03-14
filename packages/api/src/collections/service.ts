import {
    createCollection as createCollectionRepo,
    deleteCollection as deleteCollectionRepo,
    getCollectionById,
    listCollections as listCollectionsRepo,
    updateCollection as updateCollectionRepo
} from "@fubbik/db/repository";
import type { CollectionFilter } from "@fubbik/db/repository";
import { Effect } from "effect";

import { listChunks } from "../chunks/service";
import { NotFoundError } from "../errors";

export function listCollections(userId: string) {
    return listCollectionsRepo(userId);
}

export function createCollection(
    userId: string,
    body: {
        name: string;
        description?: string;
        filter: CollectionFilter;
        codebaseId?: string;
    }
) {
    return createCollectionRepo({
        id: crypto.randomUUID(),
        name: body.name,
        description: body.description,
        filter: body.filter,
        userId,
        codebaseId: body.codebaseId
    });
}

export function updateCollection(
    id: string,
    userId: string,
    body: {
        name?: string;
        description?: string;
        filter?: CollectionFilter;
    }
) {
    return getCollectionById(id, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Collection" }))
        ),
        Effect.flatMap(() => updateCollectionRepo(id, userId, body)),
        Effect.flatMap(updated =>
            updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Collection" }))
        )
    );
}

export function deleteCollection(id: string, userId: string) {
    return deleteCollectionRepo(id, userId).pipe(
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Collection" }))
        )
    );
}

export function getCollectionChunks(id: string, userId: string) {
    return getCollectionById(id, userId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Collection" }))
        ),
        Effect.flatMap(col => {
            const filter = col.filter;
            return listChunks(userId, {
                type: filter.type,
                search: filter.search,
                sort: filter.sort as "newest" | "oldest" | "alpha" | "updated" | undefined,
                tags: filter.tags,
                after: filter.after,
                enrichment: filter.enrichment as "missing" | "complete" | undefined,
                minConnections: filter.minConnections,
                origin: filter.origin,
                reviewStatus: filter.reviewStatus,
                codebaseId: col.codebaseId ?? undefined
            });
        })
    );
}
