import { eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkConnection } from "../schema/chunk";

export function createConnection(params: { id: string; sourceId: string; targetId: string; relation: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(chunkConnection).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteConnection(connectionId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(chunkConnection)
                .where(eq(chunkConnection.id, connectionId))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getConnectionById(connectionId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(chunkConnection)
                .where(eq(chunkConnection.id, connectionId));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
