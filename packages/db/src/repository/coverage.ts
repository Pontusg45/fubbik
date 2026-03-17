import { and, eq, isNull, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { requirement, requirementChunk } from "../schema/requirement";

export function getChunkCoverage(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.userId, userId), isNull(chunk.archivedAt)];

            let chunkQuery;
            if (codebaseId) {
                chunkQuery = db
                    .select({
                        id: chunk.id,
                        title: chunk.title,
                        requirementCount: sql<number>`count(${requirementChunk.requirementId})`
                    })
                    .from(chunk)
                    .innerJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                    .leftJoin(requirementChunk, eq(requirementChunk.chunkId, chunk.id))
                    .where(and(...conditions, eq(chunkCodebase.codebaseId, codebaseId)))
                    .groupBy(chunk.id, chunk.title);
            } else {
                chunkQuery = db
                    .select({
                        id: chunk.id,
                        title: chunk.title,
                        requirementCount: sql<number>`count(${requirementChunk.requirementId})`
                    })
                    .from(chunk)
                    .leftJoin(requirementChunk, eq(requirementChunk.chunkId, chunk.id))
                    .where(and(...conditions))
                    .groupBy(chunk.id, chunk.title);
            }

            return chunkQuery;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunkCoverageMatrix(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            if (codebaseId) {
                return db
                    .select({
                        chunkId: requirementChunk.chunkId,
                        chunkTitle: chunk.title,
                        requirementId: requirementChunk.requirementId,
                        requirementTitle: requirement.title,
                        requirementStatus: requirement.status
                    })
                    .from(requirementChunk)
                    .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                    .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                    .innerJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                    .where(and(
                        eq(chunk.userId, userId),
                        isNull(chunk.archivedAt),
                        eq(chunkCodebase.codebaseId, codebaseId)
                    ));
            } else {
                return db
                    .select({
                        chunkId: requirementChunk.chunkId,
                        chunkTitle: chunk.title,
                        requirementId: requirementChunk.requirementId,
                        requirementTitle: requirement.title,
                        requirementStatus: requirement.status
                    })
                    .from(requirementChunk)
                    .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                    .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                    .where(and(
                        eq(chunk.userId, userId),
                        isNull(chunk.archivedAt)
                    ));
            }
        },
        catch: cause => new DatabaseError({ cause })
    });
}
