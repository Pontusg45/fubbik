import { and, desc, eq, isNotNull, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkVersion } from "../schema/chunk-version";

export interface CreateVersionParams {
    id: string;
    chunkId: string;
    version: number;
    title: string;
    content: string;
    type: string;
    tags: string[];
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
    scope?: Record<string, string> | null;
    updateTag?: string | null;
}

export function createVersion(params: CreateVersionParams) {
    return dbEffect(async () => {
        const [created] = await db.insert(chunkVersion).values(params).returning();
        return created;
    });
}

export function getVersionsByChunkId(chunkId: string) {
    return dbEffect(() =>
        db.select().from(chunkVersion).where(eq(chunkVersion.chunkId, chunkId)).orderBy(desc(chunkVersion.version))
    );
}

export function getNextVersionNumber(chunkId: string) {
    return dbEffect(async () => {
        const result = await db
            .select({ maxVersion: sql<number>`COALESCE(MAX(${chunkVersion.version}), 0)` })
            .from(chunkVersion)
            .where(eq(chunkVersion.chunkId, chunkId));
        return (result[0]?.maxVersion ?? 0) + 1;
    });
}

export function getVersionsByTag(tag: string, userId: string, codebaseId?: string) {
    return dbEffect(async () => {
        const conditions = [
            eq(chunkVersion.updateTag, tag),
            eq(chunk.userId, userId)
        ];
        if (codebaseId) {
            conditions.push(
                sql`EXISTS (SELECT 1 FROM chunk_codebase WHERE chunk_codebase.chunk_id = ${chunkVersion.chunkId} AND chunk_codebase.codebase_id = ${codebaseId})`
            );
        }

        const versions = await db
            .select({
                versionId: chunkVersion.id,
                chunkId: chunkVersion.chunkId,
                version: chunkVersion.version,
                updateTag: chunkVersion.updateTag,
                title: chunkVersion.title,
                content: chunkVersion.content,
                type: chunkVersion.type,
                rationale: chunkVersion.rationale,
                alternatives: chunkVersion.alternatives,
                consequences: chunkVersion.consequences,
                scope: chunkVersion.scope,
                createdAt: chunkVersion.createdAt,
                chunkTitle: chunk.title,
                chunkContent: chunk.content,
                chunkType: chunk.type,
                chunkRationale: chunk.rationale,
                chunkAlternatives: chunk.alternatives,
                chunkConsequences: chunk.consequences,
                chunkScope: chunk.scope,
            })
            .from(chunkVersion)
            .innerJoin(chunk, eq(chunk.id, chunkVersion.chunkId))
            .where(and(...conditions))
            .orderBy(desc(chunkVersion.createdAt));

        return versions;
    });
}

export function getDistinctUpdateTags(userId: string, codebaseId?: string) {
    return dbEffect(async () => {
        const conditions = [
            isNotNull(chunkVersion.updateTag),
            eq(chunk.userId, userId)
        ];
        if (codebaseId) {
            conditions.push(
                sql`EXISTS (SELECT 1 FROM chunk_codebase WHERE chunk_codebase.chunk_id = ${chunkVersion.chunkId} AND chunk_codebase.codebase_id = ${codebaseId})`
            );
        }

        const result = await db
            .select({
                tag: chunkVersion.updateTag,
                count: sql<number>`count(*)`.as("count"),
            })
            .from(chunkVersion)
            .innerJoin(chunk, eq(chunk.id, chunkVersion.chunkId))
            .where(and(...conditions))
            .groupBy(chunkVersion.updateTag)
            .orderBy(chunkVersion.updateTag);

        return result as { tag: string; count: number }[];
    });
}
