import { and, asc, count, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import {
    chunkProposal,
    type ChunkProposal,
    type NewChunkProposal,
} from "../schema/chunk-proposal";
import { chunk } from "../schema/chunk";

export function createProposal(input: NewChunkProposal): Effect.Effect<ChunkProposal, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(chunkProposal).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function getProposalById(id: string): Effect.Effect<ChunkProposal | null, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(chunkProposal).where(eq(chunkProposal.id, id)).limit(1);
            return row ?? null;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export interface ListProposalsFilter {
    chunkId?: string;
    status?: string;
    limit?: number;
    offset?: number;
}

export function listProposals(filter: ListProposalsFilter): Effect.Effect<
    Array<ChunkProposal & { chunkTitle: string; chunkType: string }>,
    DatabaseError
> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [];
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
                    chunkType: chunk.type,
                })
                .from(chunkProposal)
                .innerJoin(chunk, eq(chunk.id, chunkProposal.chunkId))
                .where(conditions.length > 0 ? and(...conditions) : undefined)
                .orderBy(desc(chunkProposal.createdAt))
                .limit(filter.limit ?? 50)
                .offset(filter.offset ?? 0);

            return rows;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function listProposalsForChunk(
    chunkId: string,
    status?: string,
): Effect.Effect<ChunkProposal[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunkProposal.chunkId, chunkId)];
            if (status) conditions.push(eq(chunkProposal.status, status));

            return db
                .select()
                .from(chunkProposal)
                .where(and(...conditions))
                .orderBy(asc(chunkProposal.createdAt));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function updateProposalStatus(
    id: string,
    status: string,
    reviewedBy: string,
    reviewNote?: string,
): Effect.Effect<ChunkProposal, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(chunkProposal)
                .set({
                    status,
                    reviewedBy,
                    reviewedAt: new Date(),
                    reviewNote: reviewNote ?? null,
                })
                .where(eq(chunkProposal.id, id))
                .returning();
            if (!row) throw new Error("Proposal not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function getPendingCount(): Effect.Effect<number, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .select({ count: count() })
                .from(chunkProposal)
                .where(eq(chunkProposal.status, "pending"));
            return row?.count ?? 0;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}
