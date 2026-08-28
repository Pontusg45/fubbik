import { cypher, cypherVoid, escCypher } from "@fubbik/db/age/client";
import { ensureVertex } from "@fubbik/db/age/sync";
import { getCoReferenceCounts, isAgeAvailable } from "@fubbik/db/repository";
import { Effect } from "effect";

import { logger } from "../logger";

interface ConceptCluster {
    chunkIds: string[];
    coRefCount: number;
}

function findClusters(pairs: Array<{ chunk_a: string; chunk_b: string; co_count: number }>, minSize = 3): ConceptCluster[] {
    const adjacency = new Map<string, Set<string>>();
    const pairCounts = new Map<string, number>();

    for (const { chunk_a, chunk_b, co_count } of pairs) {
        if (!adjacency.has(chunk_a)) adjacency.set(chunk_a, new Set());
        if (!adjacency.has(chunk_b)) adjacency.set(chunk_b, new Set());
        adjacency.get(chunk_a)!.add(chunk_b);
        adjacency.get(chunk_b)!.add(chunk_a);
        pairCounts.set(`${chunk_a}:${chunk_b}`, co_count);
    }

    const visited = new Set<string>();
    const clusters: ConceptCluster[] = [];

    for (const node of adjacency.keys()) {
        if (visited.has(node)) continue;
        const component: string[] = [];
        const queue = [node];
        let totalCount = 0;

        while (queue.length > 0) {
            const current = queue.shift()!;
            if (visited.has(current)) continue;
            visited.add(current);
            component.push(current);

            for (const neighbor of adjacency.get(current) ?? []) {
                if (!visited.has(neighbor)) {
                    queue.push(neighbor);
                    const key = [current, neighbor].sort().join(":");
                    totalCount += pairCounts.get(key) ?? 0;
                }
            }
        }

        if (component.length >= minSize) {
            clusters.push({ chunkIds: component, coRefCount: totalCount });
        }
    }

    return clusters;
}

export function detectEmergentConcepts(sinceDays = 30, minCoRef = 5) {
    return Effect.gen(function* () {
        const ageReady = yield* Effect.promise(() => isAgeAvailable());
        if (!ageReady) return { created: 0 };

        const since = new Date(Date.now() - sinceDays * 24 * 60 * 60 * 1000);
        const pairs = yield* getCoReferenceCounts(since, minCoRef);

        if (pairs.length === 0) return { created: 0 };

        const clusters = findClusters(pairs, 3);
        let created = 0;

        for (const cluster of clusters) {
            const label = `concept-${cluster.chunkIds.length}-${cluster.coRefCount}`;
            const conceptId = `concept:${label}`;

            yield* ensureVertex("concept", conceptId);
            yield* cypherVoid(
                `MATCH (c:concept {id: '${escCypher(conceptId)}'})
                 SET c.label = '${escCypher(label)}',
                     c.strength = ${cluster.coRefCount},
                     c.firstSeenAt = COALESCE(c.firstSeenAt, '${new Date().toISOString()}')`
            );

            for (const chunkId of cluster.chunkIds) {
                yield* cypherVoid(
                    `MATCH (concept:concept {id: '${escCypher(conceptId)}'}), (chunk:chunk {id: '${escCypher(chunkId)}'})
                     MERGE (concept)-[r:embodies]->(chunk)
                     SET r.strength = ${cluster.coRefCount}`
                );
            }

            created++;
        }

        logger.info("Emergent concepts detected", { created, clusters: clusters.length });
        return { created };
    });
}

export function listConcepts() {
    return Effect.gen(function* () {
        const ageReady = yield* Effect.promise(() => isAgeAvailable());
        if (!ageReady) return [];

        const rows = yield* cypher(
            `MATCH (c:concept)-[r:embodies]->(chunk:chunk)
             RETURN c.id AS cid, c.label AS label, c.strength AS strength,
                    collect(chunk.id) AS member_ids`,
            "cid agtype, label agtype, strength agtype, member_ids agtype"
        );

        return rows.map(row => ({
            id: String(row.cid).replace(/"/g, ""),
            label: String(row.label).replace(/"/g, ""),
            strength: Number(row.strength),
            memberChunkIds: String(row.member_ids)
                .replace(/[[\]"]/g, "")
                .split(",")
                .map(s => s.trim())
                .filter(Boolean)
        }));
    });
}
