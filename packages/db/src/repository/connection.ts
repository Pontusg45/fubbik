import { eq, inArray, or } from "drizzle-orm";
import { Effect } from "effect";

import { ensureVertex, createEdge, deleteEdge } from "../age/sync";
import { db, dbEffect } from "../index";
import { chunkConnection } from "../schema/chunk";

export function createConnection(params: { id: string; sourceId: string; targetId: string; relation: string; origin?: string; reviewStatus?: string }) {
    return dbEffect(async () => {
            const [created] = await db.insert(chunkConnection).values(params).returning();
            await Effect.runPromise(
                ensureVertex("chunk", params.sourceId).pipe(
                    Effect.flatMap(() => ensureVertex("chunk", params.targetId)),
                    Effect.flatMap(() => createEdge("connects", "chunk", params.sourceId, "chunk", params.targetId, { id: params.id, relation: params.relation })),
                    Effect.catchAll(() => Effect.succeed(undefined))
                )
            );
            return created;
        });
}

export function getConnectionsForChunks(chunkIds: string[]) {
    return dbEffect(() =>
            db
                .select()
                .from(chunkConnection)
                .where(
                    or(inArray(chunkConnection.sourceId, chunkIds), inArray(chunkConnection.targetId, chunkIds))
                ));
}

export function deleteConnection(connectionId: string) {
    return dbEffect(async () => {
            const [deleted] = await db.delete(chunkConnection).where(eq(chunkConnection.id, connectionId)).returning();
            if (deleted) {
                await Effect.runPromise(
                    deleteEdge("connects", { id: connectionId }).pipe(
                        Effect.catchAll(() => Effect.succeed(undefined))
                    )
                );
            }
            return deleted ?? null;
        });
}

export function getConnectionById(connectionId: string) {
    return dbEffect(async () => {
            const [found] = await db.select().from(chunkConnection).where(eq(chunkConnection.id, connectionId));
            return found ?? null;
        });
}
