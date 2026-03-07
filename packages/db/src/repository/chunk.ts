import { and, asc, desc, eq, ilike, inArray, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export interface ListChunksParams {
    userId?: string;
    type?: string;
    search?: string;
    exclude?: string[];
    scope?: Record<string, string>;
    alias?: string;
    sort?: "newest" | "oldest" | "alpha" | "updated";
    limit: number;
    offset: number;
}

export function listChunks(params: ListChunksParams) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = params.userId ? [eq(chunk.userId, params.userId)] : [];
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
            if (params.exclude?.length) {
                for (const term of params.exclude) {
                    conditions.push(sql`NOT (${chunk.notAbout} @> ${JSON.stringify([term])}::jsonb)`);
                }
            }
            if (params.scope && Object.keys(params.scope).length > 0) {
                conditions.push(sql`${chunk.scope} @> ${JSON.stringify(params.scope)}::jsonb`);
            }
            if (params.alias) {
                conditions.push(sql`${chunk.aliases} @> ${JSON.stringify([params.alias])}::jsonb`);
            }
            const orderClause = (() => {
                if (params.search) return sql`similarity(${chunk.title}, ${params.search}) DESC`;
                switch (params.sort) {
                    case "oldest": return asc(chunk.createdAt);
                    case "alpha": return asc(chunk.title);
                    case "updated": return desc(chunk.updatedAt);
                    case "newest":
                    default: return desc(chunk.createdAt);
                }
            })();
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

export function getChunkById(chunkId: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.id, chunkId)];
            if (userId) conditions.push(eq(chunk.userId, userId));
            const [found] = await db
                .select()
                .from(chunk)
                .where(and(...conditions));
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
    summary?: string | null;
    aliases?: string[];
    notAbout?: string[];
    scope?: Record<string, string>;
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
                    ...(params.tags !== undefined && { tags: params.tags }),
                    ...(params.summary !== undefined && { summary: params.summary }),
                    ...(params.aliases !== undefined && { aliases: params.aliases }),
                    ...(params.notAbout !== undefined && { notAbout: params.notAbout }),
                    ...(params.scope !== undefined && { scope: params.scope })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function exportAllChunks(userId?: string) {
    return Effect.tryPromise({
        try: () => {
            const query = db.select().from(chunk).orderBy(desc(chunk.createdAt));
            return userId ? query.where(eq(chunk.userId, userId)) : query;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export interface EnrichChunkParams {
    summary?: string | null;
    aliases?: string[];
    notAbout?: string[];
    scope?: Record<string, string>;
    embedding?: number[];
}

export function updateChunkEnrichment(chunkId: string, params: EnrichChunkParams) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunk)
                .set({
                    ...(params.summary !== undefined && { summary: params.summary }),
                    ...(params.aliases !== undefined && { aliases: params.aliases }),
                    ...(params.notAbout !== undefined && { notAbout: params.notAbout }),
                    ...(params.scope !== undefined && { scope: params.scope }),
                    ...(params.embedding !== undefined && { embedding: params.embedding })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        },
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

export function deleteMany(ids: string[], userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .delete(chunk)
                .where(and(inArray(chunk.id, ids), eq(chunk.userId, userId)))
                .returning({ id: chunk.id }),
        catch: cause => new DatabaseError({ cause })
    });
}
