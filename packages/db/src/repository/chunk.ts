import { and, desc, eq, ilike, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export interface ListChunksParams {
    userId: string;
    type?: string;
    search?: string;
    limit: number;
    offset: number;
}

export function listChunks(params: ListChunksParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.userId, params.userId)];
            if (params.type) {
                conditions.push(eq(chunk.type, params.type));
            }
            if (params.search) {
                conditions.push(
                    or(
                        sql`${chunk.title} % ${params.search}`,
                        sql`${chunk.content} % ${params.search}`,
                        ilike(chunk.title, `%${params.search}%`),
                        ilike(chunk.content, `%${params.search}%`)
                    )!
                );
            }
            const orderClause = params.search
                ? sql`similarity(${chunk.title}, ${params.search}) DESC`
                : desc(chunk.updatedAt);
            const chunks = await db
                .select()
                .from(chunk)
                .where(and(...conditions))
                .orderBy(orderClause)
                .limit(params.limit)
                .offset(params.offset);

            const total = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...conditions));

            return { chunks, total: Number(total[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunkById(chunkId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(chunk)
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunkConnections(chunkId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkConnection.id,
                    targetId: chunkConnection.targetId,
                    sourceId: chunkConnection.sourceId,
                    relation: chunkConnection.relation,
                    title: chunk.title
                })
                .from(chunkConnection)
                .leftJoin(
                    chunk,
                    or(
                        and(eq(chunkConnection.targetId, chunk.id), eq(chunkConnection.sourceId, chunkId)),
                        and(eq(chunkConnection.sourceId, chunk.id), eq(chunkConnection.targetId, chunkId))
                    )
                )
                .where(or(eq(chunkConnection.sourceId, chunkId), eq(chunkConnection.targetId, chunkId))),
        catch: cause => new DatabaseError({ cause })
    });
}

export interface CreateChunkParams {
    id: string;
    title: string;
    content: string;
    type: string;
    tags: string[];
    userId: string;
}

export function createChunk(params: CreateChunkParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(chunk).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateChunkParams {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
}

export function updateChunk(chunkId: string, params: UpdateChunkParams) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunk)
                .set({
                    ...(params.title !== undefined && { title: params.title }),
                    ...(params.content !== undefined && { content: params.content }),
                    ...(params.type !== undefined && { type: params.type }),
                    ...(params.tags !== undefined && { tags: params.tags })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function exportAllChunks(userId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(chunk).where(eq(chunk.userId, userId)).orderBy(desc(chunk.createdAt)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteChunk(chunkId: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(chunk)
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
