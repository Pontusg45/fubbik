import {
    archiveMany,
    deleteMany,
    findOrCreateTag,
    getChunkById,
    getTagsForChunks,
    setChunkCodebases,
    setChunkTags,
    updateManyChunks
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { DatabaseError } from "@fubbik/db/errors";
import { AuthError, NotFoundError, ValidationError } from "../errors";

export type BulkAction = "add_tags" | "remove_tags" | "set_type" | "set_codebase" | "set_review_status" | "archive" | "delete";

export function bulkUpdate(
    userId: string,
    body: {
        ids: string[];
        action: BulkAction;
        value?: string | null;
    }
) {
    const { ids, action, value } = body;

    // Validate ownership of all chunk IDs
    const validateOwnership = Effect.all(
        ids.map(id =>
            getChunkById(id, userId).pipe(
                Effect.flatMap(found =>
                    found ? Effect.succeed(found) : Effect.fail(new AuthError())
                )
            )
        ),
        { concurrency: 10 }
    );

    return validateOwnership.pipe(
        Effect.flatMap((_chunks): Effect.Effect<{ updated: number }, ValidationError | DatabaseError | AuthError | NotFoundError> => {
            switch (action) {
                case "add_tags": {
                    if (!value) return Effect.fail(new ValidationError({ message: "value is required for add_tags" }));
                    const tagNames = value.split(",").map(s => s.trim()).filter(Boolean);
                    return Effect.all(tagNames.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                        Effect.flatMap(tags => {
                            const newTagIds = tags.map(t => t.id);
                            return getTagsForChunks(ids).pipe(
                                Effect.flatMap(existingTags => {
                                    const tagsByChunk = new Map<string, string[]>();
                                    for (const id of ids) tagsByChunk.set(id, []);
                                    for (const et of existingTags) {
                                        const list = tagsByChunk.get(et.chunkId) ?? [];
                                        list.push(et.tagId);
                                        tagsByChunk.set(et.chunkId, list);
                                    }
                                    return Effect.all(
                                        ids.map(id => {
                                            const existing = tagsByChunk.get(id) ?? [];
                                            const merged = [...new Set([...existing, ...newTagIds])];
                                            return setChunkTags(id, merged);
                                        }),
                                        { concurrency: 10 }
                                    );
                                })
                            );
                        }),
                        Effect.map(() => ({ updated: ids.length }))
                    );
                }
                case "remove_tags": {
                    if (!value) return Effect.fail(new ValidationError({ message: "value is required for remove_tags" }));
                    const tagNames = value.split(",").map(s => s.trim()).filter(Boolean);
                    return Effect.all(tagNames.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                        Effect.flatMap(tags => {
                            const removeTagIds = new Set(tags.map(t => t.id));
                            return getTagsForChunks(ids).pipe(
                                Effect.flatMap(existingTags => {
                                    const tagsByChunk = new Map<string, string[]>();
                                    for (const id of ids) tagsByChunk.set(id, []);
                                    for (const et of existingTags) {
                                        const list = tagsByChunk.get(et.chunkId) ?? [];
                                        list.push(et.tagId);
                                        tagsByChunk.set(et.chunkId, list);
                                    }
                                    return Effect.all(
                                        ids.map(id => {
                                            const existing = tagsByChunk.get(id) ?? [];
                                            const filtered = existing.filter(tid => !removeTagIds.has(tid));
                                            return setChunkTags(id, filtered);
                                        }),
                                        { concurrency: 10 }
                                    );
                                })
                            );
                        }),
                        Effect.map(() => ({ updated: ids.length }))
                    );
                }
                case "set_type": {
                    if (!value) return Effect.fail(new ValidationError({ message: "value is required for set_type" }));
                    return updateManyChunks(ids, userId, { type: value }).pipe(
                        Effect.map(result => ({ updated: result.length }))
                    );
                }
                case "set_codebase": {
                    const codebaseIds = value ? [value] : [];
                    return Effect.all(
                        ids.map(id => setChunkCodebases(id, codebaseIds)),
                        { concurrency: 10 }
                    ).pipe(Effect.map(() => ({ updated: ids.length })));
                }
                case "set_review_status": {
                    if (!value || !["draft", "reviewed", "approved"].includes(value)) {
                        return Effect.fail(new ValidationError({ message: "value must be draft, reviewed, or approved" }));
                    }
                    return updateManyChunks(ids, userId, { reviewStatus: value }).pipe(
                        Effect.map(result => ({ updated: result.length }))
                    );
                }
                case "archive": {
                    return archiveMany(ids, userId).pipe(
                        Effect.map(result => ({ updated: result.length }))
                    );
                }
                case "delete": {
                    return deleteMany(ids, userId).pipe(
                        Effect.map(result => ({ updated: result.length }))
                    );
                }
                default:
                    return Effect.fail(new ValidationError({ message: `Unknown action: ${action}` }));
            }
        })
    );
}
