import {
    createTag as createTagRepo,
    deleteTag as deleteTagRepo,
    findOrCreateTag,
    getTagsForUser,
    setChunkTags,
    updateTag as updateTagRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

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
    return updateTagRepo(id, userId, data).pipe(
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Tag" }))))
    );
}

export function deleteUserTag(id: string, userId: string) {
    return deleteTagRepo(id, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Tag" }))))
    );
}

export { setChunkTags, findOrCreateTag };
