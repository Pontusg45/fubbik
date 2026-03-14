import { and, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { activityLog } from "../schema/activity";

export function listActivity(
    userId: string,
    opts: { codebaseId?: string; entityType?: string; limit?: number; offset?: number } = {}
) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(activityLog.userId, userId)];
            if (opts.codebaseId) {
                conditions.push(eq(activityLog.codebaseId, opts.codebaseId));
            }
            if (opts.entityType) {
                conditions.push(eq(activityLog.entityType, opts.entityType));
            }
            return db
                .select()
                .from(activityLog)
                .where(and(...conditions))
                .orderBy(desc(activityLog.createdAt))
                .limit(opts.limit ?? 50)
                .offset(opts.offset ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createActivity(params: {
    id: string;
    userId: string;
    entityType: string;
    entityId: string;
    entityTitle?: string;
    action: string;
    codebaseId?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(activityLog).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
