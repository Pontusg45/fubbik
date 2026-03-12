import { createTagType as createTagTypeRepo, deleteTagType as deleteTagTypeRepo, getTagTypesForUser, updateTagType as updateTagTypeRepo } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function listTagTypes(userId: string) {
    return getTagTypesForUser(userId);
}

export function createTagType(userId: string, body: { name: string; color?: string }) {
    const id = crypto.randomUUID();
    return createTagTypeRepo({ id, name: body.name, color: body.color ?? "#8b5cf6", userId });
}

export function updateTagType(id: string, userId: string, body: { name?: string; color?: string }) {
    return updateTagTypeRepo(id, userId, body).pipe(
        Effect.flatMap(updated => (updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "TagType" }))))
    );
}

export function deleteTagType(id: string, userId: string) {
    return deleteTagTypeRepo(id, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "TagType" }))))
    );
}
