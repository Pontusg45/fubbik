import {
    archiveChunk as archiveChunkRepo,
    createChunk as createChunkRepo,
    createVersion,
    deleteChunk as deleteChunkRepo,
    deleteMany as deleteManyRepo,
    findOrCreateTag,
    getChunkById,
    getDocumentById,
    getNextVersionNumber,
    mergeChunks as mergeChunksRepo,
    listArchivedChunks as listArchivedChunksRepo,
    restoreChunk as restoreChunkRepo,
    setChunkCodebases,
    setChunkTags,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import type { DatabaseError } from "@fubbik/db/errors";
import { enrichChunk } from "../enrich/service";
import { NotFoundError, ValidationError } from "../errors";
import { events, EVENTS } from "../events/bus";
import { logger } from "../logger";
import { flagDownstreamStale } from "../staleness/detect-impact";

type ResolvedDocumentLinkage = {
    documentId: string | undefined;
    documentOrder: number | undefined;
};

function resolveDocumentLinkageForNewChunk(
    userId: string,
    rawDocumentId?: string,
    rawOrder?: number
): Effect.Effect<ResolvedDocumentLinkage, DatabaseError | ValidationError> {
    const trimmed =
        typeof rawDocumentId === "string" && rawDocumentId.trim().length > 0 ? rawDocumentId.trim() : undefined;

    if (!trimmed) {
        return Effect.succeed<ResolvedDocumentLinkage>({
            documentId: undefined,
            documentOrder: undefined
        });
    }

    return getDocumentById(trimmed).pipe(
        Effect.flatMap(doc => {
            if (!doc || doc.userId !== userId) {
                return Effect.fail(new ValidationError({ message: "Invalid or unknown document for documentId" }));
            }
            return Effect.succeed<ResolvedDocumentLinkage>({
                documentId: trimmed,
                documentOrder: rawOrder
            });
        })
    );
}

export function createChunk(
    userId: string,
    body: {
        title: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
        origin?: string;
        documentId?: string;
        documentOrder?: number;
        updateTag?: string;
    }
) {
    const id = crypto.randomUUID();
    const origin = body.origin ?? "human";
    return resolveDocumentLinkageForNewChunk(userId, body.documentId, body.documentOrder).pipe(
        Effect.flatMap(({ documentId, documentOrder }) =>
            createChunkRepo({
                id,
                title: body.title,
                content: body.content ?? "",
                type: body.type ?? "note",
                userId,
                rationale: body.rationale,
                alternatives: body.alternatives,
                consequences: body.consequences,
                origin,
                reviewStatus: origin === "ai" ? "draft" : "approved",
                documentId,
                documentOrder
            })
        ),
        Effect.tap(() => {
            if (body.tags && body.tags.length > 0) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(id, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.codebaseIds && body.codebaseIds.length > 0) {
                return setChunkCodebases(id, body.codebaseIds);
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            events.emit(EVENTS.CHUNK_CREATED, { chunkId: id, userId });
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.updateTag) {
                return createVersion({
                    id: crypto.randomUUID(),
                    chunkId: id,
                    version: 0,
                    title: "",
                    content: "",
                    type: "",
                    tags: [],
                    updateTag: body.updateTag
                });
            }
            return Effect.void;
        })
    );
}

export function updateChunk(
    chunkId: string,
    userId: string,
    body: {
        title?: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        summary?: string | null;
        aliases?: string[];
        notAbout?: string[];
        scope?: Record<string, string>;
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
        origin?: string;
        reviewStatus?: string;
        isEntryPoint?: boolean;
        updateTag?: string;
    }
) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(existing => Effect.all({ existing: Effect.succeed(existing), version: getNextVersionNumber(chunkId) })),
        Effect.flatMap(({ existing, version }) =>
            createVersion({
                id: crypto.randomUUID(),
                chunkId,
                version,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: [],
                rationale: existing.rationale,
                alternatives: existing.alternatives,
                consequences: existing.consequences,
                scope: existing.scope,
                updateTag: body.updateTag
            })
        ),
        Effect.flatMap(() => {
            const { tags: _tags, codebaseIds: _codebaseIds, updateTag: _updateTag, ...repoBody } = body;
            const updateData: Record<string, unknown> = { ...repoBody };
            if (body.reviewStatus !== undefined) {
                updateData.reviewedBy = userId;
                updateData.reviewedAt = new Date();
            }
            if (Object.keys(updateData).length === 0) return Effect.void;
            return updateChunkRepo(chunkId, updateData as Parameters<typeof updateChunkRepo>[1]).pipe(Effect.asVoid);
        }),
        Effect.tap(() => {
            if (body.tags) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(chunkId, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.codebaseIds) {
                return setChunkCodebases(chunkId, body.codebaseIds);
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.title !== undefined || body.content !== undefined) {
                Effect.runPromise(enrichChunk(chunkId)).catch(err => {
                    logger.error(`[enrich] Failed to re-enrich chunk ${chunkId}:`, { err });
                });
                // Fire-and-forget: flag downstream chunks as potentially stale
                Effect.runPromise(
                    flagDownstreamStale(chunkId, body.title ?? "Unknown", userId)
                ).catch(() => {});
            }
            events.emit(EVENTS.CHUNK_UPDATED, { chunkId, userId });
            return Effect.void;
        })
    );
}

export function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function deleteMany(ids: string[], userId: string) {
    return deleteManyRepo(ids, userId);
}

export function archiveChunk(chunkId: string, userId: string) {
    return archiveChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(archived => (archived ? Effect.succeed(archived) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function restoreChunk(chunkId: string, userId: string) {
    return restoreChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(restored => (restored ? Effect.succeed(restored) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function listArchivedChunks(userId: string, codebaseId?: string) {
    return listArchivedChunksRepo(userId, codebaseId);
}

/**
 * Merge two chunks belonging to the same user. See repo `mergeChunks` for
 * the reparenting semantics. Refuses same-id merges up front so the UI can
 * display a clean 400 rather than a cryptic DB error.
 */
export function mergeChunks(userId: string, sourceId: string, targetId: string) {
    return Effect.gen(function* () {
        if (sourceId === targetId) {
            return yield* Effect.fail(new ValidationError({ message: "Cannot merge a chunk into itself" }));
        }
        return yield* mergeChunksRepo(sourceId, targetId, userId);
    });
}
