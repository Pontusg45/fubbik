import { sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";

export function getTagsWithCounts(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await db.execute(sql`
                SELECT tag, COUNT(*)::int as count
                FROM ${chunk}, jsonb_array_elements_text(${chunk.tags}) AS tag
                WHERE ${chunk.userId} = ${userId}
                GROUP BY tag
                ORDER BY count DESC
            `);
            return result.rows as { tag: string; count: number }[];
        },
        catch: cause => new DatabaseError({ cause })
    });
}
