import { desc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
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
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(chunkVersion).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getVersionsByChunkId(chunkId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(chunkVersion).where(eq(chunkVersion.chunkId, chunkId)).orderBy(desc(chunkVersion.version)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getNextVersionNumber(chunkId: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await db
                .select({ maxVersion: sql<number>`COALESCE(MAX(${chunkVersion.version}), 0)` })
                .from(chunkVersion)
                .where(eq(chunkVersion.chunkId, chunkId));
            return (result[0]?.maxVersion ?? 0) + 1;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
