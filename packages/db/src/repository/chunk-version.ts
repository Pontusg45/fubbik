import { desc, eq, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunkVersion } from "../schema/chunk-version";

export interface CreateVersionParams {
    id: string;
    chunkId: string;
    version: number;
    title: string;
    content: string;
    type: string;
    tags: string[];
}

export function createVersion(params: CreateVersionParams) {
    return dbEffect(async () => {
            const [created] = await db.insert(chunkVersion).values(params).returning();
            return created;
        });
}

export function getVersionsByChunkId(chunkId: string) {
    return dbEffect(() => db.select().from(chunkVersion).where(eq(chunkVersion.chunkId, chunkId)).orderBy(desc(chunkVersion.version)));
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
