import { eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export function getAllChunksMeta(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({ id: chunk.id, title: chunk.title, type: chunk.type, tags: chunk.tags })
                .from(chunk)
                .where(eq(chunk.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getAllConnectionsForUser(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: chunkConnection.id,
                    sourceId: chunkConnection.sourceId,
                    targetId: chunkConnection.targetId,
                    relation: chunkConnection.relation
                })
                .from(chunkConnection)
                .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
                .where(eq(chunk.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}
