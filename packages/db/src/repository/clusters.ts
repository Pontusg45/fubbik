import { and, isNull, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";

export interface Cluster {
    seedId: string;
    seedTitle: string;
    members: Array<{ id: string; title: string; type: string; similarity: number }>;
}

export function computeClusters(userId: string, maxClusters = 10, clusterSize = 8) {
    return Effect.tryPromise({
        try: async () => {
            // Pick recent chunks with embeddings as seeds
            const seeds = await db
                .select({ id: chunk.id, title: chunk.title, type: chunk.type })
                .from(chunk)
                .where(
                    and(
                        sql`${chunk.userId} = ${userId}`,
                        sql`${chunk.embedding} IS NOT NULL`,
                        isNull(chunk.archivedAt)
                    )
                )
                .orderBy(sql`${chunk.updatedAt} DESC`)
                .limit(maxClusters);

            const clusters: Cluster[] = [];
            const used = new Set<string>();

            for (const seed of seeds) {
                if (used.has(seed.id)) continue;

                const neighbors = await db
                    .select({
                        id: chunk.id,
                        title: chunk.title,
                        type: chunk.type,
                        similarity: sql<number>`1 - (${chunk.embedding} <=> (SELECT embedding FROM chunk WHERE id = ${seed.id}))`,
                    })
                    .from(chunk)
                    .where(
                        and(
                            sql`${chunk.userId} = ${userId}`,
                            sql`${chunk.embedding} IS NOT NULL`,
                            isNull(chunk.archivedAt),
                            sql`${chunk.id} != ${seed.id}`
                        )
                    )
                    .orderBy(sql`${chunk.embedding} <=> (SELECT embedding FROM chunk WHERE id = ${seed.id})`)
                    .limit(clusterSize);

                const members = neighbors.map(r => ({
                    id: r.id,
                    title: r.title,
                    type: r.type,
                    similarity: Number(r.similarity),
                }));

                used.add(seed.id);
                for (const m of members) used.add(m.id);

                clusters.push({
                    seedId: seed.id,
                    seedTitle: seed.title,
                    members,
                });
            }

            return clusters;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
