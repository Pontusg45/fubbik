import { gte, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { usageEvent } from "../schema/usage-event";

export function insertUsageEvent(data: {
    id: string;
    kind: string;
    chunkIds: string[];
    query?: string;
    userId: string;
}) {
    return dbEffect(async () => {
        await db.insert(usageEvent).values(data);
    });
}

export function getRecentUsageEvents(since: Date) {
    return dbEffect(async () => {
        return db
            .select()
            .from(usageEvent)
            .where(gte(usageEvent.createdAt, since))
            .orderBy(usageEvent.createdAt);
    });
}

export function getCoReferenceCounts(since: Date, minCount = 2) {
    return dbEffect(async () => {
        const rows = await db.execute(sql`
            WITH pairs AS (
                SELECT a.value::text AS chunk_a, b.value::text AS chunk_b
                FROM usage_event,
                     jsonb_array_elements_text(chunk_ids) a,
                     jsonb_array_elements_text(chunk_ids) b
                WHERE a.value::text < b.value::text
                  AND created_at >= ${since}
            )
            SELECT chunk_a, chunk_b, COUNT(*) AS co_count
            FROM pairs
            GROUP BY chunk_a, chunk_b
            HAVING COUNT(*) >= ${minCount}
            ORDER BY co_count DESC
        `);
        return rows.rows as Array<{ chunk_a: string; chunk_b: string; co_count: number }>;
    });
}
