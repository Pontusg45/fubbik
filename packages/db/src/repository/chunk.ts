import { and, asc, desc, eq, getTableColumns, gte, ilike, inArray, isNotNull, isNull, or, sql } from "drizzle-orm";
import { Effect } from "effect";

import { ensureVertex, deleteVertex } from "../age/sync";
import { db, dbEffect } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";
import { codebase, chunkCodebase } from "../schema/codebase";
import { tag, chunkTag } from "../schema/tag";
import { workspaceCodebase } from "../schema/workspace";

export interface ListChunksParams {
    userId?: string;
    type?: string;
    search?: string;
    exclude?: string[];
    scope?: Record<string, string>;
    alias?: string;
    tags?: string[];
    sort?: "newest" | "oldest" | "alpha" | "updated";
    after?: Date;
    enrichment?: "missing" | "complete";
    minConnections?: number;
    codebaseId?: string;
    workspaceId?: string;
    globalOnly?: boolean;
    origin?: string;
    reviewStatus?: string;
    includeArchived?: boolean;
    limit: number;
    offset: number;
}

export function listChunks(params: ListChunksParams) {
    return dbEffect(async () => {
            const conditions = params.userId ? [eq(chunk.userId, params.userId)] : [];
            if (!params.includeArchived) {
                conditions.push(isNull(chunk.archivedAt));
            }
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
            if (params.tags && params.tags.length > 0) {
                const tagSubquery = db
                    .select({ chunkId: chunkTag.chunkId })
                    .from(chunkTag)
                    .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                    .where(inArray(tag.name, params.tags));
                conditions.push(sql`${chunk.id} IN (${tagSubquery})`);
            }
            if (params.after) {
                conditions.push(gte(chunk.updatedAt, params.after));
            }
            if (params.minConnections && params.minConnections > 0) {
                conditions.push(
                    sql`(
                        SELECT COUNT(*) FROM ${chunkConnection}
                        WHERE ${chunkConnection.sourceId} = ${chunk.id}
                           OR ${chunkConnection.targetId} = ${chunk.id}
                    ) >= ${params.minConnections}`
                );
            }
            if (params.workspaceId) {
                const inWorkspace = db
                    .select({ codebaseId: workspaceCodebase.codebaseId })
                    .from(workspaceCodebase)
                    .where(eq(workspaceCodebase.workspaceId, params.workspaceId));
                const inCodebases = db
                    .select({ chunkId: chunkCodebase.chunkId })
                    .from(chunkCodebase)
                    .where(sql`${chunkCodebase.codebaseId} IN (${inWorkspace})`);
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(
                    or(sql`${chunk.id} IN (${inCodebases})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!
                );
            } else if (params.codebaseId) {
                const inCodebase = db
                    .select({ chunkId: chunkCodebase.chunkId })
                    .from(chunkCodebase)
                    .where(eq(chunkCodebase.codebaseId, params.codebaseId));
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(
                    or(sql`${chunk.id} IN (${inCodebase})`, sql`${chunk.id} NOT IN (${inAnyCodebase})`)!
                );
            }
            if (params.globalOnly) {
                const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
                conditions.push(sql`${chunk.id} NOT IN (${inAnyCodebase})`);
            }
            if (params.origin) {
                conditions.push(eq(chunk.origin, params.origin));
            }
            if (params.reviewStatus) {
                conditions.push(eq(chunk.reviewStatus, params.reviewStatus));
            }
            if (params.enrichment === "missing") {
                conditions.push(or(isNull(chunk.summary), isNull(chunk.embedding), sql`jsonb_array_length(${chunk.aliases}) = 0`)!);
            } else if (params.enrichment === "complete") {
                conditions.push(isNotNull(chunk.summary), isNotNull(chunk.embedding));
            }
            const orderClause = (() => {
                if (params.search) return sql`similarity(${chunk.title}, ${params.search}) DESC`;
                switch (params.sort) {
                    case "oldest":
                        return asc(chunk.createdAt);
                    case "alpha":
                        return asc(chunk.title);
                    case "updated":
                        return desc(chunk.updatedAt);
                    case "newest":
                    default:
                        return desc(chunk.createdAt);
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
        });
}

export interface ListChunksWithCodebaseParams {
    userId?: string;
    type?: string;
    search?: string;
    tags?: string[];
    sort?: "newest" | "oldest" | "alpha" | "updated";
    enrichment?: "missing" | "complete";
    origin?: string;
    reviewStatus?: string;
    limit: number;
    offset: number;
}

export function listChunksWithCodebase(params: ListChunksWithCodebaseParams) {
    return dbEffect(async () => {
            const conditions = params.userId ? [eq(chunk.userId, params.userId)] : [];
            conditions.push(isNull(chunk.archivedAt));
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
            if (params.origin) {
                conditions.push(eq(chunk.origin, params.origin));
            }
            if (params.reviewStatus) {
                conditions.push(eq(chunk.reviewStatus, params.reviewStatus));
            }
            if (params.enrichment === "missing") {
                conditions.push(or(isNull(chunk.summary), isNull(chunk.embedding), sql`jsonb_array_length(${chunk.aliases}) = 0`)!);
            } else if (params.enrichment === "complete") {
                conditions.push(isNotNull(chunk.summary), isNotNull(chunk.embedding));
            }
            const orderClause = (() => {
                if (params.search) return sql`similarity(${chunk.title}, ${params.search}) DESC`;
                switch (params.sort) {
                    case "oldest":
                        return asc(chunk.createdAt);
                    case "alpha":
                        return asc(chunk.title);
                    case "updated":
                        return desc(chunk.updatedAt);
                    case "newest":
                    default:
                        return desc(chunk.createdAt);
                }
            })();
            const whereClause = and(...conditions);

            // Left join with codebase to get first codebase name per chunk
            const chunkCols = getTableColumns(chunk);
            const rows = await db
                .select({
                    ...chunkCols,
                    codebaseName: sql<string | null>`min(${codebase.name})`.as("codebase_name")
                })
                .from(chunk)
                .leftJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                .leftJoin(codebase, eq(codebase.id, chunkCodebase.codebaseId))
                .where(whereClause)
                .groupBy(chunk.id)
                .orderBy(orderClause)
                .limit(params.limit)
                .offset(params.offset);

            // Separate count query without join to avoid inflated totals
            const total = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(whereClause);

            const chunks = rows.map(row => {
                const { codebaseName, ...chunkData } = row;
                return { ...chunkData, codebaseName: codebaseName ?? null };
            });

            return { chunks, total: Number(total[0]?.count ?? 0) };
        });
}

export function getChunkById(chunkId: string, userId?: string) {
    return dbEffect(async () => {
            const conditions = [eq(chunk.id, chunkId)];
            if (userId) conditions.push(eq(chunk.userId, userId));
            const [found] = await db
                .select()
                .from(chunk)
                .where(and(...conditions));
            return found ?? null;
        });
}

export function getChunkConnections(chunkId: string) {
    return dbEffect(() =>
            db
                .select({
                    id: chunkConnection.id,
                    targetId: chunkConnection.targetId,
                    sourceId: chunkConnection.sourceId,
                    relation: chunkConnection.relation,
                    title: chunk.title,
                    codebaseName: codebase.name
                })
                .from(chunkConnection)
                .leftJoin(
                    chunk,
                    or(
                        and(eq(chunkConnection.targetId, chunk.id), eq(chunkConnection.sourceId, chunkId)),
                        and(eq(chunkConnection.sourceId, chunk.id), eq(chunkConnection.targetId, chunkId))
                    )
                )
                .leftJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                .leftJoin(codebase, eq(codebase.id, chunkCodebase.codebaseId))
                .where(or(eq(chunkConnection.sourceId, chunkId), eq(chunkConnection.targetId, chunkId))));
}

export interface CreateChunkParams {
    id: string;
    title: string;
    content: string;
    type: string;
    userId: string;
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
    origin?: string;
    reviewStatus?: string;
    documentId?: string;
    documentOrder?: number;
}

export function createChunk(params: CreateChunkParams) {
    return dbEffect(async () => {
            const [created] = await db.insert(chunk).values({ ...params, title: params.title.trim() }).returning();
            await Effect.runPromise(
                ensureVertex("chunk", created!.id).pipe(
                    Effect.catchAll(() => Effect.succeed(undefined))
                )
            );
            return created;
        });
}

export interface UpdateChunkParams {
    title?: string;
    content?: string;
    type?: string;
    summary?: string | null;
    aliases?: string[];
    notAbout?: string[];
    scope?: Record<string, string>;
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
    origin?: string;
    reviewStatus?: string;
    reviewedBy?: string | null;
    reviewedAt?: Date | null;
    documentOrder?: number;
    isEntryPoint?: boolean;
}

export function updateChunk(chunkId: string, params: UpdateChunkParams) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(chunk)
                .set({
                    ...(params.title !== undefined && { title: params.title.trim() }),
                    ...(params.content !== undefined && { content: params.content }),
                    ...(params.type !== undefined && { type: params.type }),
                    ...(params.summary !== undefined && { summary: params.summary }),
                    ...(params.aliases !== undefined && { aliases: params.aliases }),
                    ...(params.notAbout !== undefined && { notAbout: params.notAbout }),
                    ...(params.scope !== undefined && { scope: params.scope }),
                    ...(params.rationale !== undefined && { rationale: params.rationale }),
                    ...(params.alternatives !== undefined && { alternatives: params.alternatives }),
                    ...(params.consequences !== undefined && { consequences: params.consequences }),
                    ...(params.origin !== undefined && { origin: params.origin }),
                    ...(params.reviewStatus !== undefined && { reviewStatus: params.reviewStatus }),
                    ...(params.reviewedBy !== undefined && { reviewedBy: params.reviewedBy }),
                    ...(params.reviewedAt !== undefined && { reviewedAt: params.reviewedAt }),
                    ...(params.documentOrder !== undefined && { documentOrder: params.documentOrder }),
                    ...(params.isEntryPoint !== undefined && { isEntryPoint: params.isEntryPoint })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        });
}

export function exportAllChunks(userId?: string) {
    return dbEffect(() => {
            const query = db.select().from(chunk).orderBy(desc(chunk.createdAt));
            return userId ? query.where(eq(chunk.userId, userId)) : query;
        });
}

export interface EnrichChunkParams {
    summary?: string | null;
    aliases?: string[];
    notAbout?: string[];
    scope?: Record<string, string>;
    embedding?: number[];
    embeddingUpdatedAt?: Date;
}

export function updateChunkEnrichment(chunkId: string, params: EnrichChunkParams) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(chunk)
                .set({
                    ...(params.summary !== undefined && { summary: params.summary }),
                    ...(params.aliases !== undefined && { aliases: params.aliases }),
                    ...(params.notAbout !== undefined && { notAbout: params.notAbout }),
                    ...(params.scope !== undefined && { scope: params.scope }),
                    ...(params.embedding !== undefined && { embedding: params.embedding, embeddingUpdatedAt: new Date() })
                })
                .where(eq(chunk.id, chunkId))
                .returning();
            return updated;
        });
}

export function deleteChunk(chunkId: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(chunk)
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
                .returning();
            if (deleted) {
                await Effect.runPromise(
                    deleteVertex("chunk", chunkId).pipe(
                        Effect.catchAll(() => Effect.succeed(undefined))
                    )
                );
            }
            return deleted ?? null;
        });
}

/**
 * Merge `sourceId` into `targetId` for `userId`:
 *   - reparent chunk_tag, chunk_codebase, chunk_file_ref, chunk_applies_to,
 *     chunk_connection (both endpoints), plan_task_chunk, plan_analyze_item,
 *     and favorite rows from source → target
 *   - dedupe against existing target rows wherever a unique index would fire
 *   - append the source's content to the target under a heading
 *   - delete the source (cascade cleans up versions / staleness / proposals)
 *
 * Returns the updated target chunk so callers can refresh their cache.
 */
export function mergeChunks(sourceId: string, targetId: string, userId: string) {
    return dbEffect(async () => {
            return await db.transaction(async tx => {
                const rows = await tx
                    .select()
                    .from(chunk)
                    .where(and(inArray(chunk.id, [sourceId, targetId]), eq(chunk.userId, userId)));
                const source = rows.find(r => r.id === sourceId);
                const target = rows.find(r => r.id === targetId);
                if (!source || !target) {
                    throw new Error("source or target chunk not found for user");
                }

                // --- chunk_tag (unique on (chunk_id, tag_id)) ---
                await tx.execute(sql`
                    INSERT INTO chunk_tag (chunk_id, tag_id)
                    SELECT ${targetId}, tag_id FROM chunk_tag
                    WHERE chunk_id = ${sourceId}
                    ON CONFLICT (chunk_id, tag_id) DO NOTHING
                `);
                await tx.execute(sql`DELETE FROM chunk_tag WHERE chunk_id = ${sourceId}`);

                // --- chunk_codebase (unique on (chunk_id, codebase_id)) ---
                await tx.execute(sql`
                    INSERT INTO chunk_codebase (chunk_id, codebase_id)
                    SELECT ${targetId}, codebase_id FROM chunk_codebase
                    WHERE chunk_id = ${sourceId}
                    ON CONFLICT (chunk_id, codebase_id) DO NOTHING
                `);
                await tx.execute(sql`DELETE FROM chunk_codebase WHERE chunk_id = ${sourceId}`);

                // --- chunk_connection: repoint sources then targets; dedupe on the
                // (source,target,relation) unique index; drop self-loops produced
                // by the swap.
                await tx.execute(sql`
                    UPDATE chunk_connection SET source_id = ${targetId}
                    WHERE source_id = ${sourceId}
                      AND NOT EXISTS (
                          SELECT 1 FROM chunk_connection c2
                          WHERE c2.source_id = ${targetId}
                            AND c2.target_id = chunk_connection.target_id
                            AND c2.relation = chunk_connection.relation
                      )
                `);
                await tx.execute(sql`DELETE FROM chunk_connection WHERE source_id = ${sourceId}`);
                await tx.execute(sql`
                    UPDATE chunk_connection SET target_id = ${targetId}
                    WHERE target_id = ${sourceId}
                      AND NOT EXISTS (
                          SELECT 1 FROM chunk_connection c2
                          WHERE c2.target_id = ${targetId}
                            AND c2.source_id = chunk_connection.source_id
                            AND c2.relation = chunk_connection.relation
                      )
                `);
                await tx.execute(sql`DELETE FROM chunk_connection WHERE target_id = ${sourceId}`);
                await tx.execute(sql`DELETE FROM chunk_connection WHERE source_id = target_id`);

                // --- chunk_file_ref / chunk_applies_to: simple re-parent ---
                await tx.execute(sql`UPDATE chunk_file_ref SET chunk_id = ${targetId} WHERE chunk_id = ${sourceId}`);
                await tx.execute(sql`UPDATE chunk_applies_to SET chunk_id = ${targetId} WHERE chunk_id = ${sourceId}`);

                // --- plan references ---
                await tx.execute(sql`UPDATE plan_task_chunk SET chunk_id = ${targetId} WHERE chunk_id = ${sourceId}`);
                await tx.execute(sql`UPDATE plan_analyze_item SET chunk_id = ${targetId} WHERE chunk_id = ${sourceId}`);

                // --- favorite (unique on (user_id, chunk_id)) ---
                await tx.execute(sql`
                    INSERT INTO favorite (user_id, chunk_id, created_at)
                    SELECT user_id, ${targetId}, created_at FROM favorite
                    WHERE chunk_id = ${sourceId}
                    ON CONFLICT (user_id, chunk_id) DO NOTHING
                `);
                await tx.execute(sql`DELETE FROM favorite WHERE chunk_id = ${sourceId}`);

                // --- content merge: append source body under a heading if distinct ---
                const separator = `\n\n## Merged from "${source.title}"\n\n`;
                const existingBody = target.content ?? "";
                const sourceBody = (source.content ?? "").trim();
                let mergedContent = existingBody;
                if (sourceBody && !existingBody.includes(sourceBody)) {
                    mergedContent = `${existingBody}${separator}${sourceBody}`;
                }

                const [updated] = await tx
                    .update(chunk)
                    .set({ content: mergedContent, updatedAt: new Date() })
                    .where(and(eq(chunk.id, targetId), eq(chunk.userId, userId)))
                    .returning();

                // --- finally: delete source (cascade handles chunk_version,
                // chunk_staleness, chunk_proposal, age vertex) ---
                await tx.delete(chunk).where(and(eq(chunk.id, sourceId), eq(chunk.userId, userId)));

                return updated!;
            });
        });
}

export function deleteMany(ids: string[], userId: string) {
    return dbEffect(async () => {
            const result = await db.delete(chunk).where(and(inArray(chunk.id, ids), eq(chunk.userId, userId))).returning({ id: chunk.id });
            for (const row of result) {
                await Effect.runPromise(
                    deleteVertex("chunk", row.id).pipe(
                        Effect.catchAll(() => Effect.succeed(undefined))
                    )
                );
            }
            return result;
        });
}

export function archiveChunk(chunkId: string, userId: string) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(chunk)
                .set({ archivedAt: new Date() })
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
                .returning();
            return updated ?? null;
        });
}

export function archiveMany(ids: string[], userId: string) {
    return dbEffect(() =>
            db
                .update(chunk)
                .set({ archivedAt: new Date() })
                .where(and(inArray(chunk.id, ids), eq(chunk.userId, userId)))
                .returning({ id: chunk.id }));
}

export function restoreChunk(chunkId: string, userId: string) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(chunk)
                .set({ archivedAt: null })
                .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
                .returning();
            return updated ?? null;
        });
}

export function listArchivedChunks(userId: string, codebaseId?: string) {
    return dbEffect(async () => {
            const conditions = [eq(chunk.userId, userId), isNotNull(chunk.archivedAt)];
            if (codebaseId) {
                const inCodebase = db
                    .select({ chunkId: chunkCodebase.chunkId })
                    .from(chunkCodebase)
                    .where(eq(chunkCodebase.codebaseId, codebaseId));
                conditions.push(sql`${chunk.id} IN (${inCodebase})`);
            }
            const chunks = await db
                .select()
                .from(chunk)
                .where(and(...conditions))
                .orderBy(desc(chunk.archivedAt));
            return chunks;
        });
}

export function updateManyChunks(ids: string[], userId: string, data: Partial<{ type: string; reviewStatus: string }>) {
    return dbEffect(() =>
            db
                .update(chunk)
                .set(data)
                .where(and(inArray(chunk.id, ids), eq(chunk.userId, userId)))
                .returning({ id: chunk.id }));
}

export function searchChunkTitles(prefix: string, limit = 10) {
    return dbEffect(() =>
            db
                .select({ id: chunk.id, title: chunk.title })
                .from(chunk)
                .where(ilike(chunk.title, `%${prefix}%`))
                .limit(limit));
}
