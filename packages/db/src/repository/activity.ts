import { and, desc, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { activityLog } from "../schema/activity";

export function listActivity(
    userId: string,
    opts: { codebaseId?: string; entityType?: string; entityId?: string; limit?: number; offset?: number } = {}
) {
    return dbEffect(() => {
            const conditions = [eq(activityLog.userId, userId)];
            if (opts.codebaseId) {
                conditions.push(eq(activityLog.codebaseId, opts.codebaseId));
            }
            if (opts.entityType) {
                conditions.push(eq(activityLog.entityType, opts.entityType));
            }
            if (opts.entityId) {
                conditions.push(eq(activityLog.entityId, opts.entityId));
            }
            return db
                .select()
                .from(activityLog)
                .where(and(...conditions))
                .orderBy(desc(activityLog.createdAt))
                .limit(opts.limit ?? 50)
                .offset(opts.offset ?? 0);
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
    return dbEffect(async () => {
            const [created] = await db.insert(activityLog).values(params).returning();
            return created!;
        });
}
