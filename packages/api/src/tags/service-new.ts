import { DatabaseError } from "@fubbik/db/errors";
import {
    createTag as createTagRepo,
    deleteTag as deleteTagRepo,
    findOrCreateTag,
    getTagsForUser,
    mergeTags as mergeTagsRepo,
    setChunkTags,
    tagNameConflict,
    updateTag as updateTagRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function getUserTags(userId: string) {
    return getTagsForUser(userId);
}

export function createUserTag(userId: string, body: { name: string; tagTypeId?: string; origin?: string }) {
    const id = crypto.randomUUID();
    const origin = body.origin ?? "human";
    return createTagRepo({ id, name: body.name, tagTypeId: body.tagTypeId, userId, origin, reviewStatus: origin === "ai" ? "draft" : "approved" });
}

export function updateUserTag(id: string, userId: string, body: { name?: string; tagTypeId?: string | null; reviewStatus?: string }) {
    const data: Parameters<typeof updateTagRepo>[2] = { ...body };
    if (body.reviewStatus !== undefined) {
        data.reviewedBy = userId;
        data.reviewedAt = new Date();
    }

    // If the caller is renaming, refuse collisions early. The DB has a unique
    // (user_id, name) index, so without this check the rename would surface as
    // a generic DatabaseError → 500 instead of 400.
    const guard = body.name !== undefined
        ? tagNameConflict(id, userId, body.name).pipe(
            Effect.flatMap(conflict =>
                conflict
                    ? Effect.fail(new ValidationError({ message: `Tag "${body.name}" already exists` }))
                    : Effect.succeed(undefined)
            )
        )
        : Effect.succeed(undefined);

    return guard.pipe(
        Effect.flatMap(() => updateTagRepo(id, userId, data)),
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Tag" }))))
    );
}

export function mergeUserTags(
    userId: string,
    sourceId: string,
    targetId: string
): Effect.Effect<{ targetId: string; chunkCount: number }, ValidationError | DatabaseError> {
    if (sourceId === targetId) {
        return Effect.fail(new ValidationError({ message: "Cannot merge a tag into itself" }));
    }
    return mergeTagsRepo(sourceId, targetId, userId);
}

export function deleteUserTag(id: string, userId: string) {
    return deleteTagRepo(id, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Tag" }))))
    );
}

export { setChunkTags, findOrCreateTag };
