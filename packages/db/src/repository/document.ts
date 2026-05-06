import { and, eq, ilike, isNull, or, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { document } from "../schema/document";
import { chunkTag, tag } from "../schema/tag";

export interface CreateDocumentParams {
    id: string;
    title: string;
    sourcePath: string;
    contentHash: string;
    description?: string;
    codebaseId?: string;
    userId: string;
    splitLevel?: number;
}

export function createDocument(params: CreateDocumentParams) {
    return dbEffect(async () => {
            const [created] = await db.insert(document).values(params).returning();
            return created;
        });
}

export function getDocumentById(id: string) {
    return dbEffect(async () => {
            const [doc] = await db.select().from(document).where(eq(document.id, id));
            return doc ?? null;
        });
}

export function getDocumentBySourcePath(sourcePath: string, codebaseId: string | undefined, userId: string) {
    return dbEffect(async () => {
            const conditions = [eq(document.sourcePath, sourcePath), eq(document.userId, userId)];
            if (codebaseId) {
                conditions.push(eq(document.codebaseId, codebaseId));
            } else {
                conditions.push(isNull(document.codebaseId));
            }
            const [doc] = await db.select().from(document).where(and(...conditions));
            return doc ?? null;
        });
}

export function listDocuments(userId: string, codebaseId?: string) {
    return dbEffect(async () => {
            const conditions = [eq(document.userId, userId)];
            if (codebaseId) conditions.push(eq(document.codebaseId, codebaseId));
            const docs = await db
                .select({
                    id: document.id,
                    title: document.title,
                    sourcePath: document.sourcePath,
                    contentHash: document.contentHash,
                    description: document.description,
                    codebaseId: document.codebaseId,
                    createdAt: document.createdAt,
                    updatedAt: document.updatedAt,
                    chunkCount: sql<number>`count(${chunk.id})`.as("chunk_count"),
                    lastChunkUpdatedAt: sql<Date>`max(${chunk.updatedAt})`.as("last_chunk_updated_at"),
                    oldestChunkUpdatedAt: sql<Date>`min(${chunk.updatedAt})`.as("oldest_chunk_updated_at"),
                })
                .from(document)
                .leftJoin(chunk, eq(chunk.documentId, document.id))
                .where(and(...conditions))
                .groupBy(document.id)
                .orderBy(document.title);
            return docs;
        });
}

export function listDocumentsWithTags(userId: string, codebaseId?: string) {
    return dbEffect(async () => {
        const conditions = [eq(document.userId, userId)];
        if (codebaseId) conditions.push(eq(document.codebaseId, codebaseId));

        const docs = await db
            .select({
                id: document.id,
                title: document.title,
                sourcePath: document.sourcePath,
                contentHash: document.contentHash,
                description: document.description,
                codebaseId: document.codebaseId,
                createdAt: document.createdAt,
                updatedAt: document.updatedAt,
                chunkCount: sql<number>`count(distinct ${chunk.id})`.as("chunk_count"),
                lastChunkUpdatedAt: sql<Date>`max(${chunk.updatedAt})`.as("last_chunk_updated_at"),
                oldestChunkUpdatedAt: sql<Date>`min(${chunk.updatedAt})`.as("oldest_chunk_updated_at"),
                type: sql<string>`min(case when ${chunk.documentOrder} = 0 then ${chunk.type} end)`.as("type"),
                tagsRaw: sql<string>`string_agg(distinct ${tag.name}, ',')`.as("tags_raw"),
            })
            .from(document)
            .leftJoin(chunk, eq(chunk.documentId, document.id))
            .leftJoin(chunkTag, eq(chunkTag.chunkId, chunk.id))
            .leftJoin(tag, eq(tag.id, chunkTag.tagId))
            .where(and(...conditions))
            .groupBy(document.id)
            .orderBy(document.title);

        return docs.map(d => ({
            ...d,
            type: d.type ?? "document",
            tags: d.tagsRaw ? d.tagsRaw.split(",").filter(Boolean) : [],
            tagsRaw: undefined,
        }));
    });
}

export function updateDocument(id: string, params: { title?: string; contentHash?: string; description?: string; splitLevel?: number }) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(document)
                .set({
                    ...(params.title !== undefined && { title: params.title }),
                    ...(params.contentHash !== undefined && { contentHash: params.contentHash }),
                    ...(params.description !== undefined && { description: params.description }),
                    ...(params.splitLevel !== undefined && { splitLevel: params.splitLevel })
                })
                .where(eq(document.id, id))
                .returning();
            return updated;
        });
}

export function deleteDocument(id: string) {
    return dbEffect(async () => {
            await db.update(chunk).set({ documentId: null, documentOrder: null }).where(eq(chunk.documentId, id));
            const [deleted] = await db.delete(document).where(eq(document.id, id)).returning();
            return deleted;
        });
}

export function searchDocumentChunks(userId: string, query: string, limit = 20) {
    return dbEffect(async () => {
            const results = await db
                .select({
                    chunkId: chunk.id,
                    chunkTitle: chunk.title,
                    chunkContent: chunk.content,
                    documentOrder: chunk.documentOrder,
                    documentId: document.id,
                    documentTitle: document.title,
                    sourcePath: document.sourcePath
                })
                .from(chunk)
                .innerJoin(document, eq(chunk.documentId, document.id))
                .where(
                    and(
                        eq(document.userId, userId),
                        or(
                            ilike(chunk.title, `%${query}%`),
                            ilike(chunk.content, `%${query}%`)
                        )
                    )
                )
                .orderBy(document.title, chunk.documentOrder)
                .limit(limit);
            return results;
        });
}

export function getDocumentChunks(documentId: string) {
    return dbEffect(async () => {
            const chunks = await db
                .select()
                .from(chunk)
                .where(eq(chunk.documentId, documentId))
                .orderBy(chunk.documentOrder);
            return chunks;
        });
}
