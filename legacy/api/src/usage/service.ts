import { cypherVoid, escCypher } from "@fubbik/db/age/client";
import { getCoReferenceCounts, insertUsageEvent, isAgeAvailable } from "@fubbik/db/repository";
import { Effect } from "effect";

import { logger } from "../logger";

export function recordUsage(kind: "context_query" | "chunk_view" | "mcp_resolve", chunkIds: string[], userId: string, query?: string) {
    if (chunkIds.length === 0) return Effect.succeed(undefined);
    return insertUsageEvent({
        id: crypto.randomUUID(),
        kind,
        chunkIds,
        query,
        userId
    });
}

export function aggregateCoReferences(sinceDays = 1) {
    return Effect.gen(function* () {
        const since = new Date(Date.now() - sinceDays * 24 * 60 * 60 * 1000);
        const pairs = yield* getCoReferenceCounts(since, 2);

        if (pairs.length === 0) return { upserted: 0 };

        const ageReady = yield* Effect.promise(() => isAgeAvailable());
        if (!ageReady) return { upserted: 0 };

        let upserted = 0;
        for (const { chunk_a, chunk_b, co_count } of pairs) {
            yield* cypherVoid(`
                MATCH (a:chunk {id: '${escCypher(chunk_a)}'}), (b:chunk {id: '${escCypher(chunk_b)}'})
                MERGE (a)-[r:co_referenced]->(b)
                SET r.count = ${co_count}, r.lastSeenAt = '${new Date().toISOString()}'
            `);
            upserted++;
        }

        logger.info("Co-reference aggregation complete", { upserted });
        return { upserted };
    });
}
