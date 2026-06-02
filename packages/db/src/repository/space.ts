import { and, eq, inArray, sql } from "drizzle-orm";
import { Effect } from "effect";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { document } from "../schema/document";
import { plan } from "../schema/plan";
import { requirement } from "../schema/requirement";
import { space, chunkSpace } from "../schema/space";
import { spaceCodeMetadata } from "../schema/space-code-metadata";

export interface CreateSpaceParams {
    id: string;
    name: string;
    kind: string;
    description?: string;
    userId: string;
    code?: { remoteUrl?: string; localPaths?: string[] };
}

export function createSpace(params: CreateSpaceParams) {
    return dbEffect(async () => {
        const [created] = await db
            .insert(space)
            .values({
                id: params.id,
                name: params.name,
                kind: params.kind,
                description: params.description,
                userId: params.userId
            })
            .returning();
        if (!created) throw new Error("createSpace: insert returned no row");
        if (params.kind === "code" && params.code) {
            await db.insert(spaceCodeMetadata).values({
                spaceId: created.id,
                userId: params.userId,
                remoteUrl: params.code.remoteUrl,
                localPaths: params.code.localPaths ?? []
            });
        }
        return created;
    });
}

export function getSpaceById(spaceId: string, userId?: string) {
    return dbEffect(async () => {
        const conditions = [eq(space.id, spaceId)];
        if (userId) conditions.push(eq(space.userId, userId));
        const [found] = await db.select().from(space).where(and(...conditions));
        return found ?? null;
    });
}

export function getSpaceWithCodeMetadata(spaceId: string, userId?: string) {
    return dbEffect(async () => {
        const conditions = [eq(space.id, spaceId)];
        if (userId) conditions.push(eq(space.userId, userId));
        const [row] = await db
            .select({
                space,
                code: spaceCodeMetadata
            })
            .from(space)
            .leftJoin(spaceCodeMetadata, eq(spaceCodeMetadata.spaceId, space.id))
            .where(and(...conditions));
        return row ?? null;
    });
}

export function listSpaces(userId: string) {
    return dbEffect(() => db.select().from(space).where(eq(space.userId, userId)));
}

export function getCodeSpaceByRemoteUrl(remoteUrl: string, userId: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select({ space })
            .from(space)
            .innerJoin(spaceCodeMetadata, eq(spaceCodeMetadata.spaceId, space.id))
            .where(and(eq(spaceCodeMetadata.remoteUrl, remoteUrl), eq(space.userId, userId)));
        return row?.space ?? null;
    });
}

export function getCodeSpaceByLocalPath(localPath: string, userId: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select({ space })
            .from(space)
            .innerJoin(spaceCodeMetadata, eq(spaceCodeMetadata.spaceId, space.id))
            .where(
                and(
                    sql`${spaceCodeMetadata.localPaths} @> ${JSON.stringify([localPath])}::jsonb`,
                    eq(space.userId, userId)
                )
            );
        return row?.space ?? null;
    });
}

export interface UpdateSpaceParams {
    name?: string;
    description?: string | null;
    code?: { remoteUrl?: string | null; localPaths?: string[] };
}

export function updateSpace(spaceId: string, userId: string, params: UpdateSpaceParams) {
    return dbEffect(async () => {
        const setClause: Record<string, unknown> = {};
        if (params.name !== undefined) setClause.name = params.name;
        if (params.description !== undefined) setClause.description = params.description;

        const [updated] = Object.keys(setClause).length
            ? await db
                  .update(space)
                  .set(setClause)
                  .where(and(eq(space.id, spaceId), eq(space.userId, userId)))
                  .returning()
            : await db.select().from(space).where(and(eq(space.id, spaceId), eq(space.userId, userId)));

        if (params.code) {
            await db
                .insert(spaceCodeMetadata)
                .values({
                    spaceId,
                    userId,
                    remoteUrl: params.code.remoteUrl ?? null,
                    localPaths: params.code.localPaths ?? []
                })
                .onConflictDoUpdate({
                    target: spaceCodeMetadata.spaceId,
                    set: {
                        remoteUrl: params.code.remoteUrl ?? null,
                        localPaths: params.code.localPaths ?? []
                    }
                });
        }
        return updated ?? null;
    });
}

export function resetSpaceData(spaceId: string, userId: string) {
    return dbEffect(async () => {
        const exclusiveChunkIds = await db
            .select({ id: chunkSpace.chunkId })
            .from(chunkSpace)
            .where(eq(chunkSpace.spaceId, spaceId))
            .then(rows => rows.map(r => r.id));

        let chunksDeleted = 0;
        if (exclusiveChunkIds.length > 0) {
            const sharedChunkIds = await db
                .select({ id: chunkSpace.chunkId })
                .from(chunkSpace)
                .where(and(inArray(chunkSpace.chunkId, exclusiveChunkIds), sql`${chunkSpace.spaceId} != ${spaceId}`))
                .then(rows => new Set(rows.map(r => r.id)));
            const toDelete = exclusiveChunkIds.filter(id => !sharedChunkIds.has(id));
            if (toDelete.length > 0) {
                await db.delete(chunk).where(and(inArray(chunk.id, toDelete), eq(chunk.userId, userId)));
                chunksDeleted = toDelete.length;
            }
        }
        await db.delete(chunkSpace).where(eq(chunkSpace.spaceId, spaceId));

        const docsDeleted = await db
            .delete(document)
            .where(and(eq(document.spaceId, spaceId), eq(document.userId, userId)))
            .returning()
            .then(rows => rows.length);
        const plansDeleted = await db
            .delete(plan)
            .where(and(eq(plan.spaceId, spaceId), eq(plan.userId, userId)))
            .returning()
            .then(rows => rows.length);
        const requirementsDeleted = await db
            .delete(requirement)
            .where(and(eq(requirement.spaceId, spaceId), eq(requirement.userId, userId)))
            .returning()
            .then(rows => rows.length);
        return { chunksDeleted, docsDeleted, plansDeleted, requirementsDeleted };
    });
}

export function deleteSpace(spaceId: string, userId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(space)
            .where(and(eq(space.id, spaceId), eq(space.userId, userId)))
            .returning();
        return deleted ?? null;
    });
}

export function countChunksInSpace(spaceId: string) {
    return dbEffect(async () => {
        const [result] = await db
            .select({ count: sql<number>`count(*)` })
            .from(chunkSpace)
            .where(eq(chunkSpace.spaceId, spaceId));
        return Number(result?.count ?? 0);
    });
}

export function setChunkSpaces(chunkId: string, spaceIds: string[]) {
    return dbEffect(async () => {
        await db.delete(chunkSpace).where(eq(chunkSpace.chunkId, chunkId));
        if (spaceIds.length === 0) return [];
        return db
            .insert(chunkSpace)
            .values(spaceIds.map(spaceId => ({ chunkId, spaceId })))
            .onConflictDoNothing()
            .returning();
    });
}

export function getSpacesForChunk(chunkId: string) {
    return dbEffect(() =>
        db
            .select({ id: space.id, name: space.name, kind: space.kind })
            .from(chunkSpace)
            .innerJoin(space, eq(chunkSpace.spaceId, space.id))
            .where(eq(chunkSpace.chunkId, chunkId))
    );
}

export function getSpacesForChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([] as { chunkId: string; spaceId: string; spaceName: string }[]);
    return dbEffect(() =>
        db
            .select({ chunkId: chunkSpace.chunkId, spaceId: space.id, spaceName: space.name })
            .from(chunkSpace)
            .innerJoin(space, eq(chunkSpace.spaceId, space.id))
            .where(inArray(chunkSpace.chunkId, chunkIds))
    );
}
