import { and, asc, count, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkProposal, type ChunkProposal, type NewChunkProposal, type ProposedChanges } from "../schema/chunk-proposal";

export type { ProposedChanges };

export function createProposal(input: NewChunkProposal): Effect.Effect<ChunkProposal, DatabaseError> {
    return dbEffect(async () => {
        const [row] = await db.insert(chunkProposal).values(input).returning();
        if (!row) throw new Error("Insert returned no row");
        return row;
    });
}

/**
 * SECURITY: `userId` scopes through the proposal's chunk. A proposal has no
 * owner column of its own — `proposedBy` is whoever suggested the change,
 * which may be an AI agent or another user — so the authority is the chunk
 * being proposed against. Without this, GET /proposals/:proposalId returned
 * any proposal to any authenticated caller, including the proposed content.
 */
export function getProposalById(id: string, userId: string): Effect.Effect<ChunkProposal | null, DatabaseError> {
    return dbEffect(async () => {
        const [row] = await db
            .select({
                id: chunkProposal.id,
                chunkId: chunkProposal.chunkId,
                changes: chunkProposal.changes,
                reason: chunkProposal.reason,
                status: chunkProposal.status,
                proposedBy: chunkProposal.proposedBy,
                reviewedBy: chunkProposal.reviewedBy,
                reviewedAt: chunkProposal.reviewedAt,
                reviewNote: chunkProposal.reviewNote,
                createdAt: chunkProposal.createdAt
            })
            .from(chunkProposal)
            .innerJoin(chunk, eq(chunk.id, chunkProposal.chunkId))
            .where(and(eq(chunkProposal.id, id), eq(chunk.userId, userId)))
            .limit(1);
        return row ?? null;
    });
}

export interface ListProposalsFilter {
    /** SECURITY: required. Scopes the list to proposals against the caller's
     *  own chunks — this used to be absent entirely, so GET /proposals
     *  returned every user's pending proposals, proposed content included. */
    userId: string;
    chunkId?: string;
    status?: string;
    limit?: number;
    offset?: number;
}

export function listProposals(
    filter: ListProposalsFilter
): Effect.Effect<Array<ChunkProposal & { chunkTitle: string; chunkType: string }>, DatabaseError> {
    return dbEffect(async () => {
        const conditions = [eq(chunk.userId, filter.userId)];
        if (filter.chunkId) conditions.push(eq(chunkProposal.chunkId, filter.chunkId));
        if (filter.status) conditions.push(eq(chunkProposal.status, filter.status));

        const rows = await db
            .select({
                id: chunkProposal.id,
                chunkId: chunkProposal.chunkId,
                changes: chunkProposal.changes,
                reason: chunkProposal.reason,
                status: chunkProposal.status,
                proposedBy: chunkProposal.proposedBy,
                reviewedBy: chunkProposal.reviewedBy,
                reviewedAt: chunkProposal.reviewedAt,
                reviewNote: chunkProposal.reviewNote,
                createdAt: chunkProposal.createdAt,
                chunkTitle: chunk.title,
                chunkType: chunk.type
            })
            .from(chunkProposal)
            .innerJoin(chunk, eq(chunk.id, chunkProposal.chunkId))
            .where(conditions.length > 0 ? and(...conditions) : undefined)
            .orderBy(desc(chunkProposal.createdAt))
            .limit(filter.limit ?? 50)
            .offset(filter.offset ?? 0);

        return rows;
    });
}

/** SECURITY: scoped through the chunk's owner — see `getProposalById`. */
export function listProposalsForChunk(chunkId: string, userId: string, status?: string): Effect.Effect<ChunkProposal[], DatabaseError> {
    return dbEffect(async () => {
        const conditions = [eq(chunkProposal.chunkId, chunkId), eq(chunk.userId, userId)];
        if (status) conditions.push(eq(chunkProposal.status, status));

        const rows = await db
            .select({
                id: chunkProposal.id,
                chunkId: chunkProposal.chunkId,
                changes: chunkProposal.changes,
                reason: chunkProposal.reason,
                status: chunkProposal.status,
                proposedBy: chunkProposal.proposedBy,
                reviewedBy: chunkProposal.reviewedBy,
                reviewedAt: chunkProposal.reviewedAt,
                reviewNote: chunkProposal.reviewNote,
                createdAt: chunkProposal.createdAt
            })
            .from(chunkProposal)
            .innerJoin(chunk, eq(chunk.id, chunkProposal.chunkId))
            .where(and(...conditions))
            .orderBy(asc(chunkProposal.createdAt));
        return rows;
    });
}

export function updateProposalStatus(
    id: string,
    status: string,
    reviewedBy: string,
    reviewNote?: string
): Effect.Effect<ChunkProposal, DatabaseError> {
    return dbEffect(async () => {
        const [row] = await db
            .update(chunkProposal)
            .set({
                status,
                reviewedBy,
                reviewedAt: new Date(),
                reviewNote: reviewNote ?? null
            })
            .where(eq(chunkProposal.id, id))
            .returning();
        if (!row) throw new Error("Proposal not found");
        return row;
    });
}

/** SECURITY: counts only the caller's own pending proposals — this used to
 *  be a global count, leaking how much review traffic other users had. */
export function getPendingCount(userId: string): Effect.Effect<number, DatabaseError> {
    return dbEffect(async () => {
        const [row] = await db
            .select({ count: count() })
            .from(chunkProposal)
            .innerJoin(chunk, eq(chunk.id, chunkProposal.chunkId))
            .where(and(eq(chunkProposal.status, "pending"), eq(chunk.userId, userId)));
        return row?.count ?? 0;
    });
}
